# ADR-008: NBT Library Design

| Field | Value |
|-------|-------|
| Status | Accepted |
| Date | 2026-03-26 |
| Phases | P05 |
| Deciders | HeadlessCraft Core Team |

## Context

Named Binary Tag (NBT) is Minecraft's primary structured data format, used pervasively across
the protocol and world storage. As a headless client, HeadlessCraft encounters NBT in many
contexts: registry data sent during Configuration, chunk sections in Play, entity metadata,
item stack components, chat components, and more. The format defines 13 tag types (End,
Byte, Short, Int, Long, Float, Double, ByteArray, String, List, Compound, IntArray,
LongArray) organized in a tree rooted at a Compound tag.

Since Minecraft 1.20.2, the protocol uses a variant called **Network NBT** that differs from
classic (file-based) NBT in two ways: (1) the root compound has **no name** (classic NBT
prefixes the root with a TAG_Compound type byte and a UTF-8 name), and (2) an empty root
is encoded as a single `0x00` byte (TAG_End) rather than an empty TAG_Compound. HeadlessCraft
must handle both variants — network NBT for protocol packets and classic NBT for any offline
data processing (e.g., reading `.dat` files for test fixtures).

The key architectural question is where the NBT implementation lives. Oxidized places it in a
separate `oxidized-nbt` crate because multiple crates (protocol, world, game) depend on it.
HeadlessCraft has a simpler 3-crate workspace where only `headlesscraft-protocol` needs NBT.
The main `headlesscraft` crate accesses NBT through re-exports from the protocol crate.

## Decision Drivers

- **Scope:** Only `headlesscraft-protocol` directly parses/writes NBT; other crates interact through typed protocol structs.
- **Performance:** Chunk data contains large NBT compounds; parsing must be efficient and minimize allocations.
- **Serde compatibility:** Bot authors expect to deserialize NBT into custom Rust structs via `serde`.
- **Protocol integration:** NBT values must implement `Encode`/`Decode` from ADR-005 for seamless use in packet structs.
- **Simplicity:** Fewer crates means less coordination, faster builds, and simpler dependency management.
- **Correctness:** Both classic and network NBT variants must be supported without ambiguity.

## Considered Options

### Option 1: Separate `headlesscraft-nbt` Crate

Create a fourth workspace crate dedicated to NBT, mirroring Oxidized's `oxidized-nbt`.

**Pros:** Clean separation, reusable outside the project, follows Oxidized's proven pattern.
**Cons:** Adds a crate to the workspace for a single consumer, increases build graph
complexity, `Encode`/`Decode` integration requires cross-crate trait impls or adapter types.

### Option 2: Module Within `headlesscraft-protocol`

Implement NBT as a `pub mod nbt` inside the protocol crate, alongside packet definitions and
codec types.

**Pros:** Single consumer simplifies dependency graph, `Encode`/`Decode` integration is
trivial (same crate), fewer crates to manage, still publicly accessible via
`headlesscraft_protocol::nbt`.
**Cons:** Protocol crate grows larger, NBT module cannot be used without pulling in the
protocol crate, less reusable for hypothetical future consumers.

### Option 3: Use Existing Community Crate

Depend on `valence_nbt`, `fastnbt`, `hematite-nbt`, or another community NBT library.

**Pros:** Zero implementation effort, battle-tested, maintained by others.
**Cons:** May not support network NBT (the 1.20.2+ variant), adds an external dependency
with its own release cadence, trait integration with our `Encode`/`Decode` requires wrapper
types, feature gaps require upstream PRs or forks.

## Decision

**Option 2 — Module within `headlesscraft-protocol`.**

HeadlessCraft's 3-crate workspace is deliberately minimal. Adding a fourth crate for a single
consumer adds complexity without benefit. The `nbt` module lives at
`headlesscraft-protocol/src/nbt/` and is publicly exported. If a future crate needs direct NBT
access, extraction to a separate crate is a straightforward refactor.

### Module Structure

```
headlesscraft-protocol/src/nbt/
├── mod.rs          // Public API, re-exports
├── tag.rs          // Tag enum and tag type IDs
├── decode.rs       // Decoding (both classic and network)
├── encode.rs       // Encoding (both classic and network)
├── compound.rs     // Compound type (ordered map)
├── list.rs         // List type (homogeneous tag vector)
└── serde_impl.rs   // Optional serde Serialize/Deserialize (behind feature flag)
```

### Tag Type Enum

```rust
/// The 13 NBT tag types.
#[derive(Debug, Clone, PartialEq)]
pub enum Tag {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ByteArray(Vec<i8>),
    String(String),
    List(List),
    Compound(Compound),
    IntArray(Vec<i32>),
    LongArray(Vec<i64>),
}

/// Tag type IDs as defined by the NBT specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TagId {
    End       = 0,
    Byte      = 1,
    Short     = 2,
    Int       = 3,
    Long      = 4,
    Float     = 5,
    Double    = 6,
    ByteArray = 7,
    String    = 8,
    List      = 9,
    Compound  = 10,
    IntArray   = 11,
    LongArray  = 12,
}
```

### Compound and List Types

```rust
/// An ordered map of string keys to NBT tags.
///
/// Preserves insertion order (important for consistent re-encoding).
#[derive(Debug, Clone, PartialEq)]
pub struct Compound {
    entries: IndexMap<String, Tag>,
}

impl Compound {
    pub fn new() -> Self { ... }
    pub fn get(&self, key: &str) -> Option<&Tag> { ... }
    pub fn insert(&mut self, key: String, value: Tag) -> Option<Tag> { ... }
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Tag)> { ... }

    /// Convenience accessor that returns an error if the key is missing or wrong type.
    pub fn get_i32(&self, key: &str) -> Result<i32, NbtError> { ... }
    pub fn get_string(&self, key: &str) -> Result<&str, NbtError> { ... }
    pub fn get_compound(&self, key: &str) -> Result<&Compound, NbtError> { ... }
    // ... etc for each tag type
}

/// A homogeneous list of NBT tags. All elements must share the same tag type.
#[derive(Debug, Clone, PartialEq)]
pub struct List {
    element_type: TagId,
    tags: Vec<Tag>,
}
```

`IndexMap` (from the `indexmap` crate) preserves insertion order, which is important for
deterministic round-trip encoding and compatibility with vanilla behavior.

### Encoding and Decoding API

```rust
/// Decode a classic NBT compound (with root name) from a byte buffer.
pub fn decode_classic(buf: &mut &[u8]) -> Result<(String, Compound), NbtError> {
    let tag_type = TagId::try_from(read_u8(buf)?)?;
    if tag_type != TagId::Compound {
        return Err(NbtError::InvalidRootType(tag_type));
    }
    let name = read_utf8(buf)?;
    let compound = decode_compound(buf)?;
    Ok((name, compound))
}

/// Decode network NBT (no root name, 1.20.2+ protocol variant).
///
/// An empty compound is represented as a single 0x00 byte (TAG_End).
pub fn decode_network(buf: &mut &[u8]) -> Result<Compound, NbtError> {
    let tag_type = TagId::try_from(read_u8(buf)?)?;
    match tag_type {
        TagId::End => Ok(Compound::new()),      // Empty compound
        TagId::Compound => decode_compound(buf), // No name follows
        _ => Err(NbtError::InvalidNetworkRoot(tag_type)),
    }
}

/// Encode a compound as network NBT.
pub fn encode_network(compound: &Compound, buf: &mut impl BufMut) -> Result<(), NbtError> {
    if compound.is_empty() {
        buf.put_u8(TagId::End as u8);
    } else {
        buf.put_u8(TagId::Compound as u8);
        encode_compound(compound, buf)?;
    }
    Ok(())
}
```

### Encode/Decode Trait Integration

Because NBT lives in the same crate as `Encode`/`Decode`, integration is direct:

```rust
impl Encode for Compound {
    fn encode(&self, buf: &mut impl BufMut) -> Result<(), EncodeError> {
        encode_network(self, buf).map_err(EncodeError::Nbt)
    }
}

impl<'a> Decode<'a> for Compound {
    fn decode(buf: &mut &'a [u8]) -> Result<Self, DecodeError> {
        decode_network(buf).map_err(DecodeError::Nbt)
    }
}
```

This allows packet structs to use `Compound` directly:

```rust
#[derive(McEncode, McDecode)]
#[mc(id = 0x07, state = "configuration", direction = "client_bound")]
pub struct RegistryData {
    pub registry_id: String,
    pub entries: Vec<RegistryEntry>,
}

pub struct RegistryEntry {
    pub entry_id: String,
    /// NBT data for this registry entry, if present.
    pub data: Option<Compound>,
}
```

### Serde Integration (Feature-Gated)

Behind a `serde` feature flag, `Tag`, `Compound`, and `List` implement `Serialize` and
`Deserialize`. Additionally, a `from_compound` / `to_compound` API lets users convert
between typed Rust structs and NBT compounds:

```rust
#[cfg(feature = "serde")]
pub fn from_compound<'de, T: Deserialize<'de>>(compound: &'de Compound) -> Result<T, NbtError> {
    T::deserialize(NbtDeserializer::new(compound))
}

// Usage:
#[derive(Deserialize)]
struct DimensionType {
    min_y: i32,
    height: i32,
    has_skylight: bool,
}

let dim: DimensionType = nbt::from_compound(&registry_entry.data)?;
```

### Zero-Copy Where Possible

For large byte/int/long arrays (common in chunk heightmaps and biome data), the decoder
can borrow directly from the input buffer when the caller provides a reference with
sufficient lifetime:

```rust
/// A borrowed byte array tag that avoids copying.
pub struct ByteArrayRef<'a>(&'a [u8]);

impl<'a> Decode<'a> for ByteArrayRef<'a> {
    fn decode(buf: &mut &'a [u8]) -> Result<Self, DecodeError> {
        let len = i32::decode(buf)? as usize;
        if buf.len() < len {
            return Err(DecodeError::UnexpectedEof);
        }
        let (data, rest) = buf.split_at(len);
        *buf = rest;
        Ok(ByteArrayRef(data))
    }
}
```

## Consequences

### Positive

- No additional crate in the workspace — simpler dependency graph and faster incremental builds.
- `Encode`/`Decode` integration is trivial since NBT and packet codecs share a crate.
- Both classic and network NBT are supported with clear, separate entry points.
- Serde integration (feature-gated) gives bot authors ergonomic typed access to NBT data.
- `IndexMap` preserves insertion order for deterministic round-trip encoding.
- Zero-copy borrowed types reduce allocations when parsing large chunk data.

### Negative

- The `headlesscraft-protocol` crate grows larger, which may slow full-crate compilation.
- NBT cannot be used independently without depending on the protocol crate. This is acceptable given the single-consumer design but limits hypothetical reuse.
- We maintain our own NBT implementation rather than leveraging community work. The upside is full control over network NBT support and trait integration.

### Neutral

- If a future crate (e.g., a world storage crate) needs direct NBT access, we can extract the `nbt` module into its own crate without breaking the public API — `headlesscraft_protocol::nbt` would become a re-export.
- The `serde` feature flag keeps the dependency optional for users who only need raw `Tag` access.
- Classic NBT support is included for test fixture parsing (`.dat` files) even though the protocol only uses network NBT.

## Compliance

- Round-trip property tests: `decode(encode(compound)) == compound` for all tag types.
- Byte-level compliance tests against NBT blobs captured from a vanilla 1.21.5 server.
- Network NBT edge cases: empty compound (single `0x00` byte), deeply nested compounds, maximum string length (32767 bytes).
- Classic NBT tests against Mojang's canonical `bigtest.nbt` and `hello_world.nbt` test files.
- Serde integration tests verifying deserialization of registry data into typed structs.

## Related ADRs

- ADR-005: Packet Codec Framework — the `Encode`/`Decode` traits that NBT integrates with.
- ADR-006: Connection Lifecycle — NBT-heavy packets appear in Configuration (registries) and Play (chunks, entities).
- ADR-007: Encryption & Compression Pipeline — NBT payloads are compressed/encrypted like any other packet data.

## References

- [NBT Specification (wiki.vg)](https://wiki.vg/NBT)
- [Network NBT (1.20.2+ changes)](https://wiki.vg/NBT#Network_NBT)
- [Mojang's NBT Test Files](https://wiki.vg/NBT#Test_files)
- [`indexmap` crate](https://docs.rs/indexmap)
- [`serde` framework](https://serde.rs/)
- [`bytes` crate](https://docs.rs/bytes)
