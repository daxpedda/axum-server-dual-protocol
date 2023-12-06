#![cfg(test)]
#![allow(clippy::missing_assert_message)]

mod util;

use std::convert;
use std::net::SocketAddr;

use anyhow::Result;
use axum::{routing, Router};
use axum_server_dual_protocol::{ServerExt, UpgradeHttpLayer};
use http_0::header::LOCATION;
use reqwest::redirect::Policy;
use reqwest::{Certificate, Client, StatusCode};

#[tokio::test]
async fn router() -> Result<()> {
	util::test(
		util::server,
		convert::identity,
		Router::new()
			.route("/", routing::get(|| async { "test" }))
			.layer(UpgradeHttpLayer),
		test,
	)
	.await
}

#[tokio::test]
async fn acceptor() -> Result<()> {
	util::test(
		util::server,
		|mut server| {
			server.get_mut().set_upgrade(true);
			server
		},
		Router::new().route("/", routing::get(|| async { "test" })),
		test,
	)
	.await
}

#[tokio::test]
async fn server() -> Result<()> {
	util::test(
		util::server,
		|server| server.set_upgrade(true),
		Router::new().route("/", routing::get(|| async { "test" })),
		test,
	)
	.await
}

async fn test(certificate: Certificate, address: SocketAddr) -> Result<()> {
	let client = Client::builder()
		.add_root_certificate(certificate)
		.danger_accept_invalid_certs(true)
		.redirect(Policy::none())
		.build()?;

	// HTTP index.
	let response = client.get(format!("http://{address}")).send().await?;
	assert_eq!(response.status(), StatusCode::MOVED_PERMANENTLY);
	assert_eq!(
		*response.headers().get(LOCATION).unwrap(),
		format!("https://{address}/")
	);
	assert_eq!(response.text().await?, "");

	// HTTP not-existing path.
	let response = client.get(format!("http://{address}/test")).send().await?;
	assert_eq!(response.status(), StatusCode::MOVED_PERMANENTLY);
	assert_eq!(
		*response.headers().get(LOCATION).unwrap(),
		format!("https://{address}/test")
	);
	assert_eq!(response.text().await?, "");

	// HTTPS index.
	let response = client.get(format!("https://{address}")).send().await?;
	assert_eq!(response.text().await?, "test");

	// HTTPS not-existing path.
	let response = client.get(format!("https://{address}/test")).send().await?;
	assert_eq!(response.status(), StatusCode::NOT_FOUND);
	assert_eq!(response.text().await?, "");

	Ok(())
}
