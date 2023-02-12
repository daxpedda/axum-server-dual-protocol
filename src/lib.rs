//! # Description
//!
//! Provides utilities to host a [`axum-server`](axum_server) server that
//! accepts the HTTP and HTTPS protocol on the same port. See
//! [`bind_dual_protocol()`].
//!
//! A common use case for this is if a HTTPS server is hosted on a
//! non-traditional port, having no corresponding HTTP port. This can be an
//! issue for clients who try to connect over HTTP and get a connection reset
//! error. See [`ServerExt::set_upgrade()`].
//!
//! # Usage
//!
//! The simplest way to start is to use [`bind_dual_protocol()`]:
//! ```no_run
//! # use axum::{routing, Router};
//! # use axum_server::tls_rustls::RustlsConfig;
//! #
//! # #[tokio::main]
//! # async fn main() -> anyhow::Result<()> {
//! let app = Router::new().route("/", routing::get(|| async { "Hello, world!" }));
//!
//! # let address = std::net::SocketAddr::from(([127, 0, 0, 1], 0));
//! # let certificate = rcgen::generate_simple_self_signed([])?;
//! # let private_key = certificate.serialize_private_key_der();
//! # let certificate = vec![certificate.serialize_der()?];
//! #
//! // User-supplied certificate and private key.
//! let config = RustlsConfig::from_der(certificate, private_key).await?;
//!
//! axum_server_dual_protocol::bind_dual_protocol(address, config)
//! 	.serve(app.into_make_service())
//! 	.await?;
//! #
//! # Ok(())
//! # }
//! ```
//!
//! We now have a server accepting both HTTP and HTTPS requests! Now we can
//! automatically upgrade incoming HTTP requests to HTTPS using
//! [`ServerExt::set_upgrade()`] like this:
//! ```no_run
//! # use axum::{routing, Router};
//! # use axum_server::tls_rustls::RustlsConfig;
//! use axum_server_dual_protocol::ServerExt;
//!
//! # #[tokio::main]
//! # async fn main() -> anyhow::Result<()> {
//! # let app = Router::new();
//! # let address = std::net::SocketAddr::from(([127, 0, 0, 1], 0));
//! # let certificate = rcgen::generate_simple_self_signed([])?;
//! # let private_key = certificate.serialize_private_key_der();
//! # let certificate = vec![certificate.serialize_der()?];
//! # let config = RustlsConfig::from_der(certificate, private_key).await?;
//! #
//! axum_server_dual_protocol::bind_dual_protocol(address, config)
//! 	.set_upgrade(true)
//! 	.serve(app.into_make_service())
//! 	.await?;
//! #
//! # Ok(())
//! # }
//! ```
//!
//! Alternatively [`UpgradeHttpLayer`] can be used:
//! ```
//! # use axum::{routing, Router};
//! # use axum_server_dual_protocol::UpgradeHttpLayer;
//! let app = Router::new()
//! 	.route("/", routing::get(|| async { "Hello, world!" }))
//! 	.layer(UpgradeHttpLayer);
//! # // To help with type inference.
//! # axum_server::bind(std::net::SocketAddr::from(([127, 0, 0, 1], 0)))
//! # 	.serve(app.into_make_service());
//! ```
//!
//! # MSRV
//!
//! As this library heavily relies on [`axum-server`](axum_server), [`axum`],
//! [`tower`] and [`hyper`] the MSRV depends on theirs. At the point of time
//! this was written the highest MSRV was [`axum`] with 1.60.
//!
//! # Changelog
//!
//! See the [CHANGELOG] file for details.
//!
//! # License
//!
//! Licensed under either of
//!
//! - Apache License, Version 2.0 ([LICENSE-APACHE] or <http://www.apache.org/licenses/LICENSE-2.0>)
//! - MIT license ([LICENSE-MIT] or <http://opensource.org/licenses/MIT>)
//!
//! at your option.
//!
//! ## Contribution
//!
//! Unless you explicitly state otherwise, any contribution intentionally
//! submitted for inclusion in the work by you, as defined in the Apache-2.0
//! license, shall be dual licensed as above, without any additional terms or
//! conditions.
//!
//! [CHANGELOG]: https://github.com/daxpedda/axum-server-dual-protocol/blob/v0.3.0/CHANGELOG.md
//! [LICENSE-MIT]: https://github.com/daxpedda/axum-server-dual-protocol/blob/v0.3.0/LICENSE-MIT
//! [LICENSE-APACHE]: https://github.com/daxpedda/axum-server-dual-protocol/blob/v0.3.0/LICENSE-APACHE
//! [`axum`]: https://docs.rs/axum/0.6
//! [`Router`]: https://docs.rs/axum/0.6/axum/struct.Router.html
//! [`tower`]: https://docs.rs/tower/0.4

mod dual_protocol;
mod either;
mod upgrade_http;

pub use dual_protocol::{
	bind_dual_protocol, from_tcp_dual_protocol, DualProtocolAcceptor, DualProtocolAcceptorFuture,
	DualProtocolService, DualProtocolServiceFuture, ServerExt,
};
pub use either::Either;
pub use upgrade_http::{UpgradeHttp, UpgradeHttpFuture, UpgradeHttpLayer};
