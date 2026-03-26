# ADR-010: Client World State Management

| Field | Value |
|-------|-------|
| Status | Accepted |
| Date | 2026-03-26 |
| Phases | P08, P09, P10 |
| Deciders | HeadlessCraft Core Team |

## Context

A headless Minecraft client must maintain a local replica of the world state it
receives from the server. This includes chunk data (block types, biomes, light
levels), entity tracking (positions, velocities, metadata), the player list
(UUIDs, game profiles, listed status), and dimension/registry information. Unlike
a server implementation, the client never *generates* this data — it exclusively
receives and applies state updates from inbound packets.

Bot logic requires efficient querying of this state: "What block is at
(100, 64, -200)?", "Where is entity #42?", "Which players are online?", "What
biome am I standing in?" These queries must be fast since bots may evaluate
hundreds per tick for pathfinding or combat logic. At the same time, the network
reader task continuously mutates state as packets arrive, so thread safety is
essential.

In a multi-client scenario (ADR-012), each `Client` instance maintains its own
`WorldState`. Two bots on the same server see different chunk sets depending on
their positions, so sharing mutable world state between clients is neither safe
nor useful. Immutable data like the block registry and dimension definitions can
be shared (see ADR-012).

## Decision Drivers

- Read-heavy workload: bot logic reads state far more often than packets write it
- Must handle vanilla chunk format (paletted containers, bit-packed storage)
- Must support concurrent access from network task (writes) and bot task (reads)
- Per-client isolation — no shared mutable state between clients
- Memory-efficient for 100+ concurrent clients (ADR-012)
- Simple query API for bot developers

## Considered Options

### Option 1: Simple HashMap-Based Storage

Store chunks in `HashMap<ChunkPos, Vec<BlockId>>`, entities in
`HashMap<i32, Entity>`. Minimal code, easy to understand.

**Pros:** Simple. Fast to implement.
**Cons:** Wastes memory — no palette compression. Block lookups require manual
index math. No built-in concurrency story. Doesn't match vanilla's chunk format,
making packet parsing more complex.

### Option 2: ECS (bevy_ecs) for Entities + Separate Chunk Storage

Use `bevy_ecs::World` for entity tracking. Chunks in a separate structure.

**Pros:** Powerful query system for entities. Familiar to game developers.
**Cons:** Massive overkill for a *client* that tracks ~100 entities with simple
reads. ECS is designed for server-side system scheduling with thousands of
entities. Adds a heavy dependency. No benefit for chunk storage. Awkward
integration with async network tasks.

### Option 3: Flat Data Structures Optimized for Read-Heavy Access

Purpose-built structs: `ChunkStorage` for block data, `EntityTracker` for
entities, `PlayerList` for online players. Chunk format matches vanilla's
paletted container for zero-copy packet parsing.

**Pros:** Minimal overhead. Matches vanilla wire format. Simple, predictable
performance. Each component tuned for its access pattern.
**Cons:** Must implement chunk palette logic ourselves. No free "system
scheduling" — but we don't need it on the client.

## Decision

**Option 3: Flat data structures optimized for read-heavy client access.**

### WorldState Structure

```rust
/// Per-client snapshot of the world as received from the server.
pub struct WorldState {
    /// Block and biome data, keyed by chunk column position.
    pub chunks: ChunkStorage,
    /// Entities tracked by their network (entity) ID.
    pub entities: EntityTracker,
    /// Online players, keyed by UUID.
    pub players: PlayerList,
    /// Current dimension type and registry information.
    pub dimension: DimensionInfo,
}

/// Thread-safe chunk storage using DashMap for concurrent read/write.
pub struct ChunkStorage {
    columns: DashMap<ChunkPos, ChunkColumn, ahash::RandomState>,
    /// Vertical range for the current dimension (e.g., -64..320 for Overworld).
    min_y: i32,
    section_count: u32,
}

/// A full chunk column (16-wide, full-height, 16-deep).
pub struct ChunkColumn {
    /// Sections stacked vertically, indexed from bottom to top.
    sections: Vec<ChunkSection>,
    /// Heightmaps received from the server.
    heightmaps: Heightmaps,
}

/// A 16×16×16 chunk section.
pub struct ChunkSection {
    /// Block states in paletted container format (matches vanilla wire format).
    block_states: PalettedContainer<BlockState>,
    /// Biomes in paletted container format (4×4×4 resolution).
    biomes: PalettedContainer<BiomeId>,
    /// Non-air block count (used for fast isEmpty checks).
    non_air_count: u16,
}

/// Entity tracked by the client.
pub struct Entity {
    pub network_id: i32,
    pub uuid: Uuid,
    pub entity_type: EntityType,
    pub position: glam::DVec3,
    pub rotation: glam::Vec2,
    pub velocity: glam::Vec3,
    pub on_ground: bool,
    pub metadata: EntityMetadata,
}

/// Simple entity storage keyed by network ID.
pub struct EntityTracker {
    entities: HashMap<i32, Entity, ahash::RandomState>,
    /// Reverse lookup: UUID → network ID.
    uuid_index: HashMap<Uuid, i32, ahash::RandomState>,
}

/// Online player list as received from PlayerInfo packets.
pub struct PlayerList {
    players: HashMap<Uuid, PlayerInfo, ahash::RandomState>,
}

pub struct PlayerInfo {
    pub uuid: Uuid,
    pub username: String,
    pub game_mode: GameMode,
    pub ping: i32,
    pub display_name: Option<String>,
    pub listed: bool,
}
```

### Concurrency Model

```rust
/// Shared handle to a client's world state.
/// The network reader task writes; bot logic reads.
pub type SharedWorldState = Arc<RwLock<WorldState>>;
```

`ChunkStorage` uses `DashMap` internally for fine-grained chunk-level locking —
the network task can load chunk (0, 0) while bot logic reads chunk (1, 1)
without contention. `EntityTracker` and `PlayerList` use standard `HashMap`
behind the outer `RwLock` since entity updates are less frequent and the data set
is small.

### Query API

```rust
impl WorldState {
    /// Get the block state at an absolute world position.
    pub fn block_at(&self, x: i32, y: i32, z: i32) -> Option<BlockState> {
        self.chunks.get_block(x, y, z)
    }

    /// Get the biome at an absolute world position (4×4×4 resolution).
    pub fn biome_at(&self, x: i32, y: i32, z: i32) -> Option<BiomeId> {
        self.chunks.get_biome(x, y, z)
    }

    /// Get an entity by its network ID.
    pub fn entity(&self, network_id: i32) -> Option<&Entity> {
        self.entities.get(network_id)
    }

    /// Get an entity by UUID.
    pub fn entity_by_uuid(&self, uuid: &Uuid) -> Option<&Entity> {
        self.entities.get_by_uuid(uuid)
    }

    /// Iterate all tracked entities.
    pub fn entities(&self) -> impl Iterator<Item = &Entity> {
        self.entities.iter()
    }

    /// Get a player's info by UUID.
    pub fn player(&self, uuid: &Uuid) -> Option<&PlayerInfo> {
        self.players.get(uuid)
    }

    /// All online players.
    pub fn online_players(&self) -> impl Iterator<Item = &PlayerInfo> {
        self.players.iter()
    }
}

impl ChunkStorage {
    /// Get block at absolute world coordinates.
    pub fn get_block(&self, x: i32, y: i32, z: i32) -> Option<BlockState> {
        let chunk_x = x.div_euclid(16);
        let chunk_z = z.div_euclid(16);
        let pos = ChunkPos::new(chunk_x, chunk_z);

        let column = self.columns.get(&pos)?;
        let section_y = ((y - self.min_y) / 16) as usize;
        let section = column.sections.get(section_y)?;

        let local_x = x.rem_euclid(16) as usize;
        let local_y = y.rem_euclid(16) as usize;
        let local_z = z.rem_euclid(16) as usize;
        Some(section.block_states.get(local_x, local_y, local_z))
    }

    /// Check whether a chunk column is loaded.
    pub fn is_loaded(&self, x: i32, z: i32) -> bool {
        self.columns.contains_key(&ChunkPos::new(x, z))
    }
}
```

### Usage in Bot Logic

```rust
async fn my_bot_logic(world: SharedWorldState) {
    let state = world.read();

    // Check block under the bot
    if let Some(block) = state.block_at(100, 63, -200) {
        if block.is_air() {
            tracing::warn!("no ground beneath us!");
        }
    }

    // Find nearby players
    for player in state.online_players() {
        tracing::info!("{} (ping: {}ms)", player.username, player.ping);
    }
}
```

## Consequences

### Positive

- Zero-copy chunk parsing: wire format maps directly to `PalettedContainer`
- `DashMap` allows concurrent chunk reads with minimal contention
- Simple, predictable API — no ECS learning curve for bot developers
- Memory-efficient: paletted containers compress chunk data just like vanilla
- Each data structure is independently testable

### Negative

- Must implement `PalettedContainer` with indirect/direct palettes ourselves
- Outer `RwLock` on `WorldState` means entity/player writes block all reads
  briefly — acceptable given small data sizes and sub-microsecond hold times
- No built-in spatial indexing for entity queries (e.g., "entities within 10
  blocks") — can be added later if needed

### Neutral

- Chunk unloading follows server packets (`ForgetLevelChunk`) — no client-side GC
- Entity metadata is stored as raw typed values; higher-level wrappers
  (e.g., `entity.health()`) are provided as convenience methods
- World state is per-client; multi-client scenarios share only immutable
  registries (ADR-012)

## Compliance

- `PalettedContainer` layout matches vanilla `PalettedContainer.java`
- Chunk section encoding matches protocol spec for `ChunkData` packet
- Entity network IDs match server-assigned IDs from `AddEntity` packet

## Related ADRs

- ADR-009: Authentication Flow (auth precedes world state initialization)
- ADR-011: Event & Handler System (events reference world state)
- ADR-012: Multi-Client Architecture (per-client world state, shared registries)

## References

- [wiki.vg Chunk Format](https://wiki.vg/Chunk_Format)
- [wiki.vg Entity Metadata](https://wiki.vg/Entity_metadata)
- Vanilla source: `PalettedContainer.java`, `LevelChunkSection.java`
- Vanilla source: `ClientPacketListener.java` (state application)
