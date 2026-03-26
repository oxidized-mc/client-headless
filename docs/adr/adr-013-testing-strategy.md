# ADR-013: Testing Strategy

| Field | Value |
|-------|-------|
| Status | Accepted |
| Date | 2026-03-26 |
| Phases | P01-P17 |
| Deciders | HeadlessCraft Core Team |

## Context

HeadlessCraft is a protocol library where correctness is binary: one wrong byte
in a packet encoding breaks the connection. A single off-by-one in a VarInt
codec silently corrupts every subsequent packet. The Minecraft protocol has
hundreds of packet types, complex data structures (NBT, paletted containers,
entity metadata), and version-specific behaviors that must all be verified.

Beyond protocol correctness, the library includes authentication flows, world
state management, event dispatching, and multi-client orchestration — each with
its own failure modes. Auth flows involve HTTP calls with specific JSON payloads.
World state must correctly apply sequences of chunk-load and entity-spawn
packets. Event handlers must be called in the right order with the right data.

Without rigorous testing, regressions are invisible until a real Minecraft server
rejects the connection or a bot silently misinterprets the world. Manual testing
against live servers is slow, flaky, and not CI-friendly. We need a layered
testing strategy that catches issues at every level, from individual codec
functions to full connection handshakes.

## Decision Drivers

- Protocol correctness is non-negotiable — one wrong byte = broken connection
- Must catch regressions automatically in CI, not through manual play-testing
- Must test codec roundtrip properties (encode → decode = identity) exhaustively
- Must verify exact byte sequences against vanilla captures
- Must support testing without a running Minecraft server (fast, hermetic)
- Should support optional integration tests against a real server (slow, thorough)
- Must test async behavior (connection flows, event dispatch, multi-client)
- Should produce useful failure output (what was expected vs. actual)

## Considered Options

### Option 1: Unit Tests Only

Standard `#[test]` functions in each module. Test individual functions with
hand-crafted inputs.

**Pros:** Simple. Fast. No extra dependencies.
**Cons:** Misses edge cases that hand-crafted tests don't cover. No way to
verify vanilla compatibility systematically. Easy to miss codec asymmetries.
Insufficient for a protocol library.

### Option 2: Full Pyramid with Property-Based + Compliance

Unit tests + property-based testing (proptest) for codec roundtrips +
compliance tests with captured vanilla byte sequences + snapshot tests for
error messages + optional integration tests.

**Pros:** Catches issues at every level. Property-based tests find edge cases
humans miss. Compliance tests guarantee vanilla compatibility. Snapshot tests
catch unintended output changes.
**Cons:** More test infrastructure to maintain. Compliance fixtures must be
updated when targeting new protocol versions. Property-based tests are slower
than unit tests.

### Option 3: Integration Tests Against Real MC Server

Focus on end-to-end tests using a dockerized Minecraft server. Test full
connection flows, handshakes, and gameplay.

**Pros:** Tests the real thing. High confidence in compatibility.
**Cons:** Slow (server startup ~10s). Flaky (network timing, server state).
Cannot run without Docker. Not suitable for rapid development iteration. Misses
internal invariants that integration tests don't exercise.

## Decision

**Option 2: Full test pyramid with all test types.**

### Test Types and Locations

| Type | Location | Purpose | Speed |
|------|----------|---------|-------|
| Unit | `#[cfg(test)] mod tests` in source | Individual function correctness | Fast |
| Property-based | Inline or `tests/` with `proptest` | Codec roundtrip invariants | Medium |
| Compliance | `headlesscraft-protocol/tests/compliance/` | Exact byte verification vs. vanilla | Fast |
| Snapshot | Inline with `insta::assert_snapshot!` | Error messages, debug output | Fast |
| Integration | `tests/integration/` | Full connection flows (dockerized) | Slow |
| Doc | `///` on public items | API usage examples that compile | Fast |

### Test Naming Conventions

```rust
// Unit tests: test_<thing>_<condition>
#[test]
fn test_varint_encodes_zero() { /* ... */ }

#[test]
fn test_varint_encodes_max_value() { /* ... */ }

// Outcome-when-condition style:
#[test]
fn varint_returns_error_when_too_many_bytes() { /* ... */ }

#[test]
fn chunk_section_is_empty_when_all_air() { /* ... */ }

// Proptest: proptest_<thing>_<invariant>
proptest! {
    #[test]
    fn proptest_varint_roundtrips(value in any::<i32>()) { /* ... */ }
}

// Compliance: compliance_<packet>_<scenario>
#[test]
fn compliance_handshake_packet_matches_vanilla() { /* ... */ }
```

### Test Module Setup

```rust
// In every source file with tests:
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_varint_encodes_zero() {
        let mut buf = Vec::new();
        VarInt(0).encode(&mut buf).unwrap();
        assert_eq!(buf, &[0x00]);
    }

    #[test]
    fn test_varint_encodes_negative_one() {
        let mut buf = Vec::new();
        VarInt(-1).encode(&mut buf).unwrap();
        assert_eq!(buf, &[0xFF, 0xFF, 0xFF, 0xFF, 0x0F]);
    }

    #[test]
    fn varint_returns_error_when_too_many_bytes() {
        let data = [0x80, 0x80, 0x80, 0x80, 0x80, 0x01]; // 6 continuation bytes
        let result = VarInt::decode(&mut &data[..]);
        assert!(result.is_err());
    }
}
```

### Property-Based Tests with Proptest

```rust
use proptest::prelude::*;

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    proptest! {
        /// Every i32 value must survive a VarInt encode → decode roundtrip.
        #[test]
        fn proptest_varint_roundtrips(value in any::<i32>()) {
            let mut buf = Vec::new();
            VarInt(value).encode(&mut buf).unwrap();
            let decoded = VarInt::decode(&mut &buf[..]).unwrap();
            prop_assert_eq!(decoded.0, value);
        }

        /// VarInt encoding never exceeds 5 bytes.
        #[test]
        fn proptest_varint_max_5_bytes(value in any::<i32>()) {
            let mut buf = Vec::new();
            VarInt(value).encode(&mut buf).unwrap();
            prop_assert!(buf.len() <= 5, "VarInt was {} bytes", buf.len());
        }

        /// String encode → decode preserves content and length.
        #[test]
        fn proptest_string_roundtrips(s in ".{0,32767}") {
            let mut buf = Vec::new();
            McString::encode(&s, &mut buf).unwrap();
            let decoded = McString::decode(&mut &buf[..]).unwrap();
            prop_assert_eq!(decoded, s);
        }

        /// Position encoding roundtrips for all valid coordinates.
        #[test]
        fn proptest_position_roundtrips(
            x in -33554432i32..33554432,  // 26-bit signed
            y in -2048i32..2048,          // 12-bit signed
            z in -33554432i32..33554432,  // 26-bit signed
        ) {
            let pos = BlockPos::new(x, y, z);
            let encoded = pos.encode_u64();
            let decoded = BlockPos::decode_u64(encoded);
            prop_assert_eq!(decoded, pos);
        }
    }
}
```

### Compliance Tests

Compliance tests verify that our encoding matches vanilla's byte-for-byte.
Test fixtures are captured from a real vanilla 26.1 server using a packet
proxy.

```rust
// headlesscraft-protocol/tests/compliance/handshake.rs

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use headlesscraft_protocol::packets::handshake::*;
    use headlesscraft_protocol::codec::PacketEncoder;

    /// Vanilla 26.1 handshake packet captured from wireshark.
    const VANILLA_HANDSHAKE: &[u8] = &[
        0x00,                               // Packet ID: Handshake (0x00)
        0x87, 0x06,                         // Protocol version: 775 (VarInt)
        0x09, 0x6C, 0x6F, 0x63, 0x61, 0x6C,// Server address: "localhost"
        0x68, 0x6F, 0x73, 0x74,
        0x63, 0xDD,                         // Port: 25565
        0x02,                               // Next state: Login (2)
    ];

    #[test]
    fn compliance_handshake_packet_matches_vanilla() {
        let packet = HandshakePacket {
            protocol_version: VarInt(775),
            server_address: "localhost".into(),
            server_port: 25565,
            next_state: ConnectionIntent::Login,
        };

        let mut buf = Vec::new();
        packet.encode(&mut buf).unwrap();
        assert_eq!(
            buf, VANILLA_HANDSHAKE,
            "Handshake encoding doesn't match vanilla capture"
        );
    }

    #[test]
    fn compliance_handshake_decodes_vanilla_capture() {
        let packet = HandshakePacket::decode(&mut &VANILLA_HANDSHAKE[..]).unwrap();
        assert_eq!(packet.protocol_version.0, 775);
        assert_eq!(packet.server_address, "localhost");
        assert_eq!(packet.server_port, 25565);
        assert_eq!(packet.next_state, ConnectionIntent::Login);
    }
}
```

### Snapshot Tests with Insta

```rust
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use insta::assert_snapshot;

    #[test]
    fn test_protocol_error_display() {
        let err = ProtocolError::InvalidPacketId {
            state: ConnectionState::Play,
            id: 0xFF,
        };
        assert_snapshot!(err.to_string(), @"invalid packet ID 0xFF in Play state");
    }

    #[test]
    fn test_varint_error_display() {
        let err = VarIntError::TooManyBytes { count: 6 };
        assert_snapshot!(err.to_string(), @"VarInt too long: 6 bytes (max 5)");
    }

    #[test]
    fn test_auth_error_display() {
        let err = AuthError::MinecraftNotOwned;
        assert_snapshot!(
            err.to_string(),
            @"Microsoft account does not own Minecraft Java Edition"
        );
    }
}
```

### Integration Tests (CI-Only, Dockerized)

```rust
// tests/integration/connection.rs
// Requires: HEADLESSCRAFT_TEST_SERVER=localhost:25565 (offline-mode vanilla)
// Run with: cargo test --features integration-tests

#[cfg(feature = "integration-tests")]
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use headlesscraft::{Client, OfflineAuth, EventChannel};

    #[tokio::test]
    async fn test_connect_and_receive_join_game() {
        let server = std::env::var("HEADLESSCRAFT_TEST_SERVER")
            .unwrap_or_else(|_| "localhost:25565".into());

        let (channel, mut rx) = EventChannel::new(64);

        let client = Client::builder()
            .server_address(&server)
            .authenticator(OfflineAuth::new("IntegrationBot"))
            .event_handler(channel)
            .build()
            .await
            .expect("failed to connect");

        // Should receive dimension change event (join game)
        let event = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            rx.recv(),
        )
        .await
        .expect("timed out waiting for join event")
        .expect("channel closed");

        assert!(
            matches!(event, Event::DimensionChange(_)),
            "first event should be dimension change, got: {event:?}"
        );

        client.disconnect().await.unwrap();
    }
}
```

### Doc Tests

```rust
/// Encode a value as a Minecraft protocol VarInt.
///
/// VarInts use 1-5 bytes with 7 bits of data per byte.
///
/// # Examples
///
/// ```
/// use headlesscraft_protocol::VarInt;
///
/// let mut buf = Vec::new();
/// VarInt(300).encode(&mut buf).unwrap();
/// assert_eq!(buf, &[0xAC, 0x02]);
/// ```
///
/// # Errors
///
/// Returns [`EncodeError::BufferTooSmall`] if the writer cannot accept bytes.
pub fn encode(&self, writer: &mut impl Write) -> Result<(), EncodeError> {
    // ...
}
```

### CI Configuration

```yaml
# .github/workflows/test.yml (conceptual)
jobs:
  unit-and-property:
    steps:
      - cargo test --workspace
      - cargo test --workspace -- --ignored  # slow proptest cases

  compliance:
    steps:
      - cargo test -p headlesscraft-protocol --test compliance

  integration:
    services:
      minecraft:
        image: itzg/minecraft-server:java21
        env:
          EULA: "TRUE"
          ONLINE_MODE: "false"
          VERSION: "1.21.5"
    steps:
      - cargo test --features integration-tests
```

### Minimum Test Requirements Per PR

Every pull request must include:

1. **Unit tests** for all new/changed functions
2. **Property-based tests** for any new codec, parser, or encoder
3. **Compliance tests** when adding or modifying packet definitions
4. **Doc tests** on all new public items
5. **Snapshot tests** for new error types or display implementations

Integration tests are run in CI but not required for every PR.

## Consequences

### Positive

- Property-based tests catch edge cases that hand-crafted tests miss (e.g.,
  VarInt boundary values, unusual string lengths, extreme coordinates)
- Compliance tests guarantee byte-for-byte vanilla compatibility
- Snapshot tests catch unintended changes to error messages or debug output
- Doc tests ensure examples stay in sync with API changes
- Full pyramid gives high confidence with fast feedback (unit tests run in <1s)

### Negative

- Compliance fixtures must be re-captured when upgrading protocol versions
- Property-based tests are slower than unit tests (~5s vs. ~0.1s per test file)
- Dockerized integration tests add CI complexity and ~60s overhead
- More test code to maintain (test code may exceed production code volume)

### Neutral

- `proptest` regressions are stored in `proptest-regressions/` and committed
  to the repository to prevent re-discovery of known edge cases
- `insta` snapshots are stored in `snapshots/` directories adjacent to tests
- Integration test server version must match the protocol version we target
- Test coverage metrics are tracked but not gated — quality over quantity

## Compliance

- Compliance test fixtures are captured from vanilla Minecraft 26.1 (775)
- All VarInt/VarLong tests validate against wiki.vg specification examples
- Position encoding tests cover the full valid coordinate range

## Related ADRs

- ADR-009: Authentication Flow (auth flow integration tests)
- ADR-010: Client World State (chunk parsing property tests)
- ADR-011: Event & Handler System (event dispatch unit tests)
- ADR-012: Multi-Client Architecture (multi-client integration tests)

## References

- [proptest crate](https://docs.rs/proptest/)
- [insta snapshot testing](https://docs.rs/insta/)
- [wiki.vg Protocol Specification](https://wiki.vg/Protocol)
- [itzg/minecraft-server Docker image](https://github.com/itzg/docker-minecraft-server)
- [Minecraft Protocol Version 775](https://wiki.vg/Protocol_version_numbers)
