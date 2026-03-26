# ADR-004: Logging & Observability

| Field | Value |
|-------|-------|
| Status | Accepted |
| Date | 2026-03-26 |
| Phases | P01-P17 |
| Deciders | HeadlessCraft Core Team |

## Context

HeadlessCraft is a library — it runs inside the user's process, not its own. This creates a hard constraint that server projects like Oxidized do not face: **we must not install a logging subscriber.** If HeadlessCraft called `tracing_subscriber::init()` or `env_logger::init()`, it would conflict with the user's own subscriber, panic at runtime (double-init), or silently override the user's logging configuration. The user owns their process, their subscriber, and their log output format. We emit events; they decide where those events go.

At the same time, a headless Minecraft client has rich observability needs. Bot developers debugging flaky connections need to see the packet-level handshake flow. Load-test operators running 200 bots need per-connection metrics without drowning in noise. Protocol developers need to trace exactly which bytes were read, how they were decoded, and where parsing failed. Without structured instrumentation, users resort to sprinkling `println!` in our source code and rebuilding — an unacceptable developer experience for a published library.

The standard Minecraft Java client (vanilla launcher) uses Log4j with minimal, unstructured logging — connection events and errors as flat strings. This is insufficient for automation use cases. A bot operator needs to correlate a disconnect event with the specific bot, the server it was connected to, and the last packet it received. String-formatted messages like `"Connection lost"` carry none of this context. We need structured, machine-parseable events with typed fields that downstream tooling (Grafana, Datadog, custom dashboards) can ingest.

## Decision Drivers

- **Library-safe**: must never install a global subscriber or call any initialization function — the user's process, the user's subscriber
- **Structured key-value data**: every event carries typed fields (bot name, server address, packet ID, chunk coordinates) not just formatted strings
- **Span-based lifecycle tracing**: a bot's connection lifecycle — TCP connect → handshake → login → play → disconnect — should be a span tree that users can trace end-to-end
- **Zero-cost when disabled**: `trace!` and `debug!` events in hot paths (packet decode, world state updates) must compile to a no-op when the subscriber filters them out
- **Per-target filtering**: users must be able to set `headlesscraft_protocol=debug,headlesscraft=info` independently via `RUST_LOG` or programmatic configuration
- **Ecosystem alignment**: Tokio, reqwest, and other dependencies already emit `tracing` spans — using the same framework gives users unified observability for free

## Considered Options

### Option 1: `tracing` crate (structured spans + events)

The `tracing` crate provides structured, span-based instrumentation. Events carry typed key-value pairs: `info!(bot = %name, server = %addr, "connected")`. Spans represent scoped operations that propagate across async `.await` boundaries. The crate is designed for libraries — it emits events into a global dispatcher that the *application* configures. If no subscriber is installed, all events are silently discarded with zero overhead (the callsite check short-circuits on a cached `AtomicBool`). `tracing` is the standard for async Rust: Tokio, hyper, reqwest, tower, and sqlx all use it. Users get unified tracing across their entire dependency tree.

### Option 2: `log` crate (simple facade)

The `log` crate is Rust's original logging facade. It provides `info!`, `warn!`, `error!` macros and a `Log` trait that backends implement. It is library-safe (no global init required) and widely supported. However, `log` only supports unstructured string messages — no spans, no typed fields, no async context propagation. A `log::info!("bot {} connected to {}", name, addr)` message is a flat string that cannot be programmatically decomposed. Correlating events across a multi-bot process requires manual correlation IDs. The `tracing` crate provides a compatibility layer (`tracing-log`) that forwards `log` events into `tracing` subscribers, but not the reverse — choosing `log` means we lose spans entirely.

### Option 3: No logging — return errors only

Emit no log events at all. All information flows through return types: `Result<T, E>` for errors, typed event structs for notifications. This is the purest library approach — zero side effects, zero global state. However, it fails for diagnostic scenarios: when a connection hangs during the login handshake, there is no error to return (the future is still pending). Users need visibility into in-progress operations — "sent handshake, waiting for encryption request" — that cannot be expressed as return values. It also makes debugging opaque: without instrumentation, users must attach a debugger or add `println!` to our source to understand internal behavior.

## Decision

**We adopt the `tracing` crate for all instrumentation. `tracing-subscriber` is a dev-dependency only, used in tests and examples. The library never installs a subscriber.**

### Library Contract

HeadlessCraft emits `tracing` spans and events. The user's application installs a subscriber of their choice. If no subscriber is installed, all instrumentation is silently discarded at near-zero cost.

```rust
// User's main.rs — THEY choose the subscriber
use tracing_subscriber::{fmt, EnvFilter};

fn main() {
    // User installs their own subscriber
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(
            "headlesscraft=debug,headlesscraft_protocol=trace"
        ))
        .init();

    // Now all HeadlessCraft tracing events flow to their subscriber
    // ...
}
```

### Span Hierarchy Design

Every significant lifecycle in HeadlessCraft is wrapped in a span. Spans nest to form a tree that users can visualize, filter, or export to distributed tracing backends:

```
client{bot="farmer_01"}                                (per-client root span)
├── connect{server="mc.example.com:25565"}             (connection attempt)
│   ├── handshake                                      (protocol state)
│   │   └── send_packet{id=0x00, state="handshaking"}
│   ├── login{username="farmer_01"}                    (login sequence)
│   │   ├── send_packet{id=0x00, state="login"}
│   │   ├── encryption_setup                           (if online mode)
│   │   ├── compression_setup{threshold=256}
│   │   └── recv_packet{id=0x02, state="login"}
│   ├── configuration                                  (post-login config)
│   │   ├── recv_registry_data
│   │   └── send_finish_configuration
│   └── play                                           (main gameplay — long-lived)
│       ├── recv_packet{id=0x27}                       (chunk data)
│       ├── world_update{chunks_loaded=42}
│       └── keepalive{id=1234567890}
├── auth{provider="microsoft"}                         (authentication span)
│   ├── oauth_token_request
│   └── xbox_live_auth
└── disconnect{reason="server_shutdown"}
```

### Instrumentation Patterns

Public async functions use `#[instrument]` with field selection:

```rust
use tracing::{debug, info, instrument, warn};

impl Client {
    /// Connects to a Minecraft server.
    #[instrument(skip(self), fields(server = %address))]
    pub async fn connect(&self, address: &ServerAddress) -> Result<Session, ClientError> {
        info!("initiating connection");

        let stream = TcpStream::connect(address.to_socket_addr()).await
            .map_err(|e| {
                warn!(error = %e, "TCP connection failed");
                ClientError::ConnectionFailed { reason: e.to_string() }
            })?;

        debug!(local_addr = %stream.local_addr()?, "TCP connected");
        // ...
    }
}
```

Hot-path operations use `trace!` level to avoid noise at default log levels:

```rust
use tracing::trace;

fn decode_packet(buf: &mut BytesMut) -> Result<RawPacket, ProtocolError> {
    let length = VarInt::decode(buf)?;
    trace!(packet_length = length.0, remaining = buf.remaining(), "decoding packet frame");

    let id = VarInt::decode(buf)?;
    trace!(packet_id = id.0, "decoded packet ID");
    // ...
}
```

### Event Level Guidelines

| Level | Use for | Example |
|-------|---------|---------|
| `error!` | Unrecoverable failures, data corruption | `"failed to decrypt packet: key mismatch"` |
| `warn!` | Recoverable issues, degraded behavior | `"keepalive timeout, reconnecting"` |
| `info!` | Significant lifecycle events | `"connected to server"`, `"authenticated"` |
| `debug!` | Detailed operational flow | `"received chunk at [4, -7]"`, `"inventory updated"` |
| `trace!` | Per-packet, per-frame internals | `"decoded VarInt: 0x2F"`, `"read 1024 bytes"` |

### Testing with tracing

Tests use `tracing-subscriber` (dev-dependency) with a test-friendly configuration:

```rust
#[cfg(test)]
mod tests {
    use tracing_subscriber::fmt::format::FmtSpan;

    fn init_test_tracing() {
        let _ = tracing_subscriber::fmt()
            .with_test_writer()
            .with_span_events(FmtSpan::CLOSE)
            .with_env_filter("trace")
            .try_init();
    }

    #[tokio::test]
    async fn test_connection_lifecycle() {
        init_test_tracing();
        // tracing output captured by test harness
    }
}
```

The `let _ =` pattern silences the `SetGlobalDefaultError` that occurs when multiple tests try to initialize the subscriber — only the first one succeeds, and subsequent calls are harmlessly ignored.

## Consequences

### Positive

- Library-safe by design: HeadlessCraft never touches the global subscriber — users have full control over their observability stack
- Span hierarchy provides end-to-end tracing of bot lifecycles from connection to disconnect, even across async task boundaries
- Zero-cost filtering: `trace!` events in packet decode paths are skipped at the callsite level when the subscriber filters them — no string formatting, no allocation
- Ecosystem synergy: Tokio, reqwest, and rustls already emit `tracing` spans — users see unified traces across the entire stack
- Per-target filtering lets operators debug one subsystem (`headlesscraft_protocol=trace`) without drowning in noise from others

### Negative

- `tracing` spans in async code require `#[instrument]` or manual `Instrument` combinators to propagate correctly across `.await` points — easy to forget, causing orphaned events
- Users unfamiliar with `tracing` may not realize they need to install a subscriber to see any output — first-use confusion (mitigated by documentation and examples)
- Structured fields add verbosity to instrumentation call sites compared to simple `println!`-style logging

### Neutral

- `tracing` also provides a compatibility layer with the `log` crate (`tracing-log`) — users who prefer the `log` ecosystem can bridge events
- If users install no subscriber, all instrumentation is silently discarded — the library works identically, just without observability
- `tracing-subscriber` in dev-deps means our test output format may differ from the user's production format — this is by design

## Compliance

- **No subscriber initialization in library code**: CI grep check ensures `tracing_subscriber::init()`, `tracing_subscriber::fmt().init()`, `set_global_default`, and `env_logger::init()` do not appear outside of `#[cfg(test)]` blocks and example files
- **No `println!` / `eprintln!`**: workspace Clippy lints deny `clippy::print_stdout` and `clippy::print_stderr` in non-test code — all output goes through `tracing`
- **Structured fields required**: code review rejects format-string-only events like `info!("connected to {addr}")` — must use `info!(server = %addr, "connected")`
- **Hot path audit**: any event inside a function called per-packet or per-tick must use `trace!` level — `debug!` or higher in a per-packet function is flagged in review
- **`#[instrument]` on public async functions**: code review checks that public async methods in `headlesscraft` use `#[instrument(skip_all)]` or `#[instrument(skip(self), fields(...))]`

## Related ADRs

- [ADR-001: Crate Architecture](adr-001-crate-architecture.md) — `tracing` is a dependency of both `headlesscraft-protocol` and `headlesscraft`; `tracing-subscriber` is dev-only
- [ADR-002: Error Handling Strategy](adr-002-error-handling.md) — errors are logged as `warn!`/`error!` events with structured fields before being returned
- [ADR-003: Async Runtime Selection](adr-003-async-runtime.md) — Tokio's internal spans are captured by the same subscriber

## References

- [tracing crate documentation](https://docs.rs/tracing/latest/tracing/)
- [tracing-subscriber documentation](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/)
- [tracing — Library usage guidance](https://docs.rs/tracing/latest/tracing/#in-libraries)
- [Tokio tracing integration](https://tokio.rs/tokio/topics/tracing)
- [Eliza Weisman — tracing best practices](https://docs.rs/tracing/latest/tracing/#best-practices)
