# axum-server-dual-protocol

[![Crates.io Version](https://img.shields.io/crates/v/axum-server-dual-protocol.svg)](https://crates.io/crates/axum-server-dual-protocol)
[![Live Build Status](https://img.shields.io/github/checks-status/daxpedda/axum-server-dual-protocol/main?label=CI)](https://github.com/daxpedda/axum-server-dual-protocol/actions?query=branch%3Amain)
[![Docs.rs Documentation](https://img.shields.io/docsrs/axum-server-dual-protocol)](https://docs.rs/crate/axum-server-dual-protocol)
[![Master Documentation](https://img.shields.io/github/workflow/status/daxpedda/axum-server-dual-protocol/Documentation?label=master%20docs)](https://daxpedda.github.io/axum-server-dual-protocol/axum_server_dual_protocol/index.html)

## Description

Provides utilities to host a [`axum-server`] server that
accepts the HTTP and HTTPS protocol on the same port. See
[`bind_dual_protocol`].

A common use case for this is if a HTTPS server is hosted on a
non-traditional port, having no corresponding HTTP port. This can be an
issue for clients who try to connect over HTTP and get a connection reset
error. For this specific purpose a [`Layer`] is provided
that automatically upgrades any connection to HTTPS. See
[`UpgradeHttpLayer`].

## Usage

The simplest way to start is to use [`bind_dual_protocol`]:
```rust
let app = Router::new().route("/", routing::get(|| async { "Hello, world!" }));

// User-supplied certificate and private key.
let config = RustlsConfig::from_der(certificate, private_key).await?;

axum_server_dual_protocol::bind_dual_protocol(address, config)
	.serve(app.into_make_service())
	.await?;
```

We now have a server accepting both HTTP and HTTPS requests! To use
[`UpgradeHttpLayer`] we can simply add it to the [`Router`]:
```rust
let app = Router::new()
	.route("/", routing::get(|| async { "Hello, world!" }))
	.layer(UpgradeHttpLayer);
```

## MSRV

As this library heavily relies on [`axum-server`], [`axum`],
[`tower`] and [`hyper`] the MSRV depends on theirs. At the point of time
this was written the highest MSRV was [`axum`] with 1.56.

## Changelog

See the [CHANGELOG] file for details.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE] or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT] or <http://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.

[CHANGELOG]: https://github.com/daxpedda/axum-server-dual-protocol/blob/main/CHANGELOG.md
[LICENSE-MIT]: https://github.com/daxpedda/axum-server-dual-protocol/blob/main/LICENSE-MIT
[LICENSE-APACHE]: https://github.com/daxpedda/axum-server-dual-protocol/blob/main/LICENSE-APACHE
[`axum`]: https://docs.rs/axum/latest/axum
[`axum-server`]: https://docs.rs/axum-server/latest/axum-server
[`bind_dual_protocol`]: https://docs.rs/axum-server-dual-protocol/latest/axum-server-dual-protocol/fn.bind_dual_protocol.html
[`hyper`]: https://docs.rs/hyper/latest/hyper
[`Layer`]: https://docs.rs/tower-layer/latest/tower_layer/trait.Layer.html
[`Router`]: https://docs.rs/axum/latest/axum/struct.Router.html
[`tower`]: https://docs.rs/tower/latest/tower
[`UpgradeHttpLayer`]: https://docs.rs/axum-server-dual-protocol/latest/axum-server-dual-protocol/struct.UpgradeHttpLayer.html
