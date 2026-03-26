# ADR-006: Connection Lifecycle

| Field | Value |
|-------|-------|
| Status | Accepted |
| Date | 2026-03-26 |
| Phases | P02, P03, P04, P06, P07 |
| Deciders | HeadlessCraft Core Team |

## Context

A Minecraft Java Edition client follows a well-defined connection state machine when
connecting to a server. Unlike a server implementation (which reacts to client requests),
HeadlessCraft **initiates** the handshake, drives the login sequence, and must correctly
respond to server-driven state transitions. The protocol defines five connection states:
Handshaking, Status, Login, Configuration, and Play.

The full happy-path flow is: `Handshaking → Login → Configuration → Play`. A shorter
Status flow exists for server list ping: `Handshaking → Status`. Since Minecraft 1.20.2,
the server can request **reconfiguration** — transitioning the client from Play back to
Configuration to apply resource packs, update registries, or change feature flags, then
returning to Play. This bidirectional transition must be handled gracefully.

As a headless client library, HeadlessCraft must expose a clear, hard-to-misuse API for
connection management. Bot authors should not be able to accidentally send a Play packet
during the Login state or forget to handle reconfiguration. At the same time, the API must
be flexible enough to support advanced use cases: custom authentication, protocol analysis
tools, and bots that intentionally hold connections in specific states.

## Decision Drivers

- **Type safety:** Invalid state transitions should be caught at compile time where possible.
- **Client-initiated flow:** The client drives Handshaking and Login; the server drives Configuration↔Play transitions.
- **Reconfiguration support:** Play→Configuration→Play must be first-class, not an afterthought.
- **Async-native:** All I/O is `async` via Tokio; the state machine must compose with async code naturally.
- **Testability:** Each state's packet handling should be testable in isolation.
- **Ergonomics:** Common flows (connect → login → play) should be simple one-liners for bot authors.

## Considered Options

### Option 1: Typestate Pattern (Compile-Time State Enforcement)

Model each connection state as a distinct generic parameter. `Connection<Handshaking>`,
`Connection<Login>`, etc. State transitions consume `self` and return the new type, making
invalid transitions a compile error.

**Pros:** Zero-cost abstraction, impossible to misuse, self-documenting API.
**Cons:** Cannot store heterogeneous connections in a collection, server-initiated transitions
require runtime indirection, generics can make error messages verbose.

### Option 2: Runtime Enum State Machine

A single `Connection` struct with an internal `ConnectionState` enum. Methods check the
current state at runtime and return errors for invalid transitions.

**Pros:** Simpler types, easy to store in collections, handles server-initiated transitions
naturally.
**Cons:** State errors are runtime panics or `Result`s — easy to misuse, no compile-time
safety.

### Option 3: Flat State with Boolean Flags

Track state via booleans: `is_encrypted`, `is_compressed`, `is_authenticated`, `is_playing`.

**Pros:** Simple to implement initially.
**Cons:** Combinatorial explosion of invalid states, no structure, impossible to reason about
correctness, completely unfit for a library API.

## Decision

**Option 1 — Typestate pattern** for the client-driven flow, with a **runtime enum fallback**
for server-initiated transitions (reconfiguration).

### State Diagram

```
                    ┌──────────────┐
                    │ Disconnected │
                    └──────┬───────┘
                           │ connect()
                           ▼
                    ┌──────────────┐
                    │ Handshaking  │
                    └──────┬───────┘
                           │
                ┌──────────┼──────────┐
                │ status() │          │ login()
                ▼          │          ▼
         ┌──────────┐     │   ┌──────────┐
         │  Status   │     │   │  Login   │
         └──────────┘     │   └────┬─────┘
                          │        │ on LoginSuccess
                          │        ▼
                          │  ┌───────────────┐
                          │  │ Configuration │◄──────┐
                          │  └───────┬───────┘       │
                          │          │ on FinishConfig│ server requests
                          │          ▼               │ reconfiguration
                          │     ┌────────┐           │
                          │     │  Play  │───────────┘
                          │     └────────┘
                          │
```

### Typestate Types

```rust
// Zero-sized marker types for each connection state.
pub struct Handshaking;
pub struct Status;
pub struct Login;
pub struct Configuration;
pub struct Play;

/// A connection in a specific protocol state.
///
/// State transitions consume `self`, preventing use-after-transition.
pub struct Connection<S> {
    stream: FramedStream,
    _state: PhantomData<S>,
}
```

### Client-Driven Transitions

```rust
impl Connection<Handshaking> {
    /// Connect to a Minecraft server, completing TCP setup.
    pub async fn connect(addr: &str) -> Result<Self, ConnectError> { ... }

    /// Send a Handshake and transition to Login.
    pub async fn login(self, username: &str) -> Result<Connection<Login>, HandshakeError> {
        self.send(HandshakePacket {
            protocol_version: 775,
            server_address: addr.into(),
            server_port: port,
            next_state: 2, // Login
        }).await?;
        Ok(Connection {
            stream: self.stream,
            _state: PhantomData,
        })
    }

    /// Send a Handshake and transition to Status (server list ping).
    pub async fn status(self) -> Result<Connection<Status>, HandshakeError> { ... }
}

impl Connection<Login> {
    /// Drive the login sequence: Encryption, Compression, LoginSuccess.
    ///
    /// Returns a `Connection<Configuration>` once the server acknowledges login.
    pub async fn authenticate(self, auth: &AuthProvider) -> Result<Connection<Configuration>, LoginError> {
        // Handle EncryptionRequest → send EncryptionResponse
        // Handle SetCompression → enable compression layer
        // Handle LoginSuccess → send LoginAcknowledged
        ...
    }
}
```

### Server-Driven Transitions (Reconfiguration)

Since the server can request Play→Configuration at any time by sending a
`StartConfiguration` packet, we use a runtime wrapper for the "active" phase:

```rust
/// The active phase of a connection, after login completes.
///
/// Wraps the Configuration↔Play transitions that the server controls.
pub enum ActiveConnection {
    Configuration(Connection<Configuration>),
    Play(Connection<Play>),
}

impl ActiveConnection {
    /// Main event loop: reads packets and handles state transitions.
    pub async fn run(&mut self, handler: &mut impl EventHandler) -> Result<(), ConnectionError> {
        loop {
            match self {
                Self::Play(conn) => {
                    match conn.read_packet().await? {
                        PlayPacket::StartConfiguration(_) => {
                            // Server requests reconfiguration.
                            conn.send(ConfigurationAcknowledged).await?;
                            *self = Self::Configuration(conn.into_configuration());
                        }
                        packet => handler.on_play_packet(packet).await?,
                    }
                }
                Self::Configuration(conn) => {
                    match conn.read_packet().await? {
                        ConfigPacket::FinishConfiguration(_) => {
                            conn.send(FinishConfigurationAck).await?;
                            *self = Self::Play(conn.into_play());
                        }
                        packet => handler.on_config_packet(packet).await?,
                    }
                }
            }
        }
    }
}
```

### High-Level Convenience API

For bot authors who want the simplest path:

```rust
let client = HeadlessCraft::builder()
    .server("mc.example.com:25565")
    .account(Account::offline("Bot01"))
    .build()
    .connect()
    .await?;

// `client` is in Play state, ready to receive events.
client.on_event(|event| async {
    match event {
        Event::ChatMessage(msg) => println!("{msg}"),
        _ => {}
    }
}).await;
```

### Keep-Alive Handling

The client must respond to `KeepAlive` packets from the server (in both Play and
Configuration states) with the same payload. This is handled internally by the connection
layer — bot authors never see keep-alive packets. If no keep-alive response is sent within
15 seconds, the server disconnects the client.

```rust
// Internal — not exposed to users.
async fn handle_keep_alive(conn: &mut Connection<Play>, id: i64) -> Result<(), EncodeError> {
    conn.send(KeepAliveResponse { id }).await
}
```

## Consequences

### Positive

- The typestate pattern makes it impossible to send a Play packet during Login at compile time.
- State transitions are explicit and self-documenting — reading the type signatures tells you the protocol flow.
- The `ActiveConnection` enum cleanly handles server-initiated reconfiguration without sacrificing type safety for client-driven transitions.
- Keep-alive is invisible to bot authors, reducing boilerplate and preventing accidental disconnects.
- Each state is independently testable with mock streams.

### Negative

- Typestate connections cannot be stored in homogeneous collections (e.g., `Vec<Connection<_>>`). This is mitigated by `ActiveConnection` for the post-login phase.
- The generic parameter can make error messages verbose, though type aliases help.
- Server-initiated transitions require the runtime `ActiveConnection` wrapper, which is a partial retreat from pure compile-time safety.

### Neutral

- The `Connection<S>` type aliases (`type HandshakingConn = Connection<Handshaking>`) may be useful for documentation but are not required.
- Future protocol versions may add new states (unlikely but possible); adding a new marker type and transition methods is straightforward.

## Compliance

- Integration tests drive a full `Handshaking → Login → Configuration → Play` flow against a vanilla 1.21.5 server.
- Reconfiguration tests verify `Play → Configuration → Play` round-trips using server-sent resource packs.
- Timeout tests verify keep-alive handling under latency.
- Each state transition is unit-tested in isolation with mock I/O.

## Related ADRs

- ADR-005: Packet Codec Framework — provides the `Encode`/`Decode` traits and packet types used by each state.
- ADR-007: Encryption & Compression Pipeline — layers activated during the Login state.
- ADR-008: NBT Library Design — NBT payloads appear in Configuration and Play packets.

## References

- [wiki.vg Protocol Flow](https://wiki.vg/Protocol#Packet_format)
- [wiki.vg Protocol FAQ — State Transitions](https://wiki.vg/Protocol_FAQ)
- [Typestate Pattern in Rust](https://cliffle.com/blog/rust-typestate/)
- [Minecraft 1.20.2 Configuration Phase](https://wiki.vg/Protocol#Configuration)
- [Keep-Alive Handling](https://wiki.vg/Protocol#Keep_Alive)
