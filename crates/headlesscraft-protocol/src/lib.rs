//! Minecraft Java Edition protocol codec, packet definitions, and NBT for HeadlessCraft.
//!
//! This crate handles the wire protocol from the client perspective:
//!
//! - **Packets** — encoding serverbound packets, decoding clientbound packets
//! - **Codecs** — VarInt/VarLong, wire-format readers/writers via [`oxidized_codec`]
//! - **NBT** — Named Binary Tag serialization via [`oxidized_nbt`]
//! - **Types** — Shared coordinate types, block IDs, protocol primitives via [`oxidized_mc_types`]
//! - **Chat** — Chat component tree, formatting, events via [`oxidized_chat`]
//! - **Connection states** — Handshaking, Status, Login, Configuration, Play
//!
//! Low-level protocol primitives are provided by the shared `oxidized-mc` crate
//! ecosystem. Client-specific packet definitions and derive macros will be added
//! in future phases.
#![warn(missing_docs)]
#![deny(unsafe_code)]
