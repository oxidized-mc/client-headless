//! # oxidized-client-headless
//!
//! A Rust framework for building headless Minecraft Java Edition clients.
//!
//! oxidized-client-headless connects to vanilla servers, handles the full protocol lifecycle
//! (handshake → login → configuration → play), and exposes a high-level API for
//! building bots, testing tools, stress testers, and automation.
//!
//! ## Shared Crate Ecosystem
//!
//! Low-level protocol primitives (codec, NBT, types, chat, crypto, compression)
//! are provided by the [`oxidized-mc`](https://github.com/oxidized-mc) crate
//! ecosystem, shared with the Oxidized server:
//!
//! - [`oxidized_codec`] — VarInt/VarLong, wire-format readers/writers
//! - [`oxidized_nbt`] — Named Binary Tag serialization
//! - [`oxidized_mc_types`] — `BlockPos`, `ResourceLocation`, coordinate types
//! - [`oxidized_chat`] — Chat components, formatting, events
//! - [`oxidized_transport`] — Connection state machine, framing, encryption, compression
//! - [`oxidized_auth`] — `GameProfile`, `ProfileProperty`, Mojang session verification
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use oxidized_client_headless::Client;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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
#![warn(missing_docs)]
#![deny(unsafe_code)]

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
    pub async fn build(self) -> Result<Client, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Client)
    }
}

impl Client {
    /// Connect to the configured server.
    ///
    /// # Errors
    ///
    /// Returns an error if the connection fails.
    pub async fn connect(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
}
