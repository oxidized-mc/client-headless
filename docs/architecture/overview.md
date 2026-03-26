# Architecture Overview — HeadlessCraft

HeadlessCraft is a Rust framework for building headless Minecraft Java Edition clients.
It connects to vanilla servers, handles the full protocol lifecycle, and exposes
a high-level API for bots, testing tools, and automation.

## Crate Layout

```
headlesscraft-macros    ← Proc-macro derives (packet codecs, etc.)
headlesscraft-protocol  ← Packet definitions, VarInt codec, encryption, compression, NBT, types
headlesscraft           ← Client logic, world state, bot API (the main library)
```

Modules within `headlesscraft-protocol`:
- **packets** — all packet definitions for every connection state
- **codec** — VarInt/VarLong, framing, encryption, compression
- **nbt** — Named Binary Tag serialization (13 tag types)
- **types** — shared coordinate types, block IDs, protocol primitives

Modules within `headlesscraft` (main crate):
- **client** — connection management, authentication, session handling
- **world** — client-side world state (chunks, entities, biomes)
- **bot** — high-level bot behavior API and event system

## Design Principles

1. **Wire compatibility** — Every packet must match what vanilla servers expect
2. **Ergonomic API** — Builder patterns, sensible defaults, event-driven
3. **Performance** — Async-first, zero-copy where possible, hundreds of concurrent clients
4. **Safety** — No unsafe code, strong typing, comprehensive error handling
5. **Testability** — Every component independently testable
