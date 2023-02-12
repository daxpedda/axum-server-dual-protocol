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
use axum_server_dual_protocol::ServerExt;

#[tokio::main]
async fn main() -> Result<()> {
	let app = Router::new().route("/", routing::get(|| async { "Hello, world!" }));

	let address = SocketAddr::from(([127, 0, 0, 1], 3000));
	println!("Listening on {address}.");
	println!(
		"Connecting to \"http://{address}\" with any path or query will automatically redirect to \"https://{address}\" with the same path and query."
	);

	let certificate = rcgen::generate_simple_self_signed([])?;
	let config = RustlsConfig::from_der(
		vec![certificate.serialize_der()?],
		certificate.serialize_private_key_der(),
	)
	.await?;

	axum_server_dual_protocol::bind_dual_protocol(address, config)
		.set_upgrade(true)
		.serve(app.into_make_service())
		.await?;

	Ok(())
}
