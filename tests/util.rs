use std::error::Error as StdError;
use std::future::Future;
use std::net::SocketAddr;

use anyhow::{Error, Result};
use axum::{Router, ServiceExt};
use axum_server::tls_rustls::RustlsConfig;
use axum_server::{Handle, Server};
use axum_server_dual_protocol::DualProtocolAcceptor;
use bytes::Bytes;
use futures_util::{future, TryFutureExt};
use http::{Request, Response};
use hyper::body::HttpBody;
use hyper::service::Service;
use hyper::Body;
use reqwest::Certificate;

pub(crate) fn server(address: SocketAddr, config: RustlsConfig) -> Server<DualProtocolAcceptor> {
	axum_server_dual_protocol::bind_dual_protocol(address, config)
}

// TODO: False-positive: <https://github.com/rust-lang/rust-clippy/issues/9076>.
#[allow(clippy::trait_duplication_in_bounds)]
pub(crate) async fn test<
	RouterBody,
	ResponseBody,
	ServerFn,
	ServerLogicFn,
	ClientFn,
	ClientFuture,
>(
	server: ServerFn,
	server_logic: ServerLogicFn,
	app: Router<RouterBody>,
	client_logic: ClientFn,
) -> Result<()>
where
	RouterBody: 'static,
	Router<RouterBody>: Service<Request<Body>, Response = Response<ResponseBody>>,
	<Router<RouterBody> as Service<Request<Body>>>::Error: StdError + Send + Sync,
	<Router<RouterBody> as Service<Request<Body>>>::Future: Send,
	ResponseBody: 'static + HttpBody<Data = Bytes> + Send,
	<ResponseBody as HttpBody>::Error: StdError + Send + Sync,
	ServerFn: 'static + FnOnce(SocketAddr, RustlsConfig) -> Server<DualProtocolAcceptor> + Send,
	ServerLogicFn:
		'static + FnOnce(Server<DualProtocolAcceptor>) -> Server<DualProtocolAcceptor> + Send,
	ClientFn: 'static + FnOnce(Certificate, SocketAddr) -> ClientFuture + Send,
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
			let address = SocketAddr::from(([127, 0, 0, 1], 0));

			let mut server = server(address, config).handle(handle);

			server = server_logic(server);

			server.serve(app.into_make_service()).await?;

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
