//! Startup handshake primitives (D1 · backend/01 §1.1.1).
//!
//! The Flutter desktop main process forks this Rust core as a child. The child:
//! 1. binds `tokio::net::TcpListener` to `127.0.0.1:0` (OS-assigned random port; INV-05),
//! 2. generates a 64-byte random token (hex-encoded → 128 hex chars),
//! 3. prints **exactly one line** `READY{"port":N,"token":"<hex>"}` to stdout.
//!
//! This module owns steps 1-2 and the READY line formatting; `crates/api/src/main.rs` calls
//! [`bind_loopback`] + [`Handshake::new`] and emits [`Handshake::ready_line`] before serving.

use rand::RngCore;
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;

/// Literal prefix that precedes the handshake JSON on the READY stdout line (no separator).
pub const READY_PREFIX: &str = "READY";

/// Number of random bytes in the Bearer token before hex encoding (backend/01 §1.1.1).
pub const TOKEN_BYTES: usize = 64;

/// Bind a [`TcpListener`] to `127.0.0.1:0` and let the OS assign a free port.
///
/// Errors only on the rare case that loopback cannot be bound at all.
pub async fn bind_loopback() -> std::io::Result<TcpListener> {
    TcpListener::bind("127.0.0.1:0").await
}

/// Generate a fresh 64-byte random token, hex-encoded to 128 lowercase chars.
pub fn generate_token() -> String {
    let mut buf = [0u8; TOKEN_BYTES];
    rand::thread_rng().fill_bytes(&mut buf);
    hex::encode(buf)
}

/// The handshake payload printed (prefixed by [`READY_PREFIX`]) to stdout for the parent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Handshake {
    /// OS-assigned loopback port.
    pub port: u16,
    /// 128-char hex Bearer token.
    pub token: String,
}

impl Handshake {
    /// Build a handshake for a bound listener, generating a fresh token.
    pub fn new(listener: &TcpListener) -> std::io::Result<Self> {
        let port = listener.local_addr()?.port();
        Ok(Self {
            port,
            token: generate_token(),
        })
    }

    /// Render the exact single READY line: `READY` literal immediately followed by the
    /// compact JSON object (no whitespace between prefix and JSON, no trailing newline).
    pub fn ready_line(&self) -> String {
        // serde_json::to_string is compact (no spaces) and field order is declaration order.
        let json = serde_json::to_string(self).expect("Handshake serializes");
        format!("{READY_PREFIX}{json}")
    }

    /// Parse a READY line back into a handshake (used by tests and the Dart-side contract).
    pub fn parse_ready_line(line: &str) -> Option<Self> {
        let json = line.strip_prefix(READY_PREFIX)?;
        serde_json::from_str(json).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_is_128_hex_chars() {
        let t = generate_token();
        assert_eq!(t.len(), TOKEN_BYTES * 2, "64 bytes => 128 hex chars");
        assert!(t.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn tokens_are_unique() {
        assert_ne!(generate_token(), generate_token());
    }

    #[test]
    fn ready_line_has_prefix_then_compact_json() {
        let hs = Handshake {
            port: 54321,
            token: "ab".repeat(64),
        };
        let line = hs.ready_line();
        assert!(line.starts_with("READY{"), "got {line}");
        assert!(line.contains("\"port\":54321"), "got {line}");
        assert!(line.contains("\"token\":\""), "got {line}");
        assert!(!line.contains('\n'), "READY must be a single line");
        // round-trip
        let back = Handshake::parse_ready_line(&line).unwrap();
        assert_eq!(back.port, 54321);
        assert_eq!(back.token, hs.token);
    }

    #[tokio::test]
    async fn bind_loopback_gives_nonzero_port() {
        let listener = bind_loopback().await.unwrap();
        let addr = listener.local_addr().unwrap();
        assert_eq!(addr.ip().to_string(), "127.0.0.1");
        assert_ne!(addr.port(), 0, "OS assigns a concrete port");
        let hs = Handshake::new(&listener).unwrap();
        assert_eq!(hs.port, addr.port());
        assert_eq!(hs.token.len(), 128);
    }
}
