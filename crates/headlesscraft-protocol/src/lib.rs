//! Minecraft Java Edition protocol codec, packet definitions, and NBT for HeadlessCraft.
//!
//! This crate handles the wire protocol from the client perspective:
//!
//! - **Packets** — encoding serverbound packets, decoding clientbound packets
//! - **Codecs** — VarInt/VarLong, framing, encryption (AES-128-CFB8), compression (zlib)
//! - **NBT** — Named Binary Tag serialization (13 tag types, SNBT, gzip/zlib I/O)
//! - **Types** — Shared coordinate types, block IDs, and protocol primitives
//! - **Connection states** — Handshaking, Status, Login, Configuration, Play
//!
//! This crate can be used standalone for building protocol analyzers, proxies,
//! or other tools that don't need the full client.
#![warn(missing_docs)]
#![deny(unsafe_code)]
