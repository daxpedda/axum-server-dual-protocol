use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use http::header::{HOST, LOCATION};
use http::uri::{Authority, Scheme};
use http::{Request, Response, StatusCode, Uri};
use hyper::service::Service as HyperService;
use hyper::Body;
use pin_project_lite::pin_project;
use tower_layer::Layer;

use crate::Either;

/// [`Layer`] upgrading HTTP requests to HTTPS. See [`UpgradeHttp`] for more
/// details.
#[derive(Clone, Copy, Debug)]
pub struct UpgradeHttpLayer;

impl<Service> Layer<Service> for UpgradeHttpLayer {
	type Service = UpgradeHttp<Service>;

	fn layer(&self, service: Service) -> Self::Service {
		UpgradeHttp::new(service)
	}
}

/// [`Service`](HyperService) upgrading HTTP requests to HTTPS by using a
/// [301 "Moved Permanently"](https://tools.ietf.org/html/rfc7231#section-6.4.2)
/// status code.
///
/// Note that this [`Service`](HyperService) always redirects with the given
/// path and query. Depending on how you apply this [`Service`](HyperService) it
/// will redirect even in the case of a resulting 404 "Not Found" status code at
/// the destination.
#[derive(Clone, Debug)]
pub struct UpgradeHttp<Service> {
	service: Service,
}

impl<Service> UpgradeHttp<Service> {
	/// Creates a new [`UpgradeHttp`].
	pub const fn new(service: Service) -> Self {
		Self { service }
	}

	/// Consumes the [`UpgradeHttp`], returning the wrapped
	/// [`Service`](HyperService).
	#[allow(clippy::missing_const_for_fn)]
	pub fn into_inner(self) -> Service {
		self.service
	}

	/// Return a reference to the wrapped [`Service`](HyperService).
	pub const fn get_ref(&self) -> &Service {
		&self.service
	}

	/// Return a mutable reference to the wrapped [`Service`](HyperService).
	pub fn get_mut(&mut self) -> &mut Service {
		&mut self.service
	}
}

impl<Service, RequestBody, ResponseBody> HyperService<Request<RequestBody>> for UpgradeHttp<Service>
where
	Service: HyperService<Request<RequestBody>, Response = Response<ResponseBody>>,
{
	type Response = Response<Either<ResponseBody, Body>>;
	type Error = Service::Error;
	type Future = UpgradeHttpFuture<Service, Request<RequestBody>>;

	fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
		self.service.poll_ready(cx)
	}

	fn call(&mut self, request: Request<RequestBody>) -> Self::Future {
		match request.uri().scheme() {
			Some(scheme) if scheme == &Scheme::HTTPS => {
				UpgradeHttpFuture::new_service(self.service.call(request))
			}
			// HTTP calls might not have their scheme set.
			_ => {
				let response = Response::builder();

				let response = if let Some(authority) = extract_authority(&request) {
					// Build URI to redirect too.
					let mut uri = Uri::builder().scheme(Scheme::HTTPS).authority(authority);

					if let Some(path_and_query) = request.uri().path_and_query() {
						uri = uri.path_and_query(path_and_query.clone());
					}

					let uri = uri.build().expect("invalid path and query");

					response
						.status(StatusCode::MOVED_PERMANENTLY)
						.header(LOCATION, uri.to_string())
				} else {
					// If we can't extract the host, tell the client there is something wrong with
					// their request.
					response.status(StatusCode::BAD_REQUEST)
				}
				.body(Body::empty())
				.expect("invalid header or body");

				UpgradeHttpFuture::new_upgrade(response)
			}
		}
	}
}

pin_project! {
	/// [`Future`](HyperService::Future) type for [`UpgradeHttp`].
	pub struct UpgradeHttpFuture<Service, Request>
	where
		Service: HyperService<Request>,
	{
		#[pin]
		inner: FutureInner<Service, Request>,
	}
}

pin_project! {
	#[project = UpgradeHttpFutureProj]
	enum FutureInner<Service, Request>
	where
		Service: HyperService<Request>
	{
		Service {
			#[pin]
			future: Service::Future,
		},
		Upgrade {
			response: Option<Response<Body>>,
		},
	}
}

// TODO: This was stabilized in 1.61, our MSRV is 1.56 currently because of
// `axum`. See <https://github.com/rust-lang/rust/issues/93706>.
#[allow(clippy::missing_const_for_fn)]
impl<Service, Request> UpgradeHttpFuture<Service, Request>
where
	Service: HyperService<Request>,
{
	fn new_service(future: Service::Future) -> Self {
		Self {
			inner: FutureInner::Service { future },
		}
	}

	fn new_upgrade(response: Response<Body>) -> Self {
		Self {
			inner: FutureInner::Upgrade {
				response: Some(response),
			},
		}
	}
}

impl<Service, Request, ResponseBody> Future for UpgradeHttpFuture<Service, Request>
where
	Service: HyperService<Request, Response = Response<ResponseBody>>,
{
	type Output = Result<Response<Either<ResponseBody, Body>>, Service::Error>;

	fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
		match self.project().inner.project() {
			UpgradeHttpFutureProj::Service { future } => {
				future.poll(cx).map_ok(|result| result.map(Either::Left))
			}
			UpgradeHttpFutureProj::Upgrade { response } => Poll::Ready(Ok(response
				.take()
				.expect("polled again after `Poll::Ready`")
				.map(Either::Right))),
		}
	}
}

fn extract_authority<Body>(request: &Request<Body>) -> Option<Authority> {
	const X_FORWARDED_HOST: &str = "x-forwarded-host";

	let headers = request.headers();

	headers
		.get(X_FORWARDED_HOST)
		.or_else(|| headers.get(HOST))
		.and_then(|header| header.to_str().ok())
		.or_else(|| request.uri().host())
		.and_then(|host| Authority::try_from(host).ok())
}
