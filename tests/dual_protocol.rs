#![allow(missing_docs, reason = "Test should speak for itself")]

mod util;

use std::convert;
use std::net::TcpListener;

use anyhow::Result;
use axum::{routing, Router};
use axum_server_dual_protocol::Protocol;
use http::Extensions;
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
			let listener = TcpListener::bind(address).unwrap();
			//See github.com/tokio-rs/tokio/issues/7172 for details
			listener.set_nonblocking(true).unwrap();
			axum_server_dual_protocol::from_tcp_dual_protocol(listener, config).unwrap()
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

#[tokio::test]
async fn protocol() -> Result<()> {
	util::test(
		util::server,
		convert::identity,
		Router::new().route(
			"/",
			routing::get(|extensions: Extensions| async move {
				match extensions.get::<Protocol>().unwrap() {
					Protocol::Tls => "secure",
					Protocol::Plain => "insecure",
				}
			}),
		),
		|certificate, address| async move {
			let client = Client::builder()
				.add_root_certificate(certificate)
				.danger_accept_invalid_certs(true)
				.build()?;

			// HTTP.
			let response = client.get(format!("http://{address}")).send().await?;
			assert_eq!(response.text().await?, "insecure");

			// HTTPS.
			let response = client.get(format!("https://{address}")).send().await?;
			assert_eq!(response.text().await?, "secure");

			Ok(())
		},
	)
	.await
}
