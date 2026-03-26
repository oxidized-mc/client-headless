//! Minecraft protocol codec and packet definitions for HeadlessCraft.
//!
//! This crate handles the client-side wire protocol: encoding outbound packets,
//! decoding inbound packets, VarInt/VarLong codecs, encryption, and compression.
#![warn(missing_docs)]
#![deny(unsafe_code)]
