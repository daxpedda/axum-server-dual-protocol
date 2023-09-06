//! Dual-protocol server implementation.
//!
//! See [`bind_dual_protocol()`] and [`DualProtocolAcceptor`].

use std::fmt::{self, Debug, Formatter};
use std::future::Future;
use std::io::ErrorKind;
use std::net::{SocketAddr, TcpListener};
use std::pin::Pin;
use std::task::{Context, Poll};
use std::{io, slice};

use axum_server::accept::Accept;
use axum_server::tls_rustls::{RustlsAcceptor, RustlsConfig};
use axum_server::Server;
use http::{Request, Response};
use hyper::server::conn::AddrStream;
use hyper::service::Service as HyperService;
use hyper::Body;
use pin_project::pin_project;
use tokio::io::ReadBuf;
use tokio_rustls::server::TlsStream;
use tokio_util::either::Either as TokioEither;

use crate::{Either as BodyEither, UpgradeHttp};

/// Create a [`Server`] that will bind to the provided address, accepting both
/// HTTP and HTTPS on the same port.
#[must_use]
pub fn bind_dual_protocol(
	address: SocketAddr,
	config: RustlsConfig,
) -> Server<DualProtocolAcceptor> {
	let acceptor = DualProtocolAcceptor::new(config);

	Server::bind(address).acceptor(acceptor)
}

/// Create a [`Server`] from an existing [`TcpListener`], accepting both
/// HTTP and HTTPS on the same port.
#[must_use]
pub fn from_tcp_dual_protocol(
	listener: TcpListener,
	config: RustlsConfig,
) -> Server<DualProtocolAcceptor> {
	let acceptor = DualProtocolAcceptor::new(config);

	Server::from_tcp(listener).acceptor(acceptor)
}

/// Supplies configuration methods for [`Server`] with [`DualProtocolAcceptor`].
///
/// See [`bind_dual_protocol()`] for easy creation.
pub trait ServerExt {
	/// Set if HTTP connections should be automatically upgraded to HTTPS.
	///
	/// See [`UpgradeHttp`] for more details.
	#[must_use]
	fn set_upgrade(self, upgrade: bool) -> Self;
}

impl ServerExt for Server<DualProtocolAcceptor> {
	fn set_upgrade(mut self, upgrade: bool) -> Self {
		self.get_mut().set_upgrade(upgrade);
		self
	}
}

/// The protocol used by this connection. See
/// [`Request::extensions()`](Request::extensions()).
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Protocol {
	/// This connection is encrypted with TLS.
	Tls,
	/// This connection is unencrypted.
	Plain,
}

/// Simultaneous HTTP and HTTPS [`Accept`]or.
#[derive(Debug, Clone)]
pub struct DualProtocolAcceptor {
	/// [`RustlsAcceptor`] re-used to handle HTTPS requests.
	rustls: RustlsAcceptor,
	/// Stores if HTTP connections should be automatically upgraded to HTTPS.
	///
	/// See [`UpgradeHttp`] for more details.
	upgrade: bool,
}

impl DualProtocolAcceptor {
	/// Create a new [`DualProtocolAcceptor`].
	#[must_use]
	pub fn new(config: RustlsConfig) -> Self {
		Self {
			rustls: RustlsAcceptor::new(config),
			upgrade: false,
		}
	}

	/// Set if HTTP connections should be automatically upgraded to HTTPS.
	///
	/// See [`UpgradeHttp`] for more details.
	pub fn set_upgrade(&mut self, upgrade: bool) {
		self.upgrade = upgrade;
	}
}

impl<Service> Accept<AddrStream, Service> for DualProtocolAcceptor {
	type Stream = TokioEither<TlsStream<AddrStream>, AddrStream>;
	type Service = DualProtocolService<Service>;
	type Future = DualProtocolAcceptorFuture<Service>;

	fn accept(&self, stream: AddrStream, service: Service) -> Self::Future {
		let service = if self.upgrade {
			DualProtocolServiceBuilder::new_upgrade(service)
		} else {
			DualProtocolServiceBuilder::new_service(service)
		};

		DualProtocolAcceptorFuture::new(stream, service, self.rustls.clone())
	}
}

/// [`Future`](Accept::Future) type for [`DualProtocolAcceptor`].
#[derive(Debug)]
#[pin_project(project = DualProtocolAcceptorFutureProj)]
pub struct DualProtocolAcceptorFuture<Service>(
	/// State. `enum` variants can't be private, so this solution was used to
	/// hide implementation details.
	#[pin]
	FutureState<Service>,
);

/// State of accepting a new request for [`DualProtocolAcceptorFuture`].
#[derive(Debug)]
#[pin_project(project = FutuereStateProj)]
enum FutureState<Service> {
	/// Peeking state, still trying to determine if the incoming request is HTTP
	/// or HTTPS.
	Peek(Option<PeekState<Service>>),
	/// HTTPS state, it was determined that the incoming request is HTTPS, now
	/// the [`RustlsAcceptor`] has to be polled to completion.
	Https(#[pin] <RustlsAcceptor as Accept<AddrStream, DualProtocolService<Service>>>::Future),
}

/// Data necessary to peek and proceed to the next state.
#[derive(Debug)]
struct PeekState<Service> {
	/// Transport.
	stream: AddrStream,
	/// User-provided [`Service`](hyper::service::Service)
	service: DualProtocolServiceBuilder<Service>,
	/// Used to proceed to the [`Https`](FutureState::Https) state if
	/// necessary.
	rustls: RustlsAcceptor,
}

impl<Service> DualProtocolAcceptorFuture<Service> {
	/// Create a new [`DualProtocolAcceptorFuture`] in the
	/// [`Peek`](FutureState::Peek) state.
	const fn new(
		stream: AddrStream,
		service: DualProtocolServiceBuilder<Service>,
		rustls: RustlsAcceptor,
	) -> Self {
		Self(FutureState::Peek(Some(PeekState {
			stream,
			service,
			rustls,
		})))
	}
}

impl<Service> DualProtocolAcceptorFutureProj<'_, Service> {
	/// Proceed to the [`Https`](FutureState::Https) state.
	fn upgrade(
		&mut self,
		future: <RustlsAcceptor as Accept<AddrStream, DualProtocolService<Service>>>::Future,
	) {
		self.0.set(FutureState::Https(future));
	}
}

impl<Service> Future for DualProtocolAcceptorFuture<Service> {
	type Output = io::Result<(
		TokioEither<TlsStream<AddrStream>, AddrStream>,
		DualProtocolService<Service>,
	)>;

	fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
		let mut this = self.project();

		// After successfully peeking, continue without unnecessary yielding.
		loop {
			match this.0.as_mut().project() {
				FutuereStateProj::Peek(inner) => {
					let peek = inner.as_mut().expect("polled again after `Poll::Ready`");

					let mut byte = 0;
					let mut buffer = ReadBuf::new(slice::from_mut(&mut byte));

					match peek.stream.poll_peek(cx, &mut buffer) {
						// If `MSG_PEEK` returns `0`, the socket was closed.
						Poll::Ready(Ok(0)) => {
							return Poll::Ready(Err(ErrorKind::UnexpectedEof.into()))
						}
						Poll::Ready(Ok(_)) => {
							let PeekState {
								stream,
								service,
								rustls,
							} = inner.take().expect("`inner` was already consumed");

							// The first byte in the TLS protocol is always `0x16`.
							if byte == 0x16 {
								this.upgrade(rustls.accept(stream, service.build(Protocol::Tls)));
							} else {
								return Poll::Ready(Ok((
									TokioEither::Right(stream),
									service.build(Protocol::Plain),
								)));
							}
						}
						Poll::Ready(Err(err)) => return Poll::Ready(Err(err)),
						Poll::Pending => return Poll::Pending,
					}
				}
				FutuereStateProj::Https(future) => {
					return future
						.poll(cx)
						.map_ok(|(stream, service)| (TokioEither::Left(stream), service))
				}
			}
		}
	}
}

/// Hold the user-supplied app until the protocol type is determined.
#[derive(Debug)]
struct DualProtocolServiceBuilder<Service>(ServiceServe<Service>);

/// [`Service`](HyperService) wrapping user-supplied app to apply global
/// [`Layer`](tower_layer::Layer)s according to configuration.
#[derive(Debug)]
pub struct DualProtocolService<Service> {
	/// The user-supplied [`Service`](HyperService).
	service: ServiceServe<Service>,
	/// The protocol this connection is using.
	protocol: Protocol,
}

/// Holds [`Service`](HyperService) to serve for [`DualProtocolService`].
#[derive(Debug)]
enum ServiceServe<Service> {
	/// No configuration applied, so we will pass-through the user-supplied
	/// [`Service`](HyperService) as is.
	Service(Service),
	/// Configured to automatically upgrade HTTP requests to HTTPS, so we wrap
	/// the user-supplied [`Service`](HyperService) in the [`UpgradeHttp`]
	/// [`Service`](HyperService).
	Upgrade(UpgradeHttp<Service>),
}

impl<Service> DualProtocolServiceBuilder<Service> {
	/// Create a [`DualProtocolService`] in the
	/// [`Service`](ServiceServe::Service) state.
	const fn new_service(service: Service) -> Self {
		Self(ServiceServe::Service(service))
	}

	/// Create a [`DualProtocolService`] in the
	/// [`Upgrade`](ServiceServe::Upgrade) state.
	const fn new_upgrade(service: Service) -> Self {
		Self(ServiceServe::Upgrade(UpgradeHttp::new(service)))
	}

	/// Create a [`DualProtocolService`] when the protocol is established.
	#[allow(clippy::missing_const_for_fn)]
	fn build(self, protocol: Protocol) -> DualProtocolService<Service> {
		DualProtocolService {
			service: self.0,
			protocol,
		}
	}
}

impl<Service, RequestBody, ResponseBody> HyperService<Request<RequestBody>>
	for DualProtocolService<Service>
where
	Service: HyperService<Request<RequestBody>, Response = Response<ResponseBody>>,
{
	type Response = Response<BodyEither<ResponseBody, BodyEither<ResponseBody, Body>>>;
	type Error = Service::Error;
	type Future = DualProtocolServiceFuture<Service, RequestBody, ResponseBody>;

	fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
		match &mut self.service {
			ServiceServe::Service(service) => service.poll_ready(cx),
			ServiceServe::Upgrade(service) => service.poll_ready(cx),
		}
	}

	fn call(&mut self, mut request: Request<RequestBody>) -> Self::Future {
		let _ = request.extensions_mut().insert(self.protocol);

		match &mut self.service {
			ServiceServe::Service(service) => {
				DualProtocolServiceFuture::new_service(service.call(request))
			}
			ServiceServe::Upgrade(service) => {
				DualProtocolServiceFuture::new_upgrade(service.call(request))
			}
		}
	}
}

/// [`Future`](HyperService::Future) type for [`DualProtocolService`].
#[pin_project]
pub struct DualProtocolServiceFuture<Service, RequestBody, ResponseBody>(
	#[pin] FutureServe<Service, RequestBody, ResponseBody>,
)
where
	Service: HyperService<Request<RequestBody>, Response = Response<ResponseBody>>;

/// Holds [`Future`] to serve for [`DualProtocolServiceFuture`].
#[derive(Debug)]
#[pin_project(project = DualProtocolServiceFutureProj)]
enum FutureServe<Service, RequestBody, ResponseBody>
where
	Service: HyperService<Request<RequestBody>, Response = Response<ResponseBody>>,
{
	/// Pass-through the user-supplied [`Future`](HyperService::Future).
	Service(#[pin] Service::Future),
	/// Use the [`UpgradeHttp`] [`Future`](HyperService::Future).
	Upgrade(#[pin] <UpgradeHttp<Service> as HyperService<Request<RequestBody>>>::Future),
}

// Rust can't figure out the correct bounds.
impl<Service, RequestBody, ResponseBody> Debug
	for DualProtocolServiceFuture<Service, RequestBody, ResponseBody>
where
	Service: HyperService<Request<RequestBody>, Response = Response<ResponseBody>>,
	FutureServe<Service, RequestBody, ResponseBody>: Debug,
{
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		f.debug_tuple("DualProtocolServiceFuture")
			.field(&self.0)
			.finish()
	}
}

impl<Service, RequestBody, ResponseBody>
	DualProtocolServiceFuture<Service, RequestBody, ResponseBody>
where
	Service: HyperService<Request<RequestBody>, Response = Response<ResponseBody>>,
{
	/// Create a [`DualProtocolServiceFuture`] in the
	/// [`Service`](FutureServe::Service) state.
	const fn new_service(future: Service::Future) -> Self {
		Self(FutureServe::Service(future))
	}

	/// Create a [`DualProtocolServiceFuture`] in the
	/// [`Upgrade`](FutureServe::Upgrade) state.
	const fn new_upgrade(
		future: <UpgradeHttp<Service> as HyperService<Request<RequestBody>>>::Future,
	) -> Self {
		Self(FutureServe::Upgrade(future))
	}
}

impl<Service, RequestBody, ResponseBody> Future
	for DualProtocolServiceFuture<Service, RequestBody, ResponseBody>
where
	Service: HyperService<Request<RequestBody>, Response = Response<ResponseBody>>,
{
	type Output =
		Result<Response<BodyEither<ResponseBody, BodyEither<ResponseBody, Body>>>, Service::Error>;

	fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
		match self.project().0.project() {
			DualProtocolServiceFutureProj::Service(future) => future
				.poll(cx)
				.map_ok(|response| response.map(BodyEither::Left)),
			DualProtocolServiceFutureProj::Upgrade(future) => future
				.poll(cx)
				.map_ok(|response| response.map(BodyEither::Right)),
		}
	}
}
