# ADR-003: Async Runtime Selection

| Field | Value |
|-------|-------|
| Status | Accepted |
| Date | 2026-03-26 |
| Phases | P02, P04, P12, P17 |
| Deciders | HeadlessCraft Core Team |

## Context

HeadlessCraft is a library for headless Minecraft clients. A single user process may run one bot or hundreds simultaneously — load-testing a server with 200 concurrent clients is a core use case. Each client maintains a persistent TCP connection to a Minecraft server, continuously reading incoming packets (chunk data, entity updates, chat) and writing outgoing packets (movement, block actions, keepalive responses). This is fundamentally an I/O-bound workload with bursty CPU needs (decompressing chunk data, processing entity updates).

Rust's async model requires an executor to poll futures. Unlike Go (goroutines built into the runtime) or Java (virtual threads in Project Loom), Rust does not bundle an async runtime — the application or library must choose one. For a library, this choice has downstream implications: users who depend on HeadlessCraft must use a compatible runtime in their `main()`. Choosing an uncommon or custom runtime forces users into an unfamiliar ecosystem and limits interoperability with other async libraries.

The multi-client scenario is the key design constraint. Running 200 bots with blocking threads would require 400+ OS threads (read + write per connection), consuming ~8MB of stack per thread (800MB total) and creating excessive OS scheduler overhead. An async runtime with an M:N scheduler — many tasks on few OS threads — is essential. We also need `spawn_blocking` for CPU-heavy operations (zlib decompression of chunk data, pathfinding computations) to avoid starving the I/O reactor.

## Decision Drivers

- **Multi-client scalability**: must efficiently handle 100+ concurrent bot connections in a single process with minimal memory and CPU overhead per connection
- **Ecosystem compatibility**: authentication (reqwest), TLS (tokio-rustls/rustls), and other MC-adjacent libraries must work without compatibility shims
- **Work-stealing scheduler**: bot connections have uneven workloads (some in loaded chunks, some idle) — work-stealing prevents thread starvation
- **Blocking task escape hatch**: chunk decompression and pathfinding must not block the async reactor
- **Library ergonomics**: users should be able to use `#[tokio::main]` (the most common async entry point in Rust) without surprises
- **Production maturity**: the runtime must be proven at scale — Discord, Cloudflare, AWS all run Tokio in production

## Considered Options

### Option 1: Tokio

Tokio is the de facto standard async runtime for Rust. It provides a multi-threaded work-stealing scheduler, `spawn_blocking` for CPU-bound offloading, and comprehensive I/O primitives (TCP, UDP, timers, channels). Its ecosystem is unmatched: `reqwest` (HTTP client for Mojang/Microsoft authentication), `tokio-rustls` (TLS for online-mode servers), `tokio-tungstenite` (WebSocket, useful for Realms API), and the entire `tower` middleware stack all require Tokio. The trade-off is a larger dependency tree (~30 transitive deps) and the fact that choosing Tokio means users must also use Tokio — but this is increasingly a non-issue as Tokio has become the ecosystem default.

### Option 2: async-std

async-std provides async equivalents of the Rust standard library API. It uses a simpler global executor and has a gentler learning curve. However, its ecosystem has stagnated since 2022 — critical libraries like `reqwest` and `hyper` do not support async-std natively. Authentication with Microsoft (OAuth2 flow via HTTPS) would require either a compatibility shim (`async-compat`) or a bespoke HTTP client. The community has largely consolidated around Tokio, and new async libraries are Tokio-first or Tokio-only. Choosing async-std would isolate HeadlessCraft from the mainstream Rust async ecosystem.

### Option 3: Runtime-agnostic with trait abstraction

Define our own `Runtime` trait abstracting over `spawn`, `sleep`, `TcpStream`, etc., and let users plug in any runtime. This is the most flexible approach — no lock-in. However, the implementation cost is enormous: every I/O type must be generic over the runtime, every spawn call goes through a trait method, and we lose access to runtime-specific features (Tokio's `spawn_blocking`, `JoinSet`, `CancellationToken`). Libraries like `reqwest` still pin to Tokio internally, so the abstraction leaks. The `async-trait` overhead and type complexity make the public API harder to use. Projects that have attempted this (e.g., `sqlx`'s multi-runtime support) eventually deprecated non-Tokio backends due to maintenance burden.

## Decision

**We adopt Tokio as the async runtime for HeadlessCraft.** The main `headlesscraft` crate depends on Tokio with the feature set `rt-multi-thread, net, io-util, time, sync, macros`. The protocol crate (`headlesscraft-protocol`) does **not** depend on Tokio — its codec operations are synchronous on `bytes::Bytes` / `bytes::BytesMut`.

### Library API Design

HeadlessCraft exposes an async API. Users must run their code inside a Tokio runtime:

```rust
use headlesscraft::{Client, ServerAddress};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = Client::builder()
        .authentication(auth_token)
        .build();

    let session = client.connect(&"mc.example.com:25565".parse()?).await?;

    // Bot logic runs as async tasks
    session.on_chat(|msg| async move {
        println!("Chat: {msg}");
    });

    session.wait_for_disconnect().await?;
    Ok(())
}
```

### Multi-Client Architecture

For the 100+ bot scenario, each client connection spawns two Tokio tasks (reader + writer) communicating via bounded `mpsc` channels. Tokio's work-stealing scheduler distributes these tasks across OS threads automatically:

```rust
use headlesscraft::ClientPool;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let pool = ClientPool::builder()
        .max_concurrent(200)
        .build();

    // Spawn 200 bot connections — Tokio schedules them across CPU cores
    for i in 0..200 {
        let bot = pool.spawn_bot(format!("bot_{i}"), server_addr.clone()).await?;
        tokio::spawn(async move {
            bot.run_behavior(wander_and_mine).await;
        });
    }

    pool.wait_all().await;
    Ok(())
}
```

### Blocking Task Policy

CPU-intensive operations (zlib decompression of chunk data, NBT parsing of large payloads, pathfinding) use `tokio::task::spawn_blocking` to avoid starving the async reactor:

```rust
let decompressed = tokio::task::spawn_blocking(move || {
    zlib_decompress(&compressed_chunk_data)
}).await??;
```

### Protocol Crate Independence

`headlesscraft-protocol` remains runtime-agnostic by design (see [ADR-001](adr-001-crate-architecture.md)). Codec operations take `&[u8]` or `bytes::Bytes` as input and return `bytes::BytesMut` as output — no async, no I/O. This allows protocol-only consumers (proxies, analyzers) to use any runtime or no runtime at all.

## Consequences

### Positive

- Access to the largest Rust async ecosystem — reqwest, tokio-rustls, tower, and all Tokio-native libraries work without shims
- Work-stealing scheduler automatically balances 200+ bot connections across CPU cores without manual thread assignment
- `spawn_blocking` cleanly separates CPU-heavy chunk decompression from latency-sensitive packet I/O
- `#[tokio::main]` is already the most common async entry point — users are not surprised by the requirement
- `tokio::sync::mpsc` with backpressure prevents fast readers from overwhelming slow processors

### Negative

- Users are locked into Tokio — incompatible with `async-std` or `smol` without a compatibility layer
- Adds ~30 transitive dependencies to downstream builds, increasing compile times by 10-15 seconds on first build
- Async Rust has a steeper learning curve (pinning, `Send`/`Sync` bounds, cancellation semantics) — bot developers must understand async concepts
- `spawn_blocking` has overhead (thread pool scheduling) — must be used judiciously, not for every small computation

### Neutral

- Tokio's cancellation semantics (drop = cancel) mean that disconnecting a client automatically cleans up its spawned tasks — convenient but requires care with cleanup logic
- We commit to Tokio's `JoinHandle` / `JoinSet` patterns for managing multi-client task lifecycles
- `tokio::test` simplifies async unit testing but requires all test authors to be aware of the async context

## Compliance

- **Workspace dependency**: Tokio is declared in `[workspace.dependencies]` with explicit feature flags — no `features = ["full"]` to avoid pulling unnecessary features
- **Protocol crate isolation**: CI runs `cargo check -p headlesscraft-protocol` and verifies Tokio is not in its dependency tree (`cargo tree -p headlesscraft-protocol | grep tokio` must produce no output)
- **No blocking in async context**: code review flags any `std::thread::sleep`, blocking `std::fs` calls, or synchronous HTTP requests inside async functions
- **spawn_blocking documentation**: any call to `spawn_blocking` must include a comment explaining why the operation is blocking

## Related ADRs

- [ADR-001: Crate Architecture](adr-001-crate-architecture.md) — Tokio is confined to `headlesscraft`, not `headlesscraft-protocol`
- [ADR-002: Error Handling Strategy](adr-002-error-handling.md) — async errors are propagated via typed `Result` enums, not panics
- [ADR-004: Logging & Observability](adr-004-logging-observability.md) — Tokio instruments its internals with `tracing` spans, which our subscriber captures

## References

- [Tokio documentation](https://docs.rs/tokio/latest/tokio/)
- [Tokio tutorial — spawning](https://tokio.rs/tokio/tutorial/spawning)
- [Tokio work-stealing scheduler internals](https://tokio.rs/blog/2019-10-scheduler)
- [Alice Ryhl — "Actors with Tokio"](https://ryhl.io/blog/actors-with-tokio/)
- [Discord — "Why Discord is switching from Go to Rust"](https://discord.com/blog/why-discord-is-switching-from-go-to-rust)
- [reqwest — Tokio-based HTTP client](https://docs.rs/reqwest/latest/reqwest/)
