//! Minecraft Java Edition protocol codec, packet definitions, and NBT for HeadlessCraft.
//!
//! This crate handles the wire protocol from the client perspective:
//!
//! - **Packets** — encoding serverbound packets, decoding clientbound packets
//! - **Codecs** — VarInt/VarLong, wire-format readers/writers via [`codec`]
//! - **NBT** — Named Binary Tag serialization via [`nbt`]
//! - **Types** — Shared coordinate types, block IDs, protocol primitives via [`types`]
//! - **Chat** — Chat component tree, formatting, events via [`chat`]
//! - **Connection states** — Handshaking, Status, Login, Configuration, Play
//!
//! Low-level protocol primitives are provided by the shared `oxidized-mc` crate
//! ecosystem and re-exported here for convenience. Client-specific packet definitions
//! and derive macros will be added in future phases.
#![warn(missing_docs)]
#![deny(unsafe_code)]

/// Wire-format codec — VarInt, VarLong, `Packet` trait, type readers/writers.
pub use oxidized_codec as codec;

/// Named Binary Tag (NBT) — 13 tag types, binary reader/writer, network NBT,
/// SNBT parser/formatter, serde integration.
pub use oxidized_nbt as nbt;

/// Minecraft-specific types — `BlockPos`, `ResourceLocation`, `Vec3`, `Direction`, etc.
pub use oxidized_mc_types as types;

/// Chat component tree — text, translatable, styles, click/hover events,
/// JSON and NBT serialization.
pub use oxidized_chat as chat;
