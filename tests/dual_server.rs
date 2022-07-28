mod util;

use anyhow::Result;
use axum::{routing, Router};
use reqwest::Client;

#[tokio::test]
async fn main() -> Result<()> {
    util::test(
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
