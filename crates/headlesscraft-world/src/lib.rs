//! Client-side world representation for HeadlessCraft.
//!
//! Maintains the world state as received from the server: chunks, block states,
//! entities, biomes, and light data. Provides query APIs for bot navigation and
//! pathfinding.
#![warn(missing_docs)]
#![deny(unsafe_code)]
