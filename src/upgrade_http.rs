//! HTTP to HTTPS upgrade implementation.
//!
//! See [`UpgradeHttp`].

use std::fmt::{self, Debug, Formatter};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::Bytes;
use http::header::{HOST, LOCATION, UPGRADE};
use http::uri::{Authority, Scheme};
use http::{HeaderValue, Request, Response, StatusCode, Uri};
use http_body_util::{Either, Empty};
use pin_project::pin_project;
use tower_layer::Layer;
use tower_service::Service as TowerService;

use crate::Protocol;

/// [`Layer`] upgrading HTTP requests to HTTPS.
///
/// See [`UpgradeHttp`] for more details.
#[derive(Clone, Copy, Debug)]
pub struct UpgradeHttpLayer;

impl<Service> Layer<Service> for UpgradeHttpLayer {
	type Service = UpgradeHttp<Service>;

	fn layer(&self, inner: Service) -> Self::Service {
		UpgradeHttp::new(inner)
	}
}

/// [`Service`](TowerService) upgrading HTTP requests to HTTPS by using a
/// [301 "Moved Permanently"](https://tools.ietf.org/html/rfc7231#section-6.4.2)
/// status code.
///
/// Note that this [`Service`](TowerService) always redirects with the given
/// path and query. Depending on how you apply this [`Service`](TowerService) it
/// will redirect even in the case of a resulting 404 "Not Found" status code at
/// the destination.
#[derive(Clone, Debug)]
pub struct UpgradeHttp<Service> {
	/// Wrapped user-proided [`Service`](TowerService).
	service: Service,
}

impl<Service> UpgradeHttp<Service> {
	/// Creates a new [`UpgradeHttp`].
	pub const fn new(service: Service) -> Self {
		Self { service }
	}

	/// Consumes the [`UpgradeHttp`], returning the wrapped
	/// [`Service`](TowerService).
	pub fn into_inner(self) -> Service {
		self.service
	}

	/// Return a reference to the wrapped [`Service`](TowerService).
	pub const fn get_ref(&self) -> &Service {
		&self.service
	}

	/// Return a mutable reference to the wrapped [`Service`](TowerService).
	pub fn get_mut(&mut self) -> &mut Service {
		&mut self.service
	}
}

impl<Service, RequestBody, ResponseBody> TowerService<Request<RequestBody>> for UpgradeHttp<Service>
where
	Service: TowerService<Request<RequestBody>, Response = Response<ResponseBody>>,
{
	type Response = Response<Either<ResponseBody, Empty<Bytes>>>;
	type Error = Service::Error;
	type Future = UpgradeHttpFuture<Service, Request<RequestBody>>;

	fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
		self.service.poll_ready(cx)
	}

	fn call(&mut self, req: Request<RequestBody>) -> Self::Future {
		match req
			.extensions()
			.get::<Protocol>()
			.expect("`Protocol` should always be set by `DualProtocolService`")
		{
			Protocol::Tls => UpgradeHttpFuture::new_service(self.service.call(req)),
			Protocol::Plain => {
				let response = Response::builder();

				let response = if let Some((authority, scheme)) =
					extract_authority(&req).and_then(|authority| {
						let uri = req.uri();

						// Depending on the scheme we need a different scheme to redirect to.

						// WebSocket handshakes often don't send a scheme, so we check the "Upgrade"
						// header as well.
						if uri.scheme_str() == Some("ws")
							|| req.headers().get(UPGRADE)
								== Some(&HeaderValue::from_static("websocket"))
						{
							Some((
								authority,
								Scheme::try_from("wss").expect("ASCII string is valid"),
							))
						}
						// HTTP requests often don't send a scheme.
						else if uri.scheme() == Some(&Scheme::HTTP) || uri.scheme_str().is_none()
						{
							Some((authority, Scheme::HTTPS))
						}
						// Unknown scheme, abort.
						else {
							None
						}
					}) {
					// Build URI to redirect to.
					let mut uri = Uri::builder().scheme(scheme).authority(authority);

					if let Some(path_and_query) = req.uri().path_and_query() {
						uri = uri.path_and_query(path_and_query.clone());
					}

					let uri = uri.build().expect("invalid path and query");

					response
						.status(StatusCode::MOVED_PERMANENTLY)
						.header(LOCATION, uri.to_string())
				} else {
					// If we can't extract the host or have an unknown scheme, tell the client there
					// is something wrong with their request.
					response.status(StatusCode::BAD_REQUEST)
				}
				.body(Empty::new())
				.expect("invalid header or body");

				UpgradeHttpFuture::new_upgrade(response)
			}
		}
	}
}

/// [`Future`](TowerService::Future) type for [`UpgradeHttp`].
#[pin_project]
pub struct UpgradeHttpFuture<Service, Request>(#[pin] FutureServe<Service, Request>)
where
	Service: TowerService<Request>;

/// Holds [`Future`] to serve for [`UpgradeHttpFuture`].
#[derive(Debug)]
#[pin_project(project = UpgradeHttpFutureProj)]
enum FutureServe<Service, Request>
where
	Service: TowerService<Request>,
{
	/// The request was using the HTTPS protocol, so we
	/// will pass-through the wrapped [`Service`](TowerService).
	Service(#[pin] Service::Future),
	/// The request was using the HTTP protocol, so we
	/// will upgrade the connection.
	Upgrade(Option<Response<Empty<Bytes>>>),
}

// Rust can't figure out the correct bounds.
impl<Service, Request> Debug for UpgradeHttpFuture<Service, Request>
where
	Service: TowerService<Request>,
	FutureServe<Service, Request>: Debug,
{
	fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
		formatter
			.debug_tuple("UpgradeHttpFuture")
			.field(&self.0)
			.finish()
	}
}

impl<Service, Request> UpgradeHttpFuture<Service, Request>
where
	Service: TowerService<Request>,
{
	/// Create a [`UpgradeHttpFuture`] in the [`Service`](FutureServe::Service)
	/// state.
	const fn new_service(future: Service::Future) -> Self {
		Self(FutureServe::Service(future))
	}

	/// Create a [`UpgradeHttpFuture`] in the [`Upgrade`](FutureServe::Upgrade)
	/// state.
	const fn new_upgrade(response: Response<Empty<Bytes>>) -> Self {
		Self(FutureServe::Upgrade(Some(response)))
	}
}

impl<Service, Request, ResponseBody> Future for UpgradeHttpFuture<Service, Request>
where
	Service: TowerService<Request, Response = Response<ResponseBody>>,
{
	type Output = Result<Response<Either<ResponseBody, Empty<Bytes>>>, Service::Error>;

	fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
		match self.project().0.project() {
			UpgradeHttpFutureProj::Service(future) => {
				future.poll(cx).map_ok(|result| result.map(Either::Left))
			}
			UpgradeHttpFutureProj::Upgrade(response) => Poll::Ready(Ok(response
				.take()
				.expect("polled again after `Poll::Ready`")
				.map(Either::Right))),
		}
	}
}

/// Extracts the host from a request, converting it to an [`Authority`].
fn extract_authority<Body>(request: &Request<Body>) -> Option<Authority> {
	/// `X-Forwarded-Host` header string.
	const X_FORWARDED_HOST: &str = "x-forwarded-host";

	let headers = request.headers();

	headers
		.get(X_FORWARDED_HOST)
		.or_else(|| headers.get(HOST))
		.and_then(|header| header.to_str().ok())
		.or_else(|| request.uri().host())
		.and_then(|host| Authority::try_from(host).ok())
}
