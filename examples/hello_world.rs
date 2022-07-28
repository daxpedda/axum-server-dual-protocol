//! This example demonstrates the use-case of wanting to host a HTTPS server
//! without a corresponding HTTP port to redirect HTTP requests from.
//!
//! Instead of hosting a HTTP server on a different port, this accepts both
//! protocols on the same port and upgrades all HTTP requests correctly to
//! HTTPS.
//!
//! You can try it by visiting <http://127.0.0.1:3000>. You can also observe
//! any path added to the URI will redirect to the corresponding HTTPS URI.
//! HTTPS requests should function as expected.

use std::net::SocketAddr;

use anyhow::Result;
use axum::{routing, Router};
use axum_server::tls_rustls::RustlsConfig;
use axum_server_dual_protocol::UpgradeHttpLayer;

#[tokio::main]
async fn main() -> Result<()> {
	let app = Router::new()
		.route("/", routing::get(|| async { "Hello, world!" }))
		.layer(UpgradeHttpLayer);

	let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
	println!("Listening on {}.", addr);
	println!(
		"Connecting to \"http://{}\" with any path or query will automatically redirect to \"https://{0}\" with the same path and query.",
		addr
	);

	let certificate = rcgen::generate_simple_self_signed([])?;
	let config = RustlsConfig::from_der(
		vec![certificate.serialize_der()?],
		certificate.serialize_private_key_der(),
	)
	.await?;

	axum_server_dual_protocol::bind_dual_protocol(addr, config)
		.serve(app.into_make_service())
		.await?;

	Ok(())
}
