# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Add `max_udp_len` configuration parameter
- Add range checks to MTU (between 1296 and 8192)
- Add network version to handshake messages
- Add Ray-ID to MessageInfo for message tracking

### Fixed

- Fix raptorQ cache default config
- Fix ObjectTransmissionInformation deserialization

### Changed

- Change the EncodedChunk UUID generation (aka RaptorqHeader)
- Change `raptorq` dependency from `1.6` to `2.0`

## [0.6.1] - 2024-04-10

### Added

- Add `BucketConfig::min_peers` in configuration [#135]

### Changed

- Change 'need_bootstrappers' to have dinamically threshold [#135]
- Change `BinaryID::from_nonce` to return result [#136]
- Change maintainer to ping nodes while removal [#138]

## [0.6.0] - 2023-11-01

### Added

- Add new facade function `new` to creating `RwLock` based on feature flag [#94]
- Add `NetworkId` in configuration [#123]

### Changed

- Change `RwLock` API to support diagnostics feature flag [#94]
- Change network wire encoding to support `NetworkId` [#123]

## [0.5.0] - 2023-05-17

### Added
- Add network blocklist implementation [#117]

### Changed 
- Change `Peer::new` to return a Result [#115]
- Change `blake2` dependency from `0.9` to `0.10` [#115]

## [0.4.1] - 2022-07-27

### Added
- Remove idle nodes during bucket mantainance [#108]

### Fixed
- Use provided nonce instead of regenerate it [#110]
- Network bootstrap after being disconnected [#112]

## [0.4.0] - 2022-07-06
### Added

- Add `kadcast::Config` [#96]
- Add `Peer::alive_nodes(amount)` to return random alive socket_addr [#103]

### Removed

- `PeerBuilder` in favor of `Peer::new()`

### Fixed

- Stalled peer bootstrap [#97] [#99]
- Dupemap cache expiring [#101]

## [0.3.0] - 2022-01-07
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

## [0.2.0] - 2021-12-16

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

## [0.1.0] - 2021-10-29

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
[#94]: https://github.com/dusk-network/kadcast/issues/94
[#96]: https://github.com/dusk-network/kadcast/issues/96
[#97]: https://github.com/dusk-network/kadcast/issues/97
[#99]: https://github.com/dusk-network/kadcast/issues/99
[#101]: https://github.com/dusk-network/kadcast/issues/101
[#103]: https://github.com/dusk-network/kadcast/issues/103
[#108]: https://github.com/dusk-network/kadcast/issues/108
[#110]: https://github.com/dusk-network/kadcast/issues/110
[#112]: https://github.com/dusk-network/kadcast/issues/112
[#115]: https://github.com/dusk-network/kadcast/issues/115
[#117]: https://github.com/dusk-network/kadcast/issues/117
[#123]: https://github.com/dusk-network/kadcast/issues/123
[#135]: https://github.com/dusk-network/kadcast/issues/135
[#136]: https://github.com/dusk-network/kadcast/issues/136
[#138]: https://github.com/dusk-network/kadcast/issues/138

<!-- Releases -->

[Unreleased]: https://github.com/dusk-network/kadcast/compare/v0.6.1...HEAD
[0.6.1]: https://github.com/dusk-network/kadcast/compare/v0.6.0...v0.6.1
[0.6.0]: https://github.com/dusk-network/kadcast/compare/v0.5.0...v0.6.0
[0.5.1]: https://github.com/dusk-network/kadcast/compare/v0.4.1...v0.5.0
[0.4.1]: https://github.com/dusk-network/kadcast/compare/v0.4.0...v0.4.1
[0.4.0]: https://github.com/dusk-network/kadcast/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/dusk-network/kadcast/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/dusk-network/kadcast/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/dusk-network/kadcast/releases/tag/v0.1.0
