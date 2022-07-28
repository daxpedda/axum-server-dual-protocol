use anyhow::{Error, Result};
use axum::Router;
use axum_server::{tls_rustls::RustlsConfig, Handle};
use futures_util::{future, TryFutureExt};
use http::Request;
use http::Response;
use hyper::body::HttpBody;
use hyper::service::Service;
use hyper::Body;
use reqwest::Certificate;
use std::error::Error as StdError;
use std::future::Future;
use std::net::SocketAddr;

pub async fn test<RouterBody, ResponseBody, ClientFn, ClientFuture>(
    app: Router<RouterBody>,
    client_logic: ClientFn,
) -> Result<()>
where
    RouterBody: 'static + HttpBody + Send,
    Router<RouterBody>: Service<Request<Body>, Response = Response<ResponseBody>>,
    <Router<RouterBody> as Service<Request<Body>>>::Error: StdError + Send + Sync,
    <Router<RouterBody> as Service<Request<Body>>>::Future: Send,
    ResponseBody: 'static + HttpBody + Send,
    <ResponseBody as HttpBody>::Data: Send,
    <ResponseBody as HttpBody>::Error: StdError + Send + Sync,
    ClientFn: 'static + Fn(Certificate, SocketAddr) -> ClientFuture + Send + Sync,
    ClientFuture: Future<Output = Result<()>> + Send,
{
    let handle = Handle::new();

    let key_pair = rcgen::generate_simple_self_signed([String::from("localhost")])?;
    let certificate = key_pair.serialize_der()?;

    let server = tokio::spawn({
        let handle = handle.clone();
        let certificate = certificate.clone();

        async move {
            let config =
                RustlsConfig::from_der(vec![certificate], key_pair.serialize_private_key_der())
                    .await?;

            axum_server_upgrade_http::bind_dual(SocketAddr::from(([127, 0, 0, 1], 0)), config)
                .handle(handle)
                .serve(app.into_make_service())
                .await?;

            Result::<_, Error>::Ok(())
        }
    });

    let client = tokio::spawn(async move {
        let certificate = Certificate::from_der(&certificate)?;
        let address = handle.listening().await.expect("failed to bind socket");

        client_logic(certificate, address).await?;

        handle.graceful_shutdown(None);

        Result::<_, Error>::Ok(())
    });

    future::try_join(
        server.map_err(Error::from).and_then(future::ready),
        client.map_err(Error::from).and_then(future::ready),
    )
    .await?;

    Ok(())
}
