//! Run with `cargo run --example hello_world` command.
//!
//! To connect through browser, navigate to "http://localhost:3000" url.

use anyhow::Result;
use axum::{routing::get, Router};
use axum_server::tls_rustls::RustlsConfig;
use axum_server_upgrade_http::UpgradeHttpLayer;
use std::net::SocketAddr;

#[tokio::main]
async fn main() -> Result<()> {
    let app = Router::new()
        .route("/", get(|| async { "Hello, world!" }))
        .layer(UpgradeHttpLayer);

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("Listening on {}.", addr);
    println!(
        "Connecting to \"http://{}\" with any path or query will automatically redirect to \"https://{0}\".",
        addr
    );

    let certificate = rcgen::generate_simple_self_signed([])?;
    let config = RustlsConfig::from_der(
        vec![certificate.serialize_der()?],
        certificate.serialize_private_key_der(),
    )
    .await?;

    axum_server_upgrade_http::bind_dual(addr, config)
        .serve(app.into_make_service())
        .await
        .unwrap();

    Ok(())
}
