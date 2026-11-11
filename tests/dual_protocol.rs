//! Tests for the dual-protocol acceptor.

mod util;

use std::convert;
use std::net::TcpListener;
use std::sync::Arc;

use anyhow::Result;
use axum::{routing, Router};
use axum_server::tls::TlsInfo;
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
			let listener = TcpListener::bind(address)?;
			Ok(axum_server_dual_protocol::from_tcp_dual_protocol(
				listener, config,
			)?)
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
async fn tls_info() -> Result<()> {
	util::test(
		util::server,
		convert::identity,
		Router::new().route(
			"/",
			routing::get(|extensions: Extensions| async move {
				if extensions.get::<Arc<TlsInfo>>().is_some() {
					"secure"
				} else {
					"insecure"
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
