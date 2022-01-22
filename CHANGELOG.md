# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

### Added

- Add `kadcast::Config` [#96]

### Removed

- `PeerBuilder` in favor of `Peer::new()`

### Fixed

- Stalled peer bootstrap [#97] [#99]

## [0.3.0] - 07-01-22
### Added

- Add network transport configuration [#72] [#76]
- Add recursive NetworkDiscovery configuration [#78]
- Add internal channel capacity configuration [#78]
- Add configurable FEC redundancy [#82]
- Add configurable UDP send interval [#83]
- Add UDP network tweak configuration [#86]
- Add dedicated tokio task to handle and decode chunks [#87]
- Add logs to pending RwLock [#92]

### Fixed

- Deadlock in `RWLock.write()` [#80]
- Preserve propagation in some edge-corner cases
- Messages from buckets full are correctly handled 
- Empty payload NodesMessage decoding [#90]

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
[#80]: https://github.com/dusk-network/kadcast/issues/80
[#82]: https://github.com/dusk-network/kadcast/issues/82
[#83]: https://github.com/dusk-network/kadcast/issues/83
[#87]: https://github.com/dusk-network/kadcast/issues/87
[#90]: https://github.com/dusk-network/kadcast/issues/90
[#92]: https://github.com/dusk-network/kadcast/issues/92
[#96]: https://github.com/dusk-network/kadcast/issues/96
[#97]: https://github.com/dusk-network/kadcast/issues/97
[#99]: https://github.com/dusk-network/kadcast/issues/99
