# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased
### Added
- Introduced [`Protocol`](https://daxpedda.github.io/axum-server-dual-protocol/axum_server_dual_protocol/struct.Protocol.html).
  Which can be used with [`Request::extensions()`](https://docs.rs/http/0.2.9/http/request/struct.Request.html#method.extensions)
  to extract this connections protocol.

### Fixed
- Secure WebSocket handshakes are now accepted, instead of redirected to the `https` URI scheme.

## [0.4.0] - 2023-05-04
### Changed
- Updated `axum-server` to v0.5.
- Updated `tokio-rustls` to v0.24.

## [0.3.0] - 2022-11-26
### Added
- Implemented `from_tcp_dual_protocol()`, equivalent to
  [`axum_server::from_tcp`](https://docs.rs/axum-server/0.4.4/axum_server/fn.from_tcp_rustls.html).

### Changed
- Updated `axum` to v0.6.
- Increased MSRV to v1.60.

## [0.2.0] - 2022-07-30
### Added
- Implemented `ServerExt::set_upgrade()`, an easy way to apply `UpgradeHttpLayer`
  to the entire app.

### Changed
- Renamed `DualProtocolFuture` to `DualProtocolAcceptorFuture`.

## [0.1.0] - 2022-07-28
### Added
- Initial commit.

[Unreleased]: https://github.com/daxpedda/axum-server-dual-protocol/compare/v0.4.0...main
[0.4.0]: https://github.com/daxpedda/axum-server-dual-protocol/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/daxpedda/axum-server-dual-protocol/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/daxpedda/axum-server-dual-protocol/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/daxpedda/axum-server-dual-protocol/releases/tag/v0.1.0
