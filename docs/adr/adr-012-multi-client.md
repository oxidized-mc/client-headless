# ADR-012: Multi-Client Architecture

| Field | Value |
|-------|-------|
| Status | Accepted |
| Date | 2026-03-26 |
| Phases | P17 |
| Deciders | HeadlessCraft Core Team |

## Context

A primary use case for HeadlessCraft is running many bot clients concurrently:
stress testing a server with 100+ simultaneous connections, coordinating bot
swarms for game automation, or simulating realistic player populations. Each
client needs its own TCP connection, authentication session, world state, and
event handler — they are logically independent players.

However, these clients share common infrastructure: a tokio runtime, DNS
resolution, the HTTP client used for authentication, and immutable game data
like the block state registry and dimension type definitions. Naively creating
100 fully independent `Client` instances would duplicate this shared data and
potentially exhaust system resources (thread pools, file descriptors, DNS
caches).

The challenge is finding the right boundary between shared resources (for
efficiency) and isolated state (for correctness). Two bots on the same server
see different chunks depending on their position, so world state must be
per-client. But the block registry that maps state ID 1234 to `minecraft:stone`
is identical for all clients connecting to the same server version.

## Decision Drivers

- Must support 100+ concurrent clients on a single machine
- Must share expensive immutable resources (registries, runtime, HTTP client)
- Must fully isolate mutable per-client state (world, entities, connection)
- Must not require users to manually manage shared resources
- Must support mixed server targets (some bots on server A, others on server B)
- Should bound resource usage (connections, memory) with configurable limits
- Should provide simple API for common "N bots on one server" pattern

## Considered Options

### Option 1: Fully Isolated (Each Client Is Independent)

Each `Client` is a standalone unit with its own tokio runtime, HTTP client, and
data. Users manage concurrency themselves.

**Pros:** Simplest implementation. No shared state to reason about. Clients
are trivially independent.
**Cons:** 100 clients = 100 HTTP clients, 100 copies of block registry (~2 MB
each), 100 thread pools. Wasteful. Hard for users to coordinate swarms.

### Option 2: Shared Runtime with Isolated State

Clients share a tokio runtime and HTTP client but each has its own world state.
No sharing of game data.

**Pros:** Efficient runtime usage. Better than Option 1 for resource usage.
**Cons:** Still duplicates registries. No API for coordinating multiple clients.

### Option 3: Shared Runtime + Shared Immutable Data (Registries, Block Defs)

A `ClientPool` owns shared resources. Clients spawned from the pool inherit
shared immutable data via `Arc` and run on the shared runtime. Mutable state
remains per-client.

**Pros:** Minimal resource duplication. Clean API for swarm management.
Configurable limits. Shared data is `Arc` — zero cost after initialization.
**Cons:** Pool adds API surface. Users must understand which data is shared.

## Decision

**Option 3: Shared runtime + shared immutable data via `ClientPool`.**

### Resource Sharing Model

```text
┌───────────────── ClientPool ─────────────────┐
│                                               │
│  ┌─────────────────────────────────────────┐  │
│  │          Shared Resources               │  │
│  │  • tokio Runtime (multi-threaded)       │  │
│  │  • reqwest::Client (connection pool)    │  │
│  │  • Arc<BlockRegistry>                   │  │
│  │  • Arc<DimensionRegistry>               │  │
│  │  • Arc<ProtocolCodecs>                  │  │
│  └─────────────────────────────────────────┘  │
│                                               │
│  ┌─────────┐  ┌─────────┐      ┌─────────┐   │
│  │Client #1│  │Client #2│ ···  │Client #N│   │
│  │         │  │         │      │         │   │
│  │• TcpConn│  │• TcpConn│      │• TcpConn│   │
│  │• Auth   │  │• Auth   │      │• Auth   │   │
│  │• World  │  │• World  │      │• World  │   │
│  │• Handler│  │• Handler│      │• Handler│   │
│  │• Reader │  │• Reader │      │• Reader │   │
│  │• Writer │  │• Writer │      │• Writer │   │
│  └─────────┘  └─────────┘      └─────────┘   │
│                                               │
└───────────────────────────────────────────────┘
```

### Per-Client Task Model

Each client runs as exactly 2 tokio tasks:

- **Reader task:** reads from TCP, decodes packets, updates world state,
  dispatches events to handler
- **Writer task:** receives outbound packets from `mpsc`, encodes and writes
  to TCP

100 clients = 200 tasks. Tokio handles thousands of tasks efficiently — this is
well within capacity. Each task is lightweight (no dedicated OS thread).

An additional handler task per client runs the `EventHandler` (ADR-011), making
it 3 tasks per client in practice. 100 clients = 300 tasks.

### ClientPool API

```rust
use headlesscraft::{ClientPool, PoolConfig, Client, OfflineAuth};

/// Configuration for a client pool.
pub struct PoolConfig {
    /// Maximum number of concurrent clients (default: 100).
    pub max_clients: usize,
    /// Connection rate limit: max new connections per second (default: 10).
    pub connect_rate_limit: u32,
    /// Shared HTTP client configuration for authentication.
    pub http_config: HttpConfig,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_clients: 100,
            connect_rate_limit: 10,
            http_config: HttpConfig::default(),
        }
    }
}

/// Manages multiple clients with shared resources.
pub struct ClientPool {
    config: PoolConfig,
    shared: Arc<SharedResources>,
    clients: DashMap<ClientId, ClientHandle, ahash::RandomState>,
}

impl ClientPool {
    /// Create a new pool with default configuration.
    pub fn new() -> Self {
        Self::with_config(PoolConfig::default())
    }

    /// Create a new pool with custom configuration.
    pub fn with_config(config: PoolConfig) -> Self { /* ... */ }

    /// Spawn a new client in the pool. Returns a handle for control.
    pub async fn spawn(
        &self,
        server: &str,
        auth: impl Authenticator,
        handler: impl EventHandler,
    ) -> Result<ClientHandle, PoolError> { /* ... */ }

    /// Number of currently active clients.
    pub fn active_count(&self) -> usize {
        self.clients.len()
    }

    /// Disconnect all clients gracefully.
    pub async fn disconnect_all(&self) {
        // ...
    }

    /// Wait until all clients have disconnected.
    pub async fn wait_all(&self) {
        // ...
    }
}

/// Handle to a client within the pool.
pub struct ClientHandle {
    id: ClientId,
    client: Arc<Client>,
}

impl ClientHandle {
    pub fn id(&self) -> ClientId { self.id }

    /// Access the underlying client for sending packets, querying state, etc.
    pub fn client(&self) -> &Client { &self.client }

    /// Disconnect this client.
    pub async fn disconnect(&self) -> Result<(), ConnectionError> {
        self.client.disconnect().await
    }
}
```

### Usage Example: Bot Swarm

```rust
use headlesscraft::{ClientPool, PoolConfig, OfflineAuth};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let pool = ClientPool::with_config(PoolConfig {
        max_clients: 50,
        connect_rate_limit: 5, // 5 connections/second to avoid server throttle
        ..Default::default()
    });

    // Spawn 50 bots with staggered connections
    for i in 0..50 {
        let name = format!("Bot_{i:03}");
        let bot = WanderBot::new(&name);

        pool.spawn("mc.example.com:25565", OfflineAuth::new(&name), bot).await?;
    }

    tracing::info!("All {} bots connected", pool.active_count());

    // Wait for Ctrl+C, then disconnect all
    tokio::signal::ctrl_c().await?;
    pool.disconnect_all().await;

    Ok(())
}
```

### Single-Client API (No Pool Required)

For users who only need one client, the standalone `Client::builder()` API
works without any pool:

```rust
let client = Client::builder()
    .server_address("localhost:25565")
    .authenticator(OfflineAuth::new("SingleBot"))
    .event_handler(MyHandler)
    .build()
    .await?;
```

Internally, a single client creates its own lightweight shared resources. There
is no performance penalty for not using a pool with one client.

### Capacity Estimates

| Clients | Tokio Tasks | Memory (world state) | Memory (shared) | TCP FDs |
|---------|-------------|----------------------|-----------------|---------|
| 1       | 3           | ~5 MB                | ~3 MB           | 1       |
| 10      | 30          | ~50 MB               | ~3 MB           | 10      |
| 50      | 150         | ~250 MB              | ~3 MB           | 50      |
| 100     | 300         | ~500 MB              | ~3 MB           | 100     |
| 500     | 1,500       | ~2.5 GB              | ~3 MB           | 500     |

World state per client assumes ~200 loaded chunks (typical render distance 8).
Actual memory varies with chunk content and entity count. Shared resources
(registries, codecs, HTTP client) are constant regardless of client count.

### Shared vs. Per-Client Data

| Data | Ownership | Shared via |
|------|-----------|------------|
| Block state registry | Pool | `Arc<BlockRegistry>` |
| Dimension definitions | Pool | `Arc<DimensionRegistry>` |
| Protocol codec tables | Pool | `Arc<ProtocolCodecs>` |
| HTTP client | Pool | `Arc<reqwest::Client>` |
| Tokio runtime | Pool | Implicit (tasks spawned on pool's runtime) |
| TCP connection | Client | Owned |
| World state (chunks, entities) | Client | Owned |
| Auth session/tokens | Client | Owned |
| Event handler | Client | Owned |

## Consequences

### Positive

- Shared immutable data saves ~2 MB × N memory for large swarms
- Single tokio runtime avoids thread pool explosion
- Rate limiting prevents accidental server DDoS
- `ClientHandle` provides per-client control within the swarm
- Pool API is optional — single-client use case is unaffected
- Connection count bounded by configurable limit

### Negative

- Pool adds API surface and concepts (pool vs. client vs. handle)
- Shared `reqwest::Client` means auth failures in one client could affect
  connection pool state for others (mitigated by reqwest's internal isolation)
- Users must increase `ulimit -n` for 500+ clients on Linux

### Neutral

- `ClientPool` does not implement reconnection logic — users can re-spawn a
  disconnected client manually or via their `on_disconnect` handler
- Pool does not coordinate between clients (e.g., shared chat) — that's
  user-level logic built on top of individual `ClientHandle` references
- Different server targets are supported — a pool can have bots on multiple
  servers, but registry data is only shared among bots on the same version

## Compliance

- TCP connection handling follows Minecraft protocol handshake spec
- Rate limiting respects typical server connection throttle defaults
- Task model aligns with tokio best practices for I/O-bound workloads

## Related ADRs

- ADR-009: Authentication Flow (shared HTTP client for Microsoft auth)
- ADR-010: Client World State (per-client WorldState, shared registries)
- ADR-011: Event & Handler System (per-client event handler)
- ADR-013: Testing Strategy (multi-client integration tests)

## References

- [Tokio task documentation](https://docs.rs/tokio/latest/tokio/task/)
- [DashMap concurrent HashMap](https://docs.rs/dashmap/)
- [reqwest connection pooling](https://docs.rs/reqwest/latest/reqwest/struct.Client.html)
- [Linux file descriptor limits](https://man7.org/linux/man-pages/man2/getrlimit.2.html)
