# ADR-005: Packet Codec Framework

| Field | Value |
|-------|-------|
| Status | Accepted |
| Date | 2026-03-26 |
| Phases | P02, P03, P04, P06, P07 |
| Deciders | HeadlessCraft Core Team |

## Context

The Minecraft Java Edition protocol (version 775, protocol 26.1) defines approximately 900
packet types distributed across five connection states: Handshaking, Status, Login,
Configuration, and Play. Each packet is identified by a numeric ID that is only unique within
a given state and direction (client-bound vs. server-bound). As a headless client library,
HeadlessCraft must be able to both **encode** packets it sends to the server and **decode**
packets it receives — the inverse of a server implementation.

Every packet is framed on the wire as: `length (VarInt) | packet ID (VarInt) | payload`.
The payload is a sequence of fields encoded using Minecraft-specific types: VarInt, VarLong,
strings with a VarInt length prefix, positions packed into a `u64`, UUIDs, NBT compounds,
optional fields gated by a boolean prefix, and many more. Implementing encode/decode by hand
for each of the ~900 packets is error-prone, tedious, and a maintenance burden whenever the
protocol is updated.

HeadlessCraft needs a codec framework that is type-safe, performant, and ergonomic. Because
this is a library consumed by bot authors and testing frameworks, compile-time correctness and
clear error messages are critical. The framework must support incremental adoption — core
primitives and a few packets for login first, with Play packets added over subsequent phases.

## Decision Drivers

- **Correctness:** Wire format must match vanilla exactly; a single misencoded field breaks the connection.
- **Ergonomics:** Adding a new packet should require minimal boilerplate — ideally a struct definition plus a derive.
- **Performance:** Bots may maintain many concurrent connections; codec overhead must be minimal.
- **Maintainability:** Protocol updates (new snapshot, new version) should require only struct changes, not codec logic.
- **Testability:** Each packet's encoding must be independently testable against known byte sequences.
- **Client focus:** We encode server-bound packets and decode client-bound packets — the trait impls must be directional.

## Considered Options

### Option 1: Derive Macros with Field Attributes

Provide `#[derive(McEncode, McDecode)]` proc macros in `headlesscraft-macros` that inspect
struct fields and generate trait implementations. Field-level attributes like
`#[mc(varint)]`, `#[mc(optional)]`, `#[mc(length_prefix = "varint")]` control wire encoding.

**Pros:** Minimal boilerplate, compile-time validation, self-documenting struct definitions.
**Cons:** Proc-macro complexity, potentially opaque error messages, macro debugging is harder.

### Option 2: Manual Trait Implementations

Hand-write `Encode` and `Decode` for every packet type.

**Pros:** Full control, no macro magic, simple tooling.
**Cons:** Massive boilerplate (~900 packets × 2 traits), high risk of copy-paste errors,
painful protocol updates.

### Option 3: Code Generation from Protocol Spec JSON

Use an external protocol specification (e.g., PrismarineJS `minecraft-data`) to generate
Rust packet structs and codec impls at build time via `build.rs`.

**Pros:** Automatic protocol updates, single source of truth.
**Cons:** Generated code is hard to customize, poor IDE support, spec may lag behind or
diverge from vanilla, opaque build failures, large compile-time dependency.

## Decision

**Option 1 — Derive macros with field attributes**, implemented in the `headlesscraft-macros`
crate and consumed by `headlesscraft-protocol`.

### Core Traits

```rust
/// Encode a value to the Minecraft wire format.
pub trait Encode {
    fn encode(&self, buf: &mut impl BufMut) -> Result<(), EncodeError>;
}

/// Decode a value from the Minecraft wire format.
pub trait Decode<'a>: Sized {
    fn decode(buf: &mut &'a [u8]) -> Result<Self, DecodeError>;
}
```

The lifetime on `Decode<'a>` enables zero-copy decoding for borrowed types like `&'a str`
and `&'a [u8]` when the caller retains the buffer.

### Packet Trait

```rust
pub trait Packet: Encode + for<'a> Decode<'a> {
    /// Packet ID within its connection state.
    const ID: i32;

    /// Connection state this packet belongs to.
    const STATE: ConnectionState;

    /// Direction: client-bound or server-bound.
    const DIRECTION: Direction;
}
```

### Derive Usage

```rust
use headlesscraft_macros::{McEncode, McDecode};
use headlesscraft_protocol::{Packet, ConnectionState, Direction};

#[derive(McEncode, McDecode)]
#[mc(id = 0x00, state = "handshaking", direction = "server_bound")]
pub struct HandshakePacket {
    #[mc(varint)]
    pub protocol_version: i32,
    pub server_address: String,
    pub server_port: u16,
    #[mc(varint)]
    pub next_state: i32,
}
```

The `#[mc(...)]` attribute on the struct generates the `Packet` trait impl. Field attributes
control encoding:

| Attribute | Effect |
|-----------|--------|
| `#[mc(varint)]` | Encode/decode as VarInt (max 5 bytes) |
| `#[mc(varlong)]` | Encode/decode as VarLong (max 10 bytes) |
| `#[mc(optional)]` | Prefix with `bool`; `None` writes `false` + no payload |
| `#[mc(length_prefix = "varint")]` | Vec/String prefixed with VarInt length |
| `#[mc(fixed_length = N)]` | Fixed-size array, no length prefix |
| `#[mc(nbt)]` | Encode/decode as network NBT |
| `#[mc(json)]` | JSON string (serde_json round-trip) |

### VarInt / VarLong

```rust
pub struct VarInt(pub i32);

impl Encode for VarInt {
    fn encode(&self, buf: &mut impl BufMut) -> Result<(), EncodeError> {
        let mut value = self.0 as u32;
        loop {
            let byte = (value & 0x7F) as u8;
            value >>= 7;
            if value == 0 {
                buf.put_u8(byte);
                return Ok(());
            }
            buf.put_u8(byte | 0x80);
        }
    }
}
```

VarInt uses at most 5 bytes (for negative `i32` values due to sign extension). VarLong uses
at most 10 bytes. Decoding returns `DecodeError::VarIntTooLong` if the continuation bit is
still set after the maximum number of bytes.

### Type-Safe Packet Dispatching

```rust
/// Registry of packet decoders for a given state + direction.
pub struct PacketRegistry<S: State> {
    decoders: HashMap<i32, BoxedDecoder>,
    _state: PhantomData<S>,
}
```

Each connection state has its own registry, populated at startup. When a raw frame arrives,
the dispatcher looks up the packet ID in the current state's registry and decodes it into
the correct concrete type wrapped in a `ClientBoundPacket` enum.

### Manual Impls for Primitives

Primitive types (`bool`, `u8`, `u16`, `i16`, `i32`, `i64`, `f32`, `f64`, `u128` for UUID,
`String`, `Vec<u8>`) have hand-written `Encode`/`Decode` impls in `headlesscraft-protocol`.
The derive macros compose these primitive impls for struct fields.

## Consequences

### Positive

- Adding a packet is a single struct definition with attributes — typically under 20 lines.
- The derive macro catches structural errors at compile time (missing attributes, wrong types).
- Primitive impls are independently unit-tested against known byte sequences.
- Protocol updates require only struct field changes; codec logic is regenerated by the macro.
- Packet registries enforce state-correct dispatching, preventing cross-state decoding bugs.

### Negative

- Proc-macro development has a steep learning curve and macro errors can be confusing.
- The `headlesscraft-macros` crate adds compile-time overhead.
- Complex packets (e.g., chunk data with embedded palette + bit-packed arrays) still need manual `Decode` impls — the derive macro cannot express every wire format quirk.

### Neutral

- We may later add a `#[mc(switch = "...")]` attribute for Minecraft's union-like structures (e.g., entity metadata values keyed by type ID), but this is not required for P02.

## Compliance

- All packet byte representations must be validated against captures from a vanilla 1.21.5 server.
- Property-based tests ensure `decode(encode(x)) == x` for every packet type.
- CI runs compliance tests against known packet byte fixtures extracted from vanilla traffic.

## Related ADRs

- ADR-006: Connection Lifecycle — defines the state machine that determines which packet registry is active.
- ADR-007: Encryption & Compression Pipeline — sits below the codec layer in the I/O stack.
- ADR-008: NBT Library Design — provides the `#[mc(nbt)]` field encoding.

## References

- [wiki.vg Protocol Specification](https://wiki.vg/Protocol)
- [Minecraft Protocol Version 775](https://wiki.vg/Protocol_version_numbers)
- [VarInt and VarLong encoding](https://wiki.vg/Protocol#VarInt_and_VarLong)
- [`tokio-util` codec documentation](https://docs.rs/tokio-util/latest/tokio_util/codec/)
