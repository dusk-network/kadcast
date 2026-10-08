# AGENTS.md — kadcast

## Overview

Kademlia-based P2P broadcast protocol for Dusk. UDP transport with optional RaptorQ forward error correction. See `ARCHITECTURE.md` for a detailed component diagram and description.

## Key Directories

| Path | Purpose |
|------|---------|
| `src/lib.rs` | Main orchestration and public API |
| `src/kbucket.rs` | K-bucket routing tree (core Kademlia DHT structure) |
| `src/kbucket/bucket.rs` | Individual bucket operations |
| `src/kbucket/key.rs` | Binary key (128-bit peer IDs via BLAKE2) |
| `src/transport.rs` | UDP transport layer (Tokio-based) |
| `src/transport/encoding/` | Plain and RaptorQ message encoding |
| `src/encoding.rs` | Binary marshalling (`Marshallable` trait) |
| `src/handling.rs` | Message processing logic |
| `src/maintainer.rs` | Bucket maintenance tasks |
| `src/config.rs` | Configuration and defaults |
| `src/peer.rs` | Peer/node information |
| `examples/` | Usage examples |

## Commands

```bash
make test      # Run tests (cargo test --release)
make clippy    # Run clippy (--all-features --release)
make fmt       # Format code (requires nightly)
make doc       # Generate documentation
make clean     # Clean build artifacts
make help      # Show all targets
```

## Architecture

The library is structured around four main components:

- **Kadcast Peer** (`src/peer.rs`): Entry point and only public API. Implements the Kadcast network peer.
- **Message Handler** (`src/handling.rs`): Core component that handles incoming messages and replies per the Kadcast specification.
- **Maintainer** (`src/maintainer.rs`): Performs initial bootstrap and k-table maintenance. Tracks idle buckets and triggers routing node lookups.
- **Wire Network** (`src/transport.rs`): UDP server/client for network I/O. Handles serialization/deserialization and applies FEC encoding/decoding.

Internal communication uses three async channels:
- **Inbound**: UDP packets (deserialized) → Message Handler
- **Outbound**: Any component → network transmission
- **Notification**: Decoded broadcast messages → user callback

## Feature Flags

- `raptorq` (default) — RaptorQ forward error correction
- `diagnostics` — additional diagnostic logging

## Conventions

- No `std`-incompatible patterns — but this is not a `no_std` crate
- Use `tracing` for logging, never `println!`
- No `unwrap()`/`expect()` outside of tests — return errors instead
- No `#[allow(...)]` lint suppression — fix the underlying issue
- Run `make fmt` before committing (requires nightly toolchain)
- Run `make clippy` to check for warnings

## Change Propagation

Kadcast is used by `rusk/node`. Changes here should be verified against Rusk's node component.

| Changed | Also verify |
|---------|-------------|
| `kadcast` | `rusk/node` |

## Changelog

Add an entry to `CHANGELOG.md` under `[Unreleased]` only for a change that users of the crate can see. Tests, CI, tooling and internal refactors get no entry.

- Write one fact per entry, in one sentence. Two facts get two entries.
- Name what changed at the crate's surface: the public item and its new behavior. Do not describe how the code does it.
- Name the released item that a breaking change breaks.
- Keep the reason, the consequences and the migration steps in the linked issue, not in the entry.
- Choose the section by the effect on users: new things go under `Added`, changed behavior goes under `Changed`, and removed things go under `Removed`. Use `Fixed` only for a bug that a release had. For a bug in unreleased code, correct the entry that added that code.
- Link the tracking GitHub issue, not the PR, and no other tracking identifier. Use the link format that the file already uses. A reference link needs its definition in the block at the bottom.
- Add to the existing section headings, and leave the other entries as they are.
- Use the [Keep a Changelog](https://keepachangelog.com/en/1.0.0/) format.
- Follow standard markdown formatting: separate headings from surrounding content with blank lines, leave a blank line before and after lists, and never have two headings back-to-back without a blank line between them.

## Git Conventions

- Keep commits small and focused
