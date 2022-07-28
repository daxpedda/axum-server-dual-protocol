//! Dual-protocol server implementation.
//!
//! See [`bind_dual_protocol`] and [`DualProtocolAcceptor`].

use std::future::Future;
use std::io::ErrorKind;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::{io, slice};

use axum_server::accept::Accept;
use axum_server::tls_rustls::{RustlsAcceptor, RustlsConfig};
use axum_server::Server;
use hyper::server::conn::AddrStream;
use pin_project::pin_project;
use tokio::io::ReadBuf;
use tokio_rustls::server::TlsStream;
use tokio_util::either::Either;

/// Create a [`Server`] that will bind to provided address, accepting both HTTP
/// and HTTPS on the same port.
#[must_use]
pub fn bind_dual_protocol(
	address: SocketAddr,
	config: RustlsConfig,
) -> Server<DualProtocolAcceptor> {
	let acceptor = DualProtocolAcceptor::new(config);

	Server::bind(address).acceptor(acceptor)
}

/// Simultaneous HTTP and HTTPS [`Accept`]or.
#[derive(Debug, Clone)]
pub struct DualProtocolAcceptor {
	/// [`RustlsAcceptor`] re-used to handle HTTPS requests.
	rustls: RustlsAcceptor,
}

impl DualProtocolAcceptor {
	/// Create a new [`DualProtocolAcceptor`].
	#[must_use]
	pub fn new(config: RustlsConfig) -> Self {
		Self {
			rustls: RustlsAcceptor::new(config),
		}
	}
}

impl<Service> Accept<AddrStream, Service> for DualProtocolAcceptor {
	type Stream = Either<TlsStream<AddrStream>, AddrStream>;
	type Service = Service;
	type Future = DualProtocolFuture<Service>;

	fn accept(&self, stream: AddrStream, service: Service) -> Self::Future {
		DualProtocolFuture::new(stream, service, self.rustls.clone())
	}
}

/// [`Future`](Accept::Future) type for [`DualProtocolAcceptor`].
#[derive(Debug)]
#[pin_project(project = DualProtocolFutureProj)]
pub struct DualProtocolFuture<Service>(
	/// State. `enum` variants can't be private, so this solution was used to
	/// hide implementation details.
	#[pin]
	FutureState<Service>,
);

/// State of accepting a new request for [`DualProtocolFuture`].
#[derive(Debug)]
#[pin_project(project = FutuereStateProj)]
enum FutureState<Service> {
	/// Peeking state, still trying to determine if the incoming request is HTTP
	/// or HTTPS.
	Peek(Option<PeekState<Service>>),
	/// HTTPS state, it was determined that the incoming request is HTTPS, now
	/// the [`RustlsAcceptor`] has to be polled to completion.
	Https(#[pin] <RustlsAcceptor as Accept<AddrStream, Service>>::Future),
}

/// Data necessary to peek and proceed to the next state.
#[derive(Debug)]
struct PeekState<Service> {
	/// Transport.
	stream: AddrStream,
	/// User-provided [`Service`](hyper::service::Service)
	service: Service,
	/// Used to proceed to the [`Https`](FutureState::Https) state if
	/// necessary.
	rustls: RustlsAcceptor,
}

impl<Service> DualProtocolFuture<Service> {
	/// Create a new [`DualProtocolFuture`] in the [`Peek`](FutureState::Peek)
	/// state.
	const fn new(stream: AddrStream, service: Service, rustls: RustlsAcceptor) -> Self {
		Self(FutureState::Peek(Some(PeekState {
			stream,
			service,
			rustls,
		})))
	}
}

impl<Service> DualProtocolFutureProj<'_, Service> {
	/// Proceed to the [`Https`](FutureState::Https) state.
	fn upgrade(&mut self, future: <RustlsAcceptor as Accept<AddrStream, Service>>::Future) {
		self.0.set(FutureState::Https(future));
	}
}

impl<Service> Future for DualProtocolFuture<Service> {
	type Output = io::Result<(Either<TlsStream<AddrStream>, AddrStream>, Service)>;

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
								this.upgrade(rustls.accept(stream, service));
							} else {
								return Poll::Ready(Ok((Either::Right(stream), service)));
							}
						}
						Poll::Ready(Err(err)) => return Poll::Ready(Err(err)),
						Poll::Pending => return Poll::Pending,
					}
				}
				FutuereStateProj::Https(future) => {
					return future
						.poll(cx)
						.map_ok(|(stream, service)| (Either::Left(stream), service))
				}
			}
		}
	}
}
