# ADR-009: Authentication Flow

| Field | Value |
|-------|-------|
| Status | Accepted |
| Date | 2026-03-26 |
| Phases | P04 |
| Deciders | HeadlessCraft Core Team |

## Context

Connecting to online-mode Minecraft Java Edition servers requires a multi-step
authentication flow through Microsoft's identity platform. The canonical sequence
is: Microsoft OAuth → Xbox Live token → XSTS token → Minecraft access token →
session server join. Each step is a separate HTTPS request with its own error
modes (expired tokens, rate limits, account-not-owning-Minecraft). Offline-mode
servers skip all of this and only need a username string.

As a *library*, HeadlessCraft cannot assume a single auth strategy. Some users
will run CLI bots where a device-code flow (no browser required) is ideal.
Others already manage token caches in a parent application and just want to hand
us a valid Minecraft access token. Still others may need proxy or custom OAuth
endpoints for corporate environments.

A rigid, built-in-only auth system forces every consumer into one workflow.
Conversely, a token-only API pushes too much complexity onto callers who just
want "connect this bot to a server." We need a middle ground that ships sensible
defaults while remaining fully extensible.

## Decision Drivers

- Must support both online-mode and offline-mode servers
- Must work in headless/CLI environments (no browser pop-up)
- Must allow users to supply pre-obtained tokens
- Must support token caching and automatic refresh
- Must be extensible for custom OAuth flows or token providers
- Should not force heavy dependencies on users who only need offline mode
- Should handle the full Microsoft → Xbox → XSTS → Minecraft chain correctly

## Considered Options

### Option 1: Full Built-in Auth with Device Code Flow

Ship a single, opinionated `authenticate()` function that runs the complete
Microsoft device-code flow internally. Offline mode handled as a special case.

**Pros:** Simple API — one function call.
**Cons:** No flexibility. Users with existing token caches must re-authenticate.
Hard to test. Forces `reqwest` dependency even for offline use.

### Option 2: Token-Only (Users Handle OAuth Externally)

Accept only a raw Minecraft access token (or username for offline). All OAuth
logic is the caller's responsibility.

**Pros:** Minimal library surface. No HTTP dependency for auth.
**Cons:** Pushes significant complexity onto every user. The Microsoft auth chain
is 4+ HTTP calls with specific payload formats — easy to get wrong. Poor DX for
the common "just connect my bot" case.

### Option 3: Pluggable Auth with Built-in Microsoft Flow as Default

Define an `Authenticator` trait. Ship `OfflineAuth` and `MicrosoftAuth` as
built-in implementations. Users can implement the trait for custom flows.

**Pros:** Best of both worlds — simple default, full extensibility. Each auth
strategy is independently testable. Feature-gated dependencies.
**Cons:** Slightly more API surface than Option 2.

## Decision

**Option 3: Pluggable auth via the `Authenticator` trait.**

### Core Trait

```rust
/// Result of a successful authentication flow.
pub struct AuthSession {
    /// Minecraft profile UUID.
    pub uuid: Uuid,
    /// Player name (used in-game).
    pub username: String,
    /// Access token for session-server join requests.
    /// `None` for offline-mode connections.
    pub access_token: Option<String>,
}

/// Abstraction over authentication strategies.
#[async_trait]
pub trait Authenticator: Send + Sync + 'static {
    /// Perform authentication and return a session.
    async fn authenticate(&self) -> Result<AuthSession, AuthError>;

    /// Refresh an existing session. Returns `Err` if refresh is unsupported.
    async fn refresh(&self, session: &AuthSession) -> Result<AuthSession, AuthError> {
        Err(AuthError::RefreshUnsupported)
    }
}
```

### Built-in Implementations

```rust
/// Offline-mode authentication. No network calls.
pub struct OfflineAuth {
    username: String,
}

impl OfflineAuth {
    pub fn new(username: impl Into<String>) -> Self {
        Self { username: username.into() }
    }
}

#[async_trait]
impl Authenticator for OfflineAuth {
    async fn authenticate(&self) -> Result<AuthSession, AuthError> {
        Ok(AuthSession {
            uuid: offline_uuid(&self.username),
            username: self.username.clone(),
            access_token: None,
        })
    }
}

/// Microsoft OAuth device-code flow — ideal for headless/CLI apps.
///
/// Flow sequence:
/// 1. Request device code from Microsoft identity platform
/// 2. User enters code at https://microsoft.com/devicelogin
/// 3. Poll for Microsoft access token
/// 4. Exchange for Xbox Live token (user authenticate)
/// 5. Exchange Xbox Live token for XSTS token (authorize)
/// 6. Exchange XSTS token for Minecraft access token (login with Xbox)
/// 7. Verify Minecraft ownership (optional, recommended)
///
/// Tokens are cached in-memory and auto-refreshed on expiry.
pub struct MicrosoftAuth {
    client_id: String,
    http: reqwest::Client,
    token_cache: RwLock<Option<CachedTokens>>,
}
```

### Auth Flow Sequence

```text
┌──────────┐     ┌───────────┐     ┌──────────┐     ┌──────────┐     ┌───────────┐
│ Microsoft│     │ Xbox Live │     │   XSTS   │     │ Minecraft│     │  Session   │
│  OAuth   │────▸│   Auth    │────▸│ Authorize│────▸│  Login   │────▸│  Server   │
│(device   │     │           │     │          │     │(with     │     │  (join)   │
│  code)   │     │           │     │          │     │  Xbox)   │     │           │
└──────────┘     └───────────┘     └──────────┘     └──────────┘     └───────────┘
     │                                                                      │
     └── User enters code ──────────────────────────────── Server validates ─┘
```

### Builder API

```rust
use headlesscraft::{Client, OfflineAuth, MicrosoftAuth};

// Offline mode — no network auth required
let client = Client::builder()
    .server_address("localhost:25565")
    .authenticator(OfflineAuth::new("TestBot"))
    .build()
    .await?;

// Online mode — device code flow
let auth = MicrosoftAuth::new("your-azure-client-id");
let client = Client::builder()
    .server_address("mc.example.com:25565")
    .authenticator(auth)
    .build()
    .await?;

// Custom auth — user supplies pre-obtained token
struct PreAuthToken { token: String, uuid: Uuid, name: String }

#[async_trait]
impl Authenticator for PreAuthToken {
    async fn authenticate(&self) -> Result<AuthSession, AuthError> {
        Ok(AuthSession {
            uuid: self.uuid,
            username: self.name.clone(),
            access_token: Some(self.token.clone()),
        })
    }
}
```

### Token Refresh

`MicrosoftAuth` caches all intermediate tokens and tracks expiry times. When
`refresh()` is called (automatically by the client before each reconnect), it
reuses the Microsoft refresh token to obtain a new access token chain without
user interaction. If the refresh token itself has expired, the full device-code
flow is re-triggered.

## Consequences

### Positive

- Clean separation between auth and connection logic
- Offline and online modes use the same `Client::builder()` API
- Users with existing token management can plug in directly
- Each `Authenticator` implementation is independently unit-testable
- Microsoft flow details are encapsulated — can update without breaking API

### Negative

- `MicrosoftAuth` pulls in `reqwest` and `serde_json` — mitigated by a
  `microsoft-auth` feature flag (enabled by default)
- Device-code flow requires user interaction (entering a code) — unavoidable
  for headless apps without pre-cached tokens
- Token refresh logic adds complexity to `MicrosoftAuth` internals

### Neutral

- Offline UUID generation uses the vanilla algorithm (`md5("OfflinePlayer:" + name)`)
- The `Authenticator` trait is async — even `OfflineAuth` returns a future (zero-cost)
- Session server join (`POST /session/minecraft/join`) is handled by the
  connection layer, not the authenticator — the authenticator only provides tokens

## Compliance

- Microsoft identity platform device-code flow: RFC 8628
- Minecraft session server protocol: wiki.vg Session documentation
- Offline UUID algorithm matches vanilla `UUID.nameUUIDFromBytes()`

## Related ADRs

- ADR-012: Multi-Client Architecture (shared HTTP client for auth)
- ADR-011: Event & Handler System (disconnect events trigger re-auth)

## References

- [Microsoft Device Code Flow](https://learn.microsoft.com/en-us/entra/identity-platform/v2-oauth2-device-code)
- [wiki.vg Authentication](https://wiki.vg/Microsoft_Authentication_Scheme)
- [wiki.vg Protocol Encryption](https://wiki.vg/Protocol_Encryption)
- [Xbox Live Authentication](https://learn.microsoft.com/en-us/gaming/gdk/_content/gc/reference/live/rest/uri/uri-abortuserauthenticate)
