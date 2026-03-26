# ADR-007: Encryption & Compression Pipeline

| Field | Value |
|-------|-------|
| Status | Accepted |
| Date | 2026-03-26 |
| Phases | P04 |
| Deciders | HeadlessCraft Core Team |

## Context

The Minecraft Java Edition protocol applies two optional transforms to the TCP byte stream:
**encryption** (AES-128-CFB8) and **compression** (zlib). Both are activated dynamically
during the login sequence and remain active for the rest of the connection's lifetime. As a
headless client, HeadlessCraft must implement both transforms on the **client side** —
encrypting outbound packets and decrypting inbound packets, compressing outbound payloads and
decompressing inbound payloads.

The login flow activates these layers in a specific order. First, the server sends an
`EncryptionRequest` containing its public RSA key and a verify token. The client generates a
16-byte shared secret, encrypts it (along with the verify token) using the server's public
key, and sends an `EncryptionResponse`. From that point forward, **all traffic in both
directions** is encrypted using AES-128-CFB8 with the shared secret as both the key and the
initialization vector. Later (sometimes before encryption, sometimes after — depends on the
server), the server sends a `SetCompression` packet specifying a compression threshold. After
that, all packets include an uncompressed length field, and payloads at or above the threshold
are zlib-compressed.

Critically, the order of operations on the wire is: `encrypt(compress(frame(packet)))` for
outbound and the reverse for inbound. Getting this layering wrong — or activating a layer at
the wrong time — results in immediate connection failure with no useful error message from the
server.

## Decision Drivers

- **Correctness:** The exact byte-level behavior must match the vanilla client, including CFB8 mode quirks and zlib flush behavior.
- **Activation timing:** Layers must be activated at precise points in the login packet sequence — not one packet too early or late.
- **Performance:** Bots running many connections need low per-connection overhead for encryption and compression.
- **Testability:** Each layer must be independently testable without needing a live server.
- **Security:** The RSA exchange and AES encryption must use well-audited cryptographic primitives, not custom implementations.

## Considered Options

### Option 1: Layered Tokio Codec Stack

Implement each transform as an independent `tokio_util::codec::Decoder`/`Encoder`. Stack them
using `Framed` wrappers: `FrameCodec` → `CompressionCodec` → `EncryptionCodec`. Each codec
can be swapped from a no-op passthrough to an active implementation mid-stream.

**Pros:** Clean separation of concerns, each layer independently testable, composes naturally
with Tokio's async I/O, can activate/deactivate layers dynamically.
**Cons:** Multiple layers of indirection, slight overhead from buffering between layers.

### Option 2: Single Monolithic Codec

One codec that handles framing, compression, and encryption in a single `decode`/`encode`
method with internal flags for which transforms are active.

**Pros:** No inter-layer buffering, potentially slightly faster.
**Cons:** Complex monolithic code, hard to test individual transforms, mixing concerns makes
bugs harder to isolate, difficult to reason about activation ordering.

### Option 3: Transform Wrappers Around Raw TCP Stream

Wrap `TcpStream` in custom `AsyncRead`/`AsyncWrite` implementations that apply
encryption/compression transparently.

**Pros:** Works with any reader/writer, not tied to tokio-util codecs.
**Cons:** Stream-level transforms don't naturally align with packet boundaries, compression
operates on whole packets (not byte streams), awkward impedance mismatch.

## Decision

**Option 1 — Layered tokio-util codec stack** with dynamic activation.

### Pipeline Architecture

```
Outbound (client → server):
  Packet struct
    → Encode (serialize to bytes via ADR-005)
    → FrameCodec (prepend VarInt length)
    → CompressionCodec (zlib compress if above threshold)
    → EncryptionCodec (AES-128-CFB8 encrypt)
    → TCP stream

Inbound (server → client):
  TCP stream
    → EncryptionCodec (AES-128-CFB8 decrypt)
    → CompressionCodec (zlib decompress if compressed)
    → FrameCodec (read VarInt length, extract frame)
    → Decode (deserialize to Packet struct)
```

### Layer Definitions

Each layer implements `tokio_util::codec::{Encoder, Decoder}` and wraps an inner state
that is either **inactive** (passthrough) or **active** (transforming):

```rust
pub struct EncryptionCodec {
    /// `None` before encryption is enabled; `Some` after EncryptionResponse is sent.
    state: Option<CipherState>,
}

struct CipherState {
    encryptor: Cfb8Encryptor<Aes128>,
    decryptor: Cfb8Decryptor<Aes128>,
}

impl EncryptionCodec {
    /// Activate encryption using the shared secret.
    ///
    /// The shared secret serves as both the AES-128 key and the IV.
    pub fn enable(&mut self, shared_secret: &[u8; 16]) {
        let key = GenericArray::from_slice(shared_secret);
        let iv = GenericArray::from_slice(shared_secret);
        self.state = Some(CipherState {
            encryptor: Cfb8Encryptor::<Aes128>::new(key, iv),
            decryptor: Cfb8Decryptor::<Aes128>::new(key, iv),
        });
    }
}
```

```rust
pub struct CompressionCodec {
    /// Compression threshold in bytes. `None` means compression is disabled.
    threshold: Option<u32>,
}

impl CompressionCodec {
    /// Activate compression with the given threshold.
    ///
    /// A threshold of 0 means compress everything.
    /// The server sends -1 to disable compression (we treat as `None`).
    pub fn enable(&mut self, threshold: i32) {
        self.threshold = if threshold >= 0 {
            Some(threshold as u32)
        } else {
            None
        };
    }
}
```

### AES-128-CFB8 Details

Minecraft uses CFB8 mode, **not** the more common CFB128. CFB8 feeds back only 1 byte at a
time, which means:

- Each byte of plaintext requires a full AES block encryption for the feedback.
- It is roughly 16× slower than CFB128 for the same data volume.
- The `cfb8` crate (from RustCrypto) provides a correct, audited implementation.
- The shared secret (16 bytes) is used as **both** the AES key and the initialization vector.

```rust
// Crate dependencies:
// aes = "0.8"
// cfb8 = "0.8"
use aes::Aes128;
use cfb8::cipher::{AsyncStreamCipher, NewCipher};
use cfb8::{Cfb8, Decryptor as Cfb8Decryptor, Encryptor as Cfb8Encryptor};
```

### Compression Format

When compression is active, the packet frame changes:

```
Without compression:
  [Packet Length (VarInt)] [Packet ID (VarInt)] [Payload...]

With compression (below threshold — not compressed):
  [Packet Length (VarInt)] [Data Length = 0 (VarInt)] [Packet ID (VarInt)] [Payload...]

With compression (at or above threshold — compressed):
  [Packet Length (VarInt)] [Data Length (VarInt)] [zlib-compressed: Packet ID + Payload]
```

`Data Length` is the uncompressed size of `Packet ID + Payload`. A value of `0` means the
payload is **not** compressed (it was below the threshold). The outer `Packet Length` covers
everything after itself (Data Length field + possibly compressed data).

```rust
impl CompressionCodec {
    fn compress_if_needed(&self, uncompressed: &[u8], buf: &mut BytesMut) -> Result<(), EncodeError> {
        let threshold = match self.threshold {
            Some(t) => t as usize,
            None => {
                buf.extend_from_slice(uncompressed);
                return Ok(());
            }
        };

        if uncompressed.len() < threshold {
            // Below threshold: write data_length=0, then raw data.
            VarInt(0).encode(buf)?;
            buf.extend_from_slice(uncompressed);
        } else {
            // At or above threshold: write uncompressed length, then zlib data.
            VarInt(uncompressed.len() as i32).encode(buf)?;
            let mut encoder = ZlibEncoder::new(buf.writer(), Compression::default());
            encoder.write_all(uncompressed)?;
            encoder.finish()?;
        }
        Ok(())
    }
}
```

### Activation Sequence During Login

```
Client                              Server
  │                                   │
  │──── Handshake (next=Login) ──────►│
  │──── LoginStart ──────────────────►│
  │                                   │
  │◄─── EncryptionRequest ───────────│  (RSA public key + verify token)
  │                                   │
  │  [client generates shared_secret] │
  │  [encrypts with server RSA key]   │
  │                                   │
  │──── EncryptionResponse ──────────►│
  │                                   │
  │  ┌─── ENCRYPTION ENABLED ────────┐│  (both directions, immediately)
  │  │    All subsequent bytes are    ││
  │  │    AES-128-CFB8 encrypted      ││
  │  └────────────────────────────────┘│
  │                                   │
  │◄─── SetCompression ─────────────│  (threshold value)
  │                                   │
  │  ┌─── COMPRESSION ENABLED ──────┐│  (applied inside encryption)
  │  └────────────────────────────────┘│
  │                                   │
  │◄─── LoginSuccess ───────────────│
  │──── LoginAcknowledged ──────────►│
  │                                   │
  │        [→ Configuration state]    │
```

**Critical timing:** Encryption activates **immediately after sending** `EncryptionResponse`,
meaning the `EncryptionResponse` packet itself is sent unencrypted, but the very next byte on
the wire (in both directions) is encrypted. Compression activates after the `SetCompression`
packet is fully read.

### RSA Key Exchange (Client Side)

```rust
use rsa::{RsaPublicKey, Pkcs1v15Encrypt};

/// Perform the client-side encryption handshake.
pub async fn handle_encryption_request(
    conn: &mut Connection<Login>,
    request: &EncryptionRequest,
) -> Result<[u8; 16], LoginError> {
    let shared_secret: [u8; 16] = rand::random();

    let server_key = RsaPublicKey::from_public_key_der(&request.public_key)?;
    let encrypted_secret = server_key.encrypt(&mut rand::thread_rng(), Pkcs1v15Encrypt, &shared_secret)?;
    let encrypted_token = server_key.encrypt(&mut rand::thread_rng(), Pkcs1v15Encrypt, &request.verify_token)?;

    conn.send(EncryptionResponse {
        shared_secret: encrypted_secret,
        verify_token: encrypted_token,
    }).await?;

    // Enable encryption on both read and write sides IMMEDIATELY.
    conn.enable_encryption(&shared_secret);

    Ok(shared_secret)
}
```

## Consequences

### Positive

- Each codec layer is independently unit-testable with known byte sequences.
- Dynamic activation means the same `Connection` struct works across all login phases without rebuilding the I/O stack.
- Using audited RustCrypto crates (`aes`, `cfb8`, `rsa`) avoids custom cryptography.
- The layered design matches the protocol's conceptual model, making it easy to reason about.
- `flate2` zlib is well-tested and supports streaming compression/decompression.

### Negative

- CFB8 mode is inherently slow (~16× CFB128). For bots maintaining many connections, this may become a bottleneck under heavy traffic.
- The codec stack has three layers of buffering, which adds minor memory overhead per connection.
- Dynamic layer activation requires interior mutability or careful ownership of the codec pipeline.

### Neutral

- If performance profiling reveals CFB8 as a bottleneck, we can explore platform-specific AES-NI optimizations via the `aes` crate's `aes-ni` feature (already default on x86_64).
- The compression threshold is server-configurable; most vanilla servers use 256 bytes. We defer tuning to the server's choice.

## Compliance

- Encryption tests: encrypt known plaintext with known key/IV, compare against vanilla-captured ciphertext.
- Compression tests: compress/decompress packets at various sizes around the threshold boundary.
- Full login integration test against a vanilla server verifying encryption + compression activation.
- Property tests: `decrypt(encrypt(data)) == data` and `decompress(compress(data)) == data`.

## Related ADRs

- ADR-005: Packet Codec Framework — the `Encode`/`Decode` layer that sits inside the pipeline.
- ADR-006: Connection Lifecycle — drives the login flow that activates encryption and compression.
- ADR-008: NBT Library Design — NBT payloads are compressed/encrypted like any other packet data.

## References

- [wiki.vg Encryption](https://wiki.vg/Protocol_Encryption)
- [wiki.vg Protocol — Packet Format](https://wiki.vg/Protocol#Packet_format)
- [AES-CFB8 Mode](https://en.wikipedia.org/wiki/Block_cipher_mode_of_operation#Cipher_feedback_(CFB))
- [RustCrypto `aes` crate](https://docs.rs/aes)
- [RustCrypto `cfb8` crate](https://docs.rs/cfb8)
- [`flate2` zlib compression](https://docs.rs/flate2)
- [RSA PKCS#1 v1.5 Encryption](https://docs.rs/rsa)
