# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

### Added

- Add `auto_propagate` flag to Peer [#57]

### Changed

- Change `on_message` callback into a trait [#63]

### Fixed

- Fix build with `tonic` dependency [#60]

### Removed

## [0.1.0] - 29-10-21

### Added

- Kadcast Network protocol implementation
- RaptorQ as Forward Error Correction.
- Examples in `example` dir

[#60]: https://github.com/dusk-network/kadcast/issues/60
[#60]: https://github.com/dusk-network/kadcast/issues/63
