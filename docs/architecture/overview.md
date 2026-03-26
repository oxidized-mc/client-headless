# Architecture Overview — HeadlessCraft

HeadlessCraft is a Rust framework for building headless Minecraft Java Edition clients.
It connects to vanilla servers, handles the full protocol lifecycle, and exposes
a high-level API for bots, testing tools, and automation.

## Crate Layout

```
headlesscraft-types     ← Coordinates, block IDs, shared primitives
headlesscraft-nbt       ← NBT (Named Binary Tag) serialization
headlesscraft-macros    ← Proc-macro derives (packet codecs, etc.)
headlesscraft-protocol  ← Packet definitions, VarInt codec, encryption, compression
headlesscraft-world     ← Client-side world state (chunks, entities, biomes)
headlesscraft-client    ← Connection management, session handling, bot API
headlesscraft           ← Public facade crate (re-exports for end users)
```

## Design Principles

1. **Wire compatibility** — Every packet must match what vanilla servers expect
2. **Ergonomic API** — Builder patterns, sensible defaults, event-driven
3. **Performance** — Async-first, zero-copy where possible, hundreds of concurrent clients
4. **Safety** — No unsafe code, strong typing, comprehensive error handling
5. **Testability** — Every component independently testable
