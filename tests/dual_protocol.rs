mod util;

use std::convert;
use std::net::TcpListener;

use anyhow::Result;
use axum::{routing, Router};
use reqwest::Client;

#[tokio::test]
async fn bind() -> Result<()> {
	util::test(
		util::server,
		convert::identity,
		Router::new().route("/", routing::get(|| async { "test" })),
		|certificate, address| async move {
			let client = Client::builder()
				.add_root_certificate(certificate)
				.danger_accept_invalid_certs(true)
				.build()?;

			// HTTP.
			let response = client.get(format!("http://{address}")).send().await?;
			assert_eq!(response.text().await?, "test");

			// HTTPS.
			let response = client.get(format!("https://{address}")).send().await?;
			assert_eq!(response.text().await?, "test");

			Ok(())
		},
	)
	.await
}

#[tokio::test]
async fn from_tcp() -> Result<()> {
	util::test(
		|address, config| {
			// See <https://github.com/rust-lang/rust-clippy/issues/10011>.
			#[allow(clippy::unwrap_used)]
			let listener = TcpListener::bind(address).unwrap();
			axum_server_dual_protocol::from_tcp_dual_protocol(listener, config)
		},
		convert::identity,
		Router::new().route("/", routing::get(|| async { "test" })),
		|certificate, address| async move {
			let client = Client::builder()
				.add_root_certificate(certificate)
				.danger_accept_invalid_certs(true)
				.build()?;

			// HTTP.
			let response = client.get(format!("http://{address}")).send().await?;
			assert_eq!(response.text().await?, "test");

			// HTTPS.
			let response = client.get(format!("https://{address}")).send().await?;
			assert_eq!(response.text().await?, "test");

			Ok(())
		},
	)
	.await
}
