//! # HeadlessCraft
//!
//! A Rust framework for building headless Minecraft Java Edition clients.
//!
//! HeadlessCraft connects to vanilla servers, handles the full protocol lifecycle
//! (handshake → login → configuration → play), and exposes a high-level API for
//! building bots, testing tools, stress testers, protocol analyzers, and proxy
//! frameworks.
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use headlesscraft::Client;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let client = Client::builder()
//!         .address("localhost:25565")
//!         .username("Bot")
//!         .build()
//!         .await?;
//!
//!     client.connect().await?;
//!     Ok(())
//! }
//! ```
//!
//! ## Crate Re-exports
//!
//! This crate re-exports the most commonly needed types from the workspace:
//!
//! - [`headlesscraft_types`] — Coordinate types and shared primitives
//! - [`headlesscraft_nbt`] — NBT serialization
//! - [`headlesscraft_protocol`] — Packet definitions and codecs
//! - [`headlesscraft_world`] — Client-side world state
//! - [`headlesscraft_client`] — Connection, session, and bot API
#![warn(missing_docs)]
#![deny(unsafe_code)]

pub use headlesscraft_client as client;
pub use headlesscraft_nbt as nbt;
pub use headlesscraft_protocol as protocol;
pub use headlesscraft_types as types;
pub use headlesscraft_world as world;

/// Placeholder for the high-level `Client` builder.
///
/// This will be the primary entry point for end users.
pub struct Client;

impl Client {
    /// Create a new client builder.
    pub fn builder() -> ClientBuilder {
        ClientBuilder
    }
}

/// Builder for configuring and creating a [`Client`].
pub struct ClientBuilder;

impl ClientBuilder {
    /// Set the server address to connect to.
    pub fn address(self, _addr: &str) -> Self {
        self
    }

    /// Set the username for the client.
    pub fn username(self, _name: &str) -> Self {
        self
    }

    /// Build the client. Does not connect yet.
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration is invalid.
    pub async fn build(self) -> Result<Client, Box<dyn std::error::Error>> {
        Ok(Client)
    }
}

impl Client {
    /// Connect to the configured server.
    ///
    /// # Errors
    ///
    /// Returns an error if the connection fails.
    pub async fn connect(&self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}
