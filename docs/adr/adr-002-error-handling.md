# ADR-002: Error Handling Strategy

| Field | Value |
|-------|-------|
| Status | Accepted |
| Date | 2026-03-26 |
| Phases | P01-P17 |
| Deciders | HeadlessCraft Core Team |

## Context

HeadlessCraft is a library crate — users add it to their `Cargo.toml` and call our API from their own `main()`. This fundamentally constrains our error handling strategy. Unlike a binary (where errors ultimately become log lines or exit codes), our errors are part of our public API surface. Every `Result<T, E>` we return is a type that downstream code must handle, match on, and reason about. If we get this wrong, users either can't distinguish between failure modes or must resort to string-parsing error messages.

The Minecraft protocol has many distinct failure modes: malformed packets, unexpected packet IDs for the current state, VarInt encoding overflows, NBT parse errors, authentication failures (expired tokens, rate limits, invalid credentials), network timeouts, server kicks, compression errors, and encryption handshake failures. A bot developer needs to distinguish between "server kicked me for spam" (retry with backoff), "authentication token expired" (re-authenticate), and "malformed packet" (likely a bug — log and report). Collapsing these into a single opaque error type defeats the purpose.

Rust's ecosystem offers several error handling patterns. The `anyhow` crate provides ergonomic, type-erased errors ideal for applications — but exposing `anyhow::Error` in a library's public API means callers cannot match on error variants, only inspect the string message. This is a well-known anti-pattern for libraries. We need typed, `#[non_exhaustive]` error enums that callers can pattern-match while remaining forward-compatible as we add new variants.

## Decision Drivers

- **Typed, matchable errors in public API**: callers must be able to `match` on specific error variants (e.g., `ProtocolError::UnknownPacket` vs `ProtocolError::Io`) to make recovery decisions
- **Forward compatibility**: adding a new error variant must not be a semver-breaking change for downstream match arms
- **Rich context on failure**: errors must carry structured data — packet IDs, protocol states, buffer sizes, authentication endpoint URLs — not just string messages
- **Ergonomic `?` operator usage**: error types must compose across crate boundaries with `?` and minimal `.map_err()` boilerplate
- **No panics in production**: the library must never `unwrap()` or `expect()` on user-facing code paths — a panic in a library kills the user's entire process
- **`anyhow`-free public API**: `anyhow::Error` must never appear in a public function signature

## Considered Options

### Option 1: thiserror everywhere

Use `#[derive(thiserror::Error)]` for all error enums across all crates. Each crate defines its own error type with specific variants. Cross-crate error conversion uses `#[from]` attributes. The public API exposes typed enums that callers can match on. `anyhow` is only used in dev-dependencies for test convenience. This approach gives maximum type safety and discoverability — users can read the error enum to understand all possible failure modes.

### Option 2: thiserror in library + anyhow in examples/tests

Same as Option 1 for the library crates, but use `anyhow` in examples, tests, and documentation code. This keeps the public API typed while allowing ergonomic error handling in non-library code. Examples use `anyhow::Result` in their `fn main()` for brevity, demonstrating that users can adopt whatever error strategy they prefer in their own code.

### Option 3: Custom error types without derive macros

Implement `std::fmt::Display` and `std::error::Error` by hand for all error types. This avoids the `thiserror` proc-macro dependency (one fewer compile-time dep). However, it adds 15-20 lines of boilerplate per error type — a `Display` impl, an `Error` impl, and manual `From` impls for each conversion. For a project with 3 crates and dozens of error types, this becomes a maintenance burden that discourages adding proper error context. The boilerplate also obscures the actual error variant structure, making code review harder.

## Decision

**We use `thiserror` for all error types in library code. `anyhow` is a dev-dependency only, used in tests and examples.** Each crate defines its own error enum with `#[derive(Debug, thiserror::Error)]` and `#[non_exhaustive]`.

### Error Type Design

**`headlesscraft-protocol`** — Protocol-level errors:

```rust
/// Errors that occur during protocol encoding, decoding, or validation.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ProtocolError {
    #[error("unknown packet id {id:#04x} in state {state}")]
    UnknownPacket { id: i32, state: &'static str },

    #[error("packet too large: {size} bytes (max {max})")]
    PacketTooLarge { size: usize, max: usize },

    #[error("VarInt exceeded 5 bytes")]
    VarIntTooLong,

    #[error("invalid string length: {len} (max {max})")]
    StringTooLong { len: usize, max: usize },

    #[error("NBT decode error: {0}")]
    Nbt(#[from] NbtError),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}
```

**`headlesscraft`** — Client-level errors:

```rust
/// Errors that occur during client operations.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ClientError {
    #[error("connection failed: {reason}")]
    ConnectionFailed { reason: String },

    #[error("authentication failed: {0}")]
    Auth(#[from] AuthError),

    #[error("server disconnected: {reason}")]
    Disconnected { reason: String },

    #[error("server kicked client: {message}")]
    Kicked { message: String },

    #[error(transparent)]
    Protocol(#[from] headlesscraft_protocol::ProtocolError),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Errors specific to Microsoft/Mojang authentication.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum AuthError {
    #[error("access token expired")]
    TokenExpired,

    #[error("invalid credentials")]
    InvalidCredentials,

    #[error("rate limited by auth server (retry after {retry_after_secs}s)")]
    RateLimited { retry_after_secs: u64 },

    #[error("auth server unreachable: {0}")]
    ServerUnreachable(#[from] reqwest::Error),
}
```

### Error Documentation Convention

Every public function returning `Result` must include an `# Errors` doc section listing the conditions under which each variant is returned:

```rust
/// Connects to a Minecraft server and completes the login sequence.
///
/// # Errors
///
/// Returns [`ClientError::ConnectionFailed`] if the TCP connection cannot be established.
/// Returns [`ClientError::Auth`] if authentication with Mojang/Microsoft fails.
/// Returns [`ClientError::Protocol`] if the server sends an invalid handshake response.
/// Returns [`ClientError::Kicked`] if the server rejects the login (e.g., whitelist, ban).
pub async fn connect(&self, address: &ServerAddress) -> Result<Session, ClientError> {
    // ...
}
```

### Context Wrapping Pattern

For internal functions where additional context is needed without exposing `anyhow` in the API, we use a thin extension trait:

```rust
use std::fmt;

pub(crate) trait ResultExt<T> {
    fn context(self, msg: &'static str) -> Result<T, ClientError>;
}

impl<T, E: Into<ClientError>> ResultExt<T> for Result<T, E> {
    fn context(self, msg: &'static str) -> Result<T, ClientError> {
        self.map_err(|e| {
            let err = e.into();
            tracing::debug!(error = %err, context = msg, "operation failed");
            err
        })
    }
}
```

### Test and Example Code

Tests and examples use `anyhow` for ergonomic error handling:

```rust
// In tests (anyhow is a dev-dependency)
#[tokio::test]
async fn test_connect_to_server() -> anyhow::Result<()> {
    let client = Client::builder().build()?;
    let session = client.connect(&"localhost:25565".parse()?).await?;
    assert!(session.is_connected());
    Ok(())
}
```

## Consequences

### Positive

- Library consumers can pattern-match on specific error variants to make recovery decisions (re-auth, retry, abort, log-and-continue)
- `#[non_exhaustive]` allows adding new error variants in minor versions without breaking downstream match arms
- Error messages carry structured context (packet IDs, sizes, auth endpoints) for diagnostic value
- `# Errors` doc sections make the error contract explicit — users know exactly what can fail and why
- No `anyhow` in public API — callers are not forced into any error handling strategy

### Negative

- Two error crates in the dependency tree (`thiserror` + `anyhow` in dev-deps) — though both are lightweight and ubiquitous
- Error enum variants need maintenance as new failure modes are discovered
- `#[non_exhaustive]` forces downstream callers to include a `_` wildcard arm, which may hide new variants they should handle

### Neutral

- The `ResultExt` context wrapper is internal-only and does not affect the public API surface
- Users who prefer `anyhow` in their own code can convert freely: `client.connect(addr).await.map_err(anyhow::Error::from)?`
- `#[from]` attribute generates `From` impls that allow `?` to convert between error layers automatically

## Compliance

- **Workspace Clippy lints** in root `Cargo.toml`:
  ```toml
  [workspace.lints.clippy]
  unwrap_used = "deny"
  expect_used = "deny"
  ```
- **CI check**: `cargo clippy --workspace -- -D warnings` must pass — any `unwrap()` or `expect()` in non-test code fails the build
- **Code review criteria**: every new public error enum must use `#[derive(Debug, thiserror::Error)]` and `#[non_exhaustive]`
- **Doc audit**: every public function returning `Result` must have an `# Errors` section — CI runs `cargo doc --workspace --no-deps` and reviewers verify error documentation
- **No anyhow in public signatures**: CI grep check ensures `anyhow::Error` and `anyhow::Result` do not appear in any `pub fn` signature outside of examples and tests

## Related ADRs

- [ADR-001: Crate Architecture](adr-001-crate-architecture.md) — defines the crate boundaries where error types are exchanged
- [ADR-004: Logging & Observability](adr-004-logging-observability.md) — errors are logged with tracing events carrying structured context

## References

- [thiserror documentation](https://docs.rs/thiserror/latest/thiserror/)
- [anyhow documentation](https://docs.rs/anyhow/latest/anyhow/)
- [Rust API Guidelines — C-GOOD-ERR](https://rust-lang.github.io/api-guidelines/interoperability.html#error-types-are-meaningful-and-well-behaved-c-good-err)
- [Rust Error Handling — Andrew Gallant (BurntSushi)](https://blog.burntsushi.net/rust-error-handling/)
- [DTolnay — "Semver trick for non_exhaustive"](https://github.com/dtolnay/semver-trick)
