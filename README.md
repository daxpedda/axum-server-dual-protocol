# axum-server-dual-protocol

[![Crates.io Version](https://img.shields.io/crates/v/axum-server-dual-protocol.svg)](https://crates.io/crates/axum-server-dual-protocol)
[![Live Build Status](https://img.shields.io/github/check-runs/daxpedda/axum-server-dual-protocol/main?label=CI)](https://github.com/daxpedda/axum-server-dual-protocol/actions?query=branch%3Amain)
[![Docs.rs Documentation](https://img.shields.io/docsrs/axum-server-dual-protocol?label=docs.rs)](https://docs.rs/crate/axum-server-dual-protocol)
[![Main Documentation](https://img.shields.io/github/actions/workflow/status/daxpedda/axum-server-dual-protocol/documentation.yml?branch=main&label=main%20docs)](https://daxpedda.github.io/axum-server-dual-protocol/axum_server_dual_protocol/index.html)

## Description

Provides utilities to host a [`axum-server`] server that accepts the HTTP and HTTPS protocol on the
same port. See [`bind_dual_protocol()`].

A common use case for this is if a HTTPS server is hosted on a non-traditional port, having no
corresponding HTTP port. This can be an issue for clients who try to connect over HTTP and get a
connection reset error. See [`ServerExt::set_upgrade()`].

## Usage

The simplest way to start is to use [`bind_dual_protocol()`]:

```rust
let app = Router::new().route(
	"/",
	routing::get(|request: Request<Body>| async move {
		match request.extensions().get::<Protocol>().unwrap() {
			Protocol::Tls => "Hello, secure World!",
			Protocol::Plain => "Hello, insecure World!",
		}
	}),
);

// User-supplied certificate and private key.
let config = RustlsConfig::from_der(certificate, private_key).await?;

axum_server_dual_protocol::bind_dual_protocol(address, config)
	.serve(app.into_make_service())
	.await?;
```

We now have a server accepting both HTTP and HTTPS requests! Now we can automatically upgrade
incoming HTTP requests to HTTPS using [`ServerExt::set_upgrade()`] like this:

```rust
use axum_server_dual_protocol::ServerExt;

axum_server_dual_protocol::bind_dual_protocol(address, config)
	.set_upgrade(true)
	.serve(app.into_make_service())
	.await?;
```

Alternatively [`UpgradeHttpLayer`] can be used:

```rust
let app = Router::new()
	.route("/", routing::get(|| async { "Hello, world!" }))
	.layer(UpgradeHttpLayer);
```

## Conditional Configurations

### `docsrs`

This requires Rust nightly and enhances the documentation. It must only be used with `RUSTDOCFLAGS`,
not with `RUSTFLAGS`.

## MSRV

As this library heavily relies on [`axum-server`], [`axum`], [`tower`] and [`hyper`] the MSRV
depends on theirs. At the point of time this was written the highest MSRV was [`axum`] with 1.66.

## Changelog

See the [CHANGELOG] file for details.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE] or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT] or <http://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the
work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.

[CHANGELOG]: https://github.com/daxpedda/axum-server-dual-protocol/blob/v0.6.0/CHANGELOG.md
[LICENSE-MIT]: https://github.com/daxpedda/axum-server-dual-protocol/blob/v0.6.0/LICENSE-MIT
[LICENSE-APACHE]: https://github.com/daxpedda/axum-server-dual-protocol/blob/v0.6.0/LICENSE-APACHE
[`axum`]: https://docs.rs/axum/0.7
[`axum-server`]: https://docs.rs/axum-server/0.6.0
[`bind_dual_protocol()`]:
	https://docs.rs/axum-server-dual-protocol/0.6.0/axum_server_dual_protocol/fn.bind_dual_protocol.html
[`hyper`]: https://docs.rs/hyper/1
[`Layer`]: https://docs.rs/tower-layer/0.3/tower_layer/trait.Layer.html
[`Router`]: https://docs.rs/axum/0.7/axum/struct.Router.html
[`ServerExt::set_upgrade()`]:
	https://docs.rs/axum-server-dual-protocol/0.6.0/axum_server_dual_protocol/trait.ServerExt.html#tymethod.set_upgrade
[`tower`]: https://docs.rs/tower/0.4
[`UpgradeHttpLayer`]:
	https://docs.rs/axum-server-dual-protocol/0.6.0/axum_server_dual_protocol/struct.UpgradeHttpLayer.html
