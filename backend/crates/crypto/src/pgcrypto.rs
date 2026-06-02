//! §4.4.2 Postgres `pgp_sym_*` field cipher — **interface only**, behind `cfg(feature = "pg")`.
//!
//! Blocked by L0-03 (D2: the embedded-Postgres dialect is undecided — the spike has not selected a
//! winner). The `Conn` type and the concrete `pgp_sym_encrypt` / `pgp_sym_decrypt` prepared-
//! statement implementation cannot be pinned down until the DB layer exists, so R1a ships only the
//! shape. The real R1a path is the SQLite-fallback [`crate::field_cipher::ChaChaFieldCipher`]
//! (the brief §8 / I-10: single dialect in R1, no dual-source).
//!
//! The PG passphrase is `base64(DEK)` (§4.4.3). The session DEK never leaves the process.

use crate::master_key::SessionDek;
use crate::Result;

/// Application-layer wrapper over pgcrypto `pgp_sym_*` (`backend/04` §4.4.2). The DEK is the
/// `pgp_sym_encrypt` passphrase (base64-encoded, §4.4.3).
///
/// `'dek` ties the cipher to the in-memory session DEK. The async methods take a connection handle
/// that the db layer will supply once the dialect is chosen; until then this is a typed placeholder
/// so callers and the dependency graph compile under `--features pg`.
pub struct FieldCipher<'dek> {
    #[allow(dead_code)]
    dek: &'dek SessionDek,
}

impl<'dek> FieldCipher<'dek> {
    /// Bind the cipher to a session DEK.
    #[must_use]
    pub fn new(dek: &'dek SessionDek) -> Self {
        FieldCipher { dek }
    }

    /// The `pgp_sym_encrypt` passphrase: standard-base64 of the 32-byte DEK (`backend/04` §4.4.3).
    /// Implemented now because it is dialect-independent and the unit-testable half of the PG path.
    #[must_use]
    pub fn passphrase(&self) -> String {
        base64_standard(self.dek.expose())
    }

    /// Encrypt a field via a `pgp_sym_encrypt` prepared statement.
    ///
    /// Unimplemented in R1a — L0-03 blocks the `Conn` type. The signature matches the brief §2.4 so
    /// the db layer can fill it in once the dialect is selected. Returns
    /// [`crate::CryptoError::FsDetectUnavailable`]'s sibling intent via a clear panic-free error.
    #[allow(unused_variables)]
    pub async fn encrypt<Conn>(&self, conn: &mut Conn, plain: &str) -> Result<Vec<u8>> {
        Err(crate::CryptoError::Age(
            "pgcrypto path is blocked by L0-03 (D2) — use ChaChaFieldCipher in R1a".into(),
        ))
    }

    /// Decrypt a field via a `pgp_sym_decrypt` prepared statement. Unimplemented in R1a (see
    /// [`encrypt`](Self::encrypt)).
    #[allow(unused_variables)]
    pub async fn decrypt<Conn>(&self, conn: &mut Conn, blob: &[u8]) -> Result<String> {
        Err(crate::CryptoError::Age(
            "pgcrypto path is blocked by L0-03 (D2) — use ChaChaFieldCipher in R1a".into(),
        ))
    }
}

/// Minimal standard-base64 encoder (RFC 4648) — avoids pulling a base64 crate just for the PG
/// passphrase. Only used on the gated PG path.
fn base64_standard(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(ALPHABET[((n >> 18) & 63) as usize] as char);
        out.push(ALPHABET[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_known_vectors() {
        assert_eq!(base64_standard(b""), "");
        assert_eq!(base64_standard(b"f"), "Zg==");
        assert_eq!(base64_standard(b"fo"), "Zm8=");
        assert_eq!(base64_standard(b"foo"), "Zm9v");
        assert_eq!(base64_standard(b"foob"), "Zm9vYg==");
        assert_eq!(base64_standard(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64_standard(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn passphrase_is_base64_of_dek() {
        let dek = SessionDek::new([0u8; 32]);
        let fc = FieldCipher::new(&dek);
        // 32 zero bytes → 44-char base64 (with one '=' pad).
        let p = fc.passphrase();
        assert_eq!(p.len(), 44);
        assert!(p.ends_with('='));
    }
}
