# ADR-011: Event & Handler System

| Field | Value |
|-------|-------|
| Status | Accepted |
| Date | 2026-03-26 |
| Phases | P07, P11, P12 |
| Deciders | HeadlessCraft Core Team |

## Context

Bot logic in HeadlessCraft is fundamentally *reactive*: bots respond to game
events such as chat messages, entity spawns, chunk loads, health changes, and
disconnections. The framework must provide an API that lets users write this
reactive behavior ergonomically. Users range from beginners writing a simple
chat-reply bot to advanced users building complex pathfinding swarms.

The key tension is between simplicity, flexibility, and performance. A simple
callback API is easy to learn but hard to compose. A channel-based API gives
users full control but requires them to write their own dispatch loop. A trait
with default methods offers the best type safety and documentation but requires
understanding Rust traits and async.

Since HeadlessCraft is a library (not a framework), we cannot dictate the user's
runtime or architecture. The event system must integrate cleanly with any async
Rust application. Events are dispatched from the network reader task, so the
handler must not block the reader — long-running bot logic must be spawned
separately or use async `.await` points.

## Decision Drivers

- Must be ergonomic for simple bots (< 50 lines of user code)
- Must support complex bots with state machines and multi-event logic
- Must not block the network reader task
- Must support both "react to events" and "poll for events" patterns
- Should provide compile-time event type safety
- Should allow users to hold mutable state in their handler
- Must work with `tokio` async runtime

## Considered Options

### Option 1: Callback Closures Registered Per Event Type

```rust
client.on_chat(|msg| async { /* ... */ });
client.on_entity_spawn(|entity| async { /* ... */ });
```

**Pros:** Familiar pattern. No trait boilerplate. Easy for single-event bots.
**Cons:** Hard to share state between callbacks without `Arc<Mutex<_>>`.
Closures fight the borrow checker. No single place to see all handled events.
Type-erased dispatch adds overhead.

### Option 2: Async Channel-Based (mpsc Receiver)

```rust
let (client, mut events) = Client::connect(opts).await?;
while let Some(event) = events.recv().await {
    match event { Event::Chat(msg) => { /* ... */ }, _ => {} }
}
```

**Pros:** Full user control. Natural for poll-style bots. Easy to integrate
with `tokio::select!`. No trait boilerplate.
**Cons:** Every event is boxed and sent through the channel — even unhandled
ones. Forces users to write a match-all dispatch loop. No compile-time
exhaustiveness checking (new events silently ignored). Harder to document
"what events exist" compared to trait methods.

### Option 3: Trait-Based Handler (`impl EventHandler for MyBot`)

```rust
struct MyBot;
#[async_trait]
impl EventHandler for MyBot {
    async fn on_chat_message(&mut self, msg: ChatMessage) { /* ... */ }
}
```

**Pros:** Self-documenting — trait shows all events. Default no-ops mean users
implement only what they need. Handler owns its state (`&mut self`). IDE
auto-complete shows available events. Easy to test (call methods directly).
**Cons:** Requires `async_trait` (or RPITIT). One handler per client — no
composing multiple handlers easily. Unfamiliar to users from callback-heavy
ecosystems.

### Option 4: Hybrid — Trait-Based with Channel Fallback

Offer the trait as the primary API, plus an `EventChannel` adapter that
implements the trait by forwarding everything to an `mpsc` channel.

**Pros:** Best of both worlds. Power users get the trait; simple scripts get
the channel. Same internal dispatch regardless.
**Cons:** Two APIs to document. Slightly more surface area.

## Decision

**Option 4: Hybrid — trait-based handler with channel fallback.**

### EventHandler Trait

```rust
/// Trait for handling game events. Implement only the methods you care about.
/// All methods have default no-op implementations.
///
/// The handler is called on a dedicated tokio task. Async operations are
/// supported — use `.await` freely. The handler receives `&mut self`, so
/// it can own and mutate state without external synchronization.
#[async_trait]
pub trait EventHandler: Send + 'static {
    /// Called when a chat message is received.
    async fn on_chat_message(&mut self, _event: ChatMessageEvent) {}

    /// Called when an entity is spawned within tracking range.
    async fn on_entity_spawn(&mut self, _event: EntitySpawnEvent) {}

    /// Called when an entity is removed from tracking.
    async fn on_entity_despawn(&mut self, _event: EntityDespawnEvent) {}

    /// Called when an entity's position or rotation changes.
    async fn on_entity_move(&mut self, _event: EntityMoveEvent) {}

    /// Called when a chunk column is loaded or updated.
    async fn on_chunk_load(&mut self, _event: ChunkLoadEvent) {}

    /// Called when a chunk column is unloaded.
    async fn on_chunk_unload(&mut self, _event: ChunkUnloadEvent) {}

    /// Called when the client's health, food, or saturation changes.
    async fn on_health_update(&mut self, _event: HealthUpdateEvent) {}

    /// Called when the client's position is corrected by the server.
    async fn on_teleport(&mut self, _event: TeleportEvent) {}

    /// Called when a player joins or leaves the player list.
    async fn on_player_list_update(&mut self, _event: PlayerListUpdateEvent) {}

    /// Called when the client changes dimension (respawn/portal).
    async fn on_dimension_change(&mut self, _event: DimensionChangeEvent) {}

    /// Called when the client dies.
    async fn on_death(&mut self, _event: DeathEvent) {}

    /// Called when the connection is lost or the server kicks the client.
    async fn on_disconnect(&mut self, _event: DisconnectEvent) {}

    /// Called every client tick (~50ms) after all packet events are processed.
    async fn on_tick(&mut self, _event: TickEvent) {}
}
```

### Event Types

```rust
/// Chat message received from the server.
pub struct ChatMessageEvent {
    pub sender: Option<Uuid>,
    pub content: String,
    pub position: ChatPosition,
    pub timestamp: i64,
}

/// Entity appeared in tracking range.
pub struct EntitySpawnEvent {
    pub network_id: i32,
    pub uuid: Uuid,
    pub entity_type: EntityType,
    pub position: glam::DVec3,
    pub rotation: glam::Vec2,
}

/// Client health/food update.
pub struct HealthUpdateEvent {
    pub health: f32,
    pub food: i32,
    pub saturation: f32,
}

/// Connection lost.
pub struct DisconnectEvent {
    pub reason: DisconnectReason,
    pub message: Option<String>,
}

/// Per-tick event with timing information.
pub struct TickEvent {
    pub tick_number: u64,
    pub delta: std::time::Duration,
}
```

### Example: Simple Chat Bot

```rust
use headlesscraft::{Client, EventHandler, OfflineAuth};
use headlesscraft::event::*;

struct EchoBot {
    my_name: String,
}

#[async_trait]
impl EventHandler for EchoBot {
    async fn on_chat_message(&mut self, event: ChatMessageEvent) {
        // Don't echo our own messages
        if event.content.starts_with(&self.my_name) {
            return;
        }
        tracing::info!("Chat: {}", event.content);
    }

    async fn on_health_update(&mut self, event: HealthUpdateEvent) {
        if event.health < 5.0 {
            tracing::warn!("Low health: {:.1}", event.health);
        }
    }

    async fn on_disconnect(&mut self, event: DisconnectEvent) {
        tracing::error!("Disconnected: {:?}", event.reason);
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let bot = EchoBot { my_name: "EchoBot".into() };

    let client = Client::builder()
        .server_address("localhost:25565")
        .authenticator(OfflineAuth::new("EchoBot"))
        .event_handler(bot)
        .build()
        .await?;

    // Run until disconnected
    client.wait_for_disconnect().await?;
    Ok(())
}
```

### Channel Fallback for Poll-Style Bots

```rust
use headlesscraft::{Client, EventChannel, OfflineAuth};
use headlesscraft::event::Event;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let (channel, mut receiver) = EventChannel::new(256); // bounded buffer

    let client = Client::builder()
        .server_address("localhost:25565")
        .authenticator(OfflineAuth::new("PollBot"))
        .event_handler(channel)
        .build()
        .await?;

    // Poll-style event loop — integrates with tokio::select!
    loop {
        tokio::select! {
            Some(event) = receiver.recv() => {
                match event {
                    Event::ChatMessage(msg) => {
                        println!("Chat: {}", msg.content);
                    }
                    Event::Disconnect(_) => break,
                    _ => {}
                }
            }
            _ = tokio::signal::ctrl_c() => {
                client.disconnect().await?;
                break;
            }
        }
    }
    Ok(())
}
```

### Internal Dispatch

The client's network reader task decodes packets and produces `Event` values.
These are sent to a handler task via a bounded `mpsc` channel. The handler task
calls the appropriate `EventHandler` method for each event. This ensures the
network reader is never blocked by slow handler logic.

```text
┌─────────────┐  mpsc(256)  ┌──────────────┐
│ Net Reader  │────────────▸│ Handler Task │──▸ on_chat_message()
│ (decode     │             │ (calls trait │──▸ on_entity_spawn()
│  packets)   │             │  methods)    │──▸ on_tick()
└─────────────┘             └──────────────┘
```

If the handler channel is full (handler too slow), the oldest event is dropped
and a warning is logged. This prevents a slow bot from causing back-pressure on
the network reader, which would stall the TCP connection.

## Consequences

### Positive

- Trait with default methods: users implement only what they need
- `&mut self` gives handlers natural ownership of their state
- Channel fallback supports `tokio::select!` and poll-style patterns
- Handler runs on its own task — never blocks the network reader
- Events are strongly typed — IDE auto-complete and exhaustive matching
- Easy to test: construct events, call handler methods, assert state

### Negative

- `async_trait` adds one heap allocation per handler method call (negligible
  compared to network I/O)
- Single handler per client — users who want composable behaviors must
  orchestrate within their handler (dispatch table, state machine, etc.)
- Dropped events under back-pressure — acceptable trade-off to keep the
  network connection healthy

### Neutral

- Adding new events requires a new trait method with a default no-op — this is
  backward-compatible (existing handlers compile without changes)
- `EventChannel` implements `EventHandler` internally — it is not a separate
  dispatch path, just a forwarding adapter
- `on_tick` is called at the client's tick rate (~20 Hz), not the server's

## Compliance

- Event types map 1:1 to protocol packets where applicable
- Chat handling respects the signed message system (protocol 26.1)
- Entity events match vanilla `ClientPacketListener` callback order

## Related ADRs

- ADR-009: Authentication Flow (disconnect → re-auth flow)
- ADR-010: Client World State (events reference world state for context)
- ADR-012: Multi-Client Architecture (each client has its own handler)

## References

- [Tokio mpsc channels](https://docs.rs/tokio/latest/tokio/sync/mpsc/)
- [async_trait crate](https://docs.rs/async-trait/)
- Vanilla source: `ClientPacketListener.java` (packet → event mapping)
- [wiki.vg Protocol](https://wiki.vg/Protocol) — clientbound packet list
