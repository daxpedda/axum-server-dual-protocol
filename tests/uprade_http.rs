// This is already fixed in newer versions of Clippy:
// <https://github.com/rust-lang/rust-clippy/pull/8802>.
#![allow(clippy::unwrap_used)]

mod util;

use anyhow::Result;
use axum::{routing, Router};
use axum_server_dual_protocol::UpgradeHttpLayer;
use http::header::LOCATION;
use reqwest::redirect::Policy;
use reqwest::{Client, StatusCode};

#[tokio::test]
async fn main() -> Result<()> {
	util::test(
		Router::new()
			.route("/", routing::get(|| async { "test" }))
			.layer(UpgradeHttpLayer),
		|certificate, address| async move {
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
		},
	)
	.await
}
