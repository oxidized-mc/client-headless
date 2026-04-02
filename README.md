# HeadlessCraft

> A Rust framework for building headless Minecraft Java Edition clients — bots, testing tools, and automation.

[![CI](https://github.com/dodoflix/HeadlessCraft/actions/workflows/ci.yml/badge.svg)](https://github.com/dodoflix/HeadlessCraft/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE-MIT)

---

## What is HeadlessCraft?

HeadlessCraft is a Rust library that emulates a Minecraft Java Edition client without rendering.
It connects to vanilla servers, handles the full protocol lifecycle (handshake → login → configuration → play),
and exposes a high-level API for building:

- **Bots** — automated players that can navigate, interact, chat, and follow complex behaviors
- **Testing tools** — connect to your server and verify behavior programmatically
- **Stress testers** — spawn hundreds of headless clients to load-test servers
- **Protocol analyzers** — inspect and log every packet in both directions
- **Proxy frameworks** — sit between a real client and server, intercepting traffic

## Status

**Pre-alpha** — project scaffolding and planning phase. No functional client yet.

## Target Protocol

- **Minecraft Java Edition 26.1** (protocol version `775`, world version `4786`)
- **Reference:** Decompiled vanilla 26.1 JAR in `mc-server-ref/decompiled/` (gitignored)

## Architecture

HeadlessCraft is a Cargo workspace with 3 crates:

```
headlesscraft-macros    ← no internal deps (proc-macros)
headlesscraft-protocol  ← macros (packets, codecs, NBT, types, wire format)
headlesscraft           ← protocol, macros (client logic, world state, bot API)
```

Lower-layer crates never depend on higher-layer crates.

Modules within `headlesscraft` (the main crate) handle:
- **client** — connection management, authentication, session handling
- **world** — client-side world state (chunks, entities, biomes)
- **bot** — high-level bot behavior API and event system

Modules within `headlesscraft-protocol` handle:
- **packets** — all packet definitions for every connection state
- **codec** — VarInt/VarLong, framing, encryption, compression
- **nbt** — Named Binary Tag serialization
- **types** — shared coordinate types, block IDs, protocol primitives

## Quick Start

> Requires **Rust 1.85+** (pinned in `rust-toolchain.toml`).

```bash
# Clone
git clone https://github.com/dodoflix/HeadlessCraft.git
cd HeadlessCraft

# Build
cargo build

# Run tests
cargo test --workspace

# Check lints
cargo clippy --workspace --all-targets -- -D warnings
```

### Use as a dependency

```toml
[dependencies]
headlesscraft = "0.1"
```

```rust
use headlesscraft::Client;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = Client::builder()
        .address("localhost:25565")
        .username("Bot")
        .build()
        .await?;

    client.connect().await?;
    // ... interact with the server
    Ok(())
}
```

## Goals

| Goal | Description |
|------|-------------|
| **Protocol fidelity** | Wire-compatible with vanilla 26.1 servers — every packet, every field, every edge case |
| **Ergonomic API** | High-level builder patterns for common tasks, low-level access when needed |
| **Performance** | Async-first with Tokio, zero-copy where possible, support hundreds of concurrent clients |
| **Testability** | First-class support for server testing — assert on game state, packet sequences, timing |
| **Extensibility** | Plugin/event system for custom bot behaviors without forking |

## Documentation

- [Contributing](CONTRIBUTING.md)

## License

Licensed under either of:

- [Apache License, Version 2.0](LICENSE-APACHE)
- [MIT License](LICENSE-MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
