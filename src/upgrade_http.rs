use std::future::Future;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;

use crate::Either;
use http::header::HOST;
use http::header::LOCATION;
use http::uri::Authority;
use http::uri::Scheme;
use http::Request;
use http::Response;
use http::Uri;
use hyper::service::Service as HyperService;
use hyper::Body;
use pin_project_lite::pin_project;
use tower_layer::Layer;

pub struct UpgradeHttpLayer;

impl<Service> Layer<Service> for UpgradeHttpLayer {
    type Service = UpgradeHttp<Service>;

    fn layer(&self, service: Service) -> Self::Service {
        UpgradeHttp::new(service)
    }
}

#[derive(Clone)]
pub struct UpgradeHttp<Service> {
    service: Service,
}

impl<Service> UpgradeHttp<Service> {
    pub fn new(service: Service) -> Self {
        Self { service }
    }

    pub fn into_inner(self) -> Service {
        self.service
    }

    pub fn get_ref(&self) -> &Service {
        &self.service
    }

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

                    response.status(301).header(LOCATION, uri.to_string())
                } else {
                    // If we can't extract the host, tell the client there is something wrong with their request.
                    response.status(400)
                }
                .body(Body::empty())
                .expect("invalid header or body");

                UpgradeHttpFuture::new_upgrade(response)
            }
        }
    }
}

pin_project! {
    pub struct UpgradeHttpFuture<Service, Request>
    where
        Service: HyperService<Request>,
    {
        #[pin]
        inner: UpgradeHttpFutureInner<Service, Request>,
    }
}

pin_project! {
    #[project = UpgradeHttpFutureProj]
    pub enum UpgradeHttpFutureInner<Service, Request>
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

impl<Service, Request> UpgradeHttpFuture<Service, Request>
where
    Service: HyperService<Request>,
{
    fn new_service(future: Service::Future) -> Self {
        Self {
            inner: UpgradeHttpFutureInner::Service { future },
        }
    }

    fn new_upgrade(response: Response<Body>) -> Self {
        Self {
            inner: UpgradeHttpFutureInner::Upgrade {
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
