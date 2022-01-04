# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

### Added

- Add network transport configuration [#72] [#76]
- Add recursive NetworkDiscovery configuration [#78]
- Add internal channel capacity configuration [#78]

## [0.2.0] - 16-12-21

### Added

- Add `auto_propagate` flag to Peer [#57]
- Add optional `height` parameter to `broadcast` method [#57]
- Add `send` method to public API [#58]
- Add metadata to `on_message` callback [#59]
- Add `listen_address` parameter [#69]
- Add the auto prune of expired items in RaptorQ cache [#68]

### Changed

- Change `on_message` callback into a trait [#63]

### Fixed

- Fix build with `tonic` dependency [#60]

## [0.1.0] - 29-10-21

### Added

- Kadcast Network protocol implementation
- RaptorQ as Forward Error Correction.
- Examples in `example` dir

[#57]: https://github.com/dusk-network/kadcast/issues/57
[#58]: https://github.com/dusk-network/kadcast/issues/58
[#59]: https://github.com/dusk-network/kadcast/issues/59
[#60]: https://github.com/dusk-network/kadcast/issues/60
[#63]: https://github.com/dusk-network/kadcast/issues/63
[#68]: https://github.com/dusk-network/kadcast/issues/68
[#69]: https://github.com/dusk-network/kadcast/issues/69
[#72]: https://github.com/dusk-network/kadcast/issues/72
[#76]: https://github.com/dusk-network/kadcast/issues/76
[#78]: https://github.com/dusk-network/kadcast/issues/78
