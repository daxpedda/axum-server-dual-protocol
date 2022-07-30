# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]
### Added
- Implement `ServerExt::set_upgrade()`, an easy way to apply `UpgradeHttpLayer`
  to the entire app.

### Changed
- Renamed `DualProtocolFuture` to `DualProtocolAcceptorFuture`.

## [0.1.0] - 2022-07-28
### Added
- Initial commit.

[Unreleased]: https://github.com/daxpedda/axum-server-dual-protocol/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/daxpedda/axum-server-dual-protocol/releases/tag/v0.1.0
