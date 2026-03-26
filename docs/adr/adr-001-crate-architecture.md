# ADR-001: Crate Architecture

| Field | Value |
|-------|-------|
| Status | Accepted |
| Date | 2026-03-26 |
| Phases | P01 |
| Deciders | HeadlessCraft Core Team |

## Context

HeadlessCraft is a Rust library for building headless Minecraft Java Edition clients — bots, load-testing harnesses, protocol analyzers, and automation tools. Unlike a server project (such as Oxidized, our sister project, which uses 7 crates), a client library has fundamentally different decomposition needs. The server must separate world storage, game simulation, and networking because each is large and independently complex. A client library, by contrast, has no world generation, no tick loop orchestration, and no server-side entity simulation — it *receives* world state and *sends* player actions.

The primary tension is between usability and modularity. Downstream users fall into two camps: (1) bot/automation developers who want a high-level `Client` type that connects, authenticates, and provides a world view, and (2) tool developers building proxies, packet sniffers, or protocol analyzers who only need the wire format — packets, codecs, and types — without any client logic. Forcing the second group to pull in the full client library (with Tokio, authentication, world state tracking) is wasteful and creates unnecessary coupling.

Rust's proc-macro crate rules add a hard constraint: any derive macros must live in a dedicated crate with `proc-macro = true` in its `Cargo.toml`. This crate cannot export non-macro items and cannot depend on the crates that use its macros. This is a compiler-level requirement, not a style choice.

## Decision Drivers

- **Standalone protocol usability**: proxy and analyzer tools must be able to depend on just the protocol crate without pulling in Tokio, authentication, or world state tracking
- **Minimal dependency footprint for downstream**: users pay only for what they use — a protocol-only consumer should not transitively depend on `reqwest` (auth) or `dashmap` (world state)
- **Clear layer boundaries**: the protocol layer knows nothing about client logic; the client layer knows nothing about macro implementation
- **Proc-macro isolation**: Rust requires proc-macro crates to be separate compilation units with no non-macro exports
- **Ergonomic re-exports**: bot developers should be able to `use headlesscraft::prelude::*` and get everything they need without manually importing from sub-crates
- **Independent testability**: `cargo test -p headlesscraft-protocol` runs protocol tests without building the client

## Considered Options

### Option 1: Single monolith crate

Put everything — macros, protocol, client logic — in one crate. Use `mod protocol;`, `mod client;`, etc. for internal organization. This is simple to set up: no cross-crate `pub` API design, no workspace configuration, straightforward imports. However, it violates the proc-macro rule (macros cannot live in a non-proc-macro crate), so we'd need to either abandon derive macros or use an awkward `macro_rules!` workaround. Worse, every downstream consumer pays the full dependency cost — a packet sniffer that only needs `VarInt` decoding would transitively pull in Tokio, reqwest, and the entire world state module. There's also no compile-time enforcement of layer boundaries — nothing prevents the protocol module from importing client types.

### Option 2: 3-layer architecture (macros → protocol → client)

Three crates in a strict DAG: `headlesscraft-macros` (proc-macro leaf), `headlesscraft-protocol` (wire format, depends on macros), and `headlesscraft` (client logic, depends on protocol + macros). The protocol crate is independently publishable and usable — proxy developers add `headlesscraft-protocol` to their `Cargo.toml` and get packets, codecs, VarInt, NBT, and types with no client baggage. The main `headlesscraft` crate re-exports protocol types for convenience, so bot developers never need to name the sub-crates. Macro isolation is satisfied by design.

### Option 3: Fine-grained 6+ crate design (Oxidized-style)

Mirror Oxidized's decomposition: separate crates for NBT, types, macros, protocol, world-state, and client. This maximizes boundary enforcement — NBT is its own leaf, types are shared, world state is isolated. However, for a client library, the additional boundaries add friction without proportional benefit. The client's NBT usage is confined to parsing server-sent data (chunk sections, entity metadata), not an independent concern worth its own crate. The world-state module is tightly coupled to protocol (it updates from incoming packets), making a hard boundary awkward. Six crates also increase cognitive load for contributors and make dependency management more complex for downstream users who must pick from many crate names.

## Decision

**We adopt a 3-crate Cargo workspace.** The crates and their responsibilities are:

```
headlesscraft-macros       (proc-macro leaf — no internal deps)
        ↑
headlesscraft-protocol     (wire format, packets, codecs, NBT, types)
        ↑
headlesscraft              (client logic, world state, bot API)
```

### Crate Responsibilities

**`headlesscraft-macros`** — Proc-macro crate
- Derive macros for packet serialization: `#[derive(Encode, Decode)]`
- Attribute macros for packet registration: `#[packet(id = 0x00, state = Handshaking)]`
- No internal dependencies — proc-macro crates are leaf nodes by Rust's rules
- Minimal external dependencies (only `syn`, `quote`, `proc-macro2`)

**`headlesscraft-protocol`** — Minecraft protocol library (standalone-usable)
- All ~300 packet structs across 5 protocol states (Handshaking, Status, Login, Configuration, Play)
- Wire types: `VarInt`, `VarLong`, `McString`, `Position`, `Angle`, `BitSet`, `Identifier`
- Packet codec: frame splitting, compression, encryption transform traits
- NBT serialization/deserialization (integrated, not a separate crate — client NBT usage doesn't warrant isolation)
- Packet registry: `(State, Direction, PacketId) → decode function`
- Shared coordinate types: `BlockPos`, `ChunkPos`, `Vec3d`
- **No client logic** — packets are pure data containers, codecs are pure transforms
- **No Tokio dependency** — codec operations are synchronous on `bytes::Bytes` / `bytes::BytesMut`
- Independently publishable: proxy and analyzer developers add only this crate

**`headlesscraft`** — Main client library
- `Client` type: connect, authenticate (Mojang/Microsoft), join a server
- Connection management: read/write tasks, keepalive, state transitions
- World state tracking: chunks, entities, block changes, inventory
- Bot API: movement, block interaction, chat, pathfinding interfaces
- Event system: users subscribe to game events (chat, damage, disconnect)
- Re-exports all `headlesscraft-protocol` public types for convenience
- Depends on Tokio (async networking), reqwest (authentication), and other heavy dependencies

### Workspace Configuration

```toml
# Root Cargo.toml
[workspace]
members = ["crates/*"]
resolver = "2"

[workspace.package]
edition = "2024"
rust-version = "1.85"
license = "MIT OR Apache-2.0"
repository = "https://github.com/headlesscraft/headlesscraft"

[workspace.dependencies]
tokio = { version = "1", features = ["rt-multi-thread", "net", "io-util", "time", "sync", "macros"] }
thiserror = "2"
tracing = "0.1"
bytes = "1"
serde = { version = "1", features = ["derive"] }
```

### Dependency Footprint Comparison

| Consumer | Crate | Gets Tokio? | Gets reqwest? | Gets dashmap? |
|----------|-------|-------------|---------------|---------------|
| Packet sniffer | `headlesscraft-protocol` | No | No | No |
| Proxy server | `headlesscraft-protocol` | No | No | No |
| Bot framework | `headlesscraft` | Yes | Yes | Yes |
| Load tester | `headlesscraft` | Yes | Yes | Yes |

## Consequences

### Positive

- Protocol-only consumers get a lean dependency tree: `bytes`, `thiserror`, `tracing`, `serde` — no async runtime, no HTTP client
- Compile-time enforcement of layer boundaries — `headlesscraft-protocol` physically cannot import client types
- Each crate is independently testable: `cargo test -p headlesscraft-protocol` runs without building client code
- Re-exports in `headlesscraft` mean bot developers never need to name sub-crates in their `Cargo.toml`
- Three crates is a manageable cognitive load for contributors — clear ownership of every type

### Negative

- Cross-crate API changes require coordination — adding a field to a packet struct may require updating both protocol and client crates
- Protocol crate integrates NBT directly rather than isolating it, making a future NBT-only extraction harder (acceptable trade-off for a client library)
- `pub` visibility must be carefully designed at the protocol↔client boundary

### Neutral

- The 3-crate structure may grow if a clear need emerges (e.g., a separate `headlesscraft-auth` crate if authentication becomes complex enough)
- Workspace-level `[lints]` ensure consistent Clippy and rustc warnings across all crates
- Feature flags on `headlesscraft` may gate optional functionality (e.g., `pathfinding`, `physics`) to keep the default build lean

## Compliance

- **Dependency DAG check**: CI verifies that `headlesscraft-protocol` does not depend on `tokio`, `reqwest`, or any client-only crate
- **No wildcard re-exports**: code review rejects `pub use headlesscraft_protocol::*` at the crate root — re-exports go through a curated `prelude` module
- **Independent build test**: CI runs `cargo check -p headlesscraft-protocol` with no default features to verify standalone usability
- **Feature flag audit**: new feature flags must be documented in the crate-level README and workspace CI matrix

## Related ADRs

- [ADR-002: Error Handling Strategy](adr-002-error-handling.md) — each crate defines its own error types with thiserror
- [ADR-003: Async Runtime Selection](adr-003-async-runtime.md) — Tokio is confined to `headlesscraft`, not `headlesscraft-protocol`
- [ADR-004: Logging & Observability](adr-004-logging-observability.md) — `tracing` is used in both crates, but subscriber setup is the user's responsibility

## References

- [Cargo Workspaces — The Rust Book](https://doc.rust-lang.org/book/ch14-03-cargo-workspaces.html)
- [Cargo workspace.dependencies](https://doc.rust-lang.org/cargo/reference/workspaces.html#the-dependencies-table)
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- [Proc-macro crate type — Rust Reference](https://doc.rust-lang.org/reference/procedural-macros.html)
- [Matklad — "Large Rust Workspaces"](https://matklad.github.io/2021/09/04/fast-rust-builds.html)
