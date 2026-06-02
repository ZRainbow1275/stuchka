//! §4.5 — ChaCha20-Poly1305 AEAD over the `who` / `what` segments.
//!
//! W5 (security ruling): a fresh **random** 12-byte nonce is drawn per segment per record. The
//! backend/04 §4.8 `derive_nonce_from_seq` proposal is overruled — it would reuse one nonce for
//! both `who` and `what` under the same key, which is a catastrophic ChaCha20-Poly1305 misuse.
//!
//! The audit key is `HKDF-SHA256(DEK, info="stuchka-audit-key-v1")` derived in `crates/crypto`
//! ([`crypto::hkdf_subkey`] / [`crypto::AUDIT_INFO`], C-3); the DEK never leaves the backend.

use chacha20poly1305::aead::{Aead, AeadCore, KeyInit, OsRng};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};

use crate::error::CipherError;

/// AEAD cipher bound to a 32-byte audit key.
pub struct AuditCipher {
    cipher: ChaCha20Poly1305,
}

impl AuditCipher {
    /// Construct from a raw 32-byte audit key (already HKDF-derived from the DEK).
    #[must_use]
    pub fn new(key: &[u8; 32]) -> Self {
        let key = Key::from_slice(key);
        Self {
            cipher: ChaCha20Poly1305::new(key),
        }
    }

    /// Encrypt one segment, returning `(ciphertext_with_tag, nonce)` with a fresh random nonce.
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<(Vec<u8>, Vec<u8>), CipherError> {
        let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng);
        let ct = self
            .cipher
            .encrypt(&nonce, plaintext)
            .map_err(|_| CipherError::Aead)?;
        Ok((ct, nonce.to_vec()))
    }

    /// Decrypt one segment given its ciphertext and the 12-byte nonce it was sealed with.
    pub fn decrypt(&self, ciphertext: &[u8], nonce: &[u8]) -> Result<Vec<u8>, CipherError> {
        if nonce.len() != 12 {
            return Err(CipherError::BadNonceLen(nonce.len()));
        }
        self.cipher
            .decrypt(Nonce::from_slice(nonce), ciphertext)
            .map_err(|_| CipherError::Aead)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> [u8; 32] {
        [0x42u8; 32]
    }

    #[test]
    fn round_trips() {
        let c = AuditCipher::new(&key());
        let pt = b"{\"case_id\":\"x\"}";
        let (ct, nonce) = c.encrypt(pt).unwrap();
        assert_eq!(nonce.len(), 12);
        assert_ne!(ct.as_slice(), pt.as_slice());
        let back = c.decrypt(&ct, &nonce).unwrap();
        assert_eq!(back, pt);
    }

    #[test]
    fn each_segment_gets_a_distinct_random_nonce() {
        // W5: two encryptions of the same plaintext must use different nonces and produce
        // different ciphertexts.
        let c = AuditCipher::new(&key());
        let (ct1, n1) = c.encrypt(b"same").unwrap();
        let (ct2, n2) = c.encrypt(b"same").unwrap();
        assert_ne!(n1, n2, "nonces must be random per segment (W5)");
        assert_ne!(ct1, ct2, "same plaintext under distinct nonces differs");
    }

    #[test]
    fn wrong_nonce_fails() {
        let c = AuditCipher::new(&key());
        let (ct, _nonce) = c.encrypt(b"secret").unwrap();
        let bad = [0u8; 12];
        assert!(c.decrypt(&ct, &bad).is_err());
    }

    #[test]
    fn tampered_ciphertext_fails_tag() {
        let c = AuditCipher::new(&key());
        let (mut ct, nonce) = c.encrypt(b"secret").unwrap();
        ct[0] ^= 0xFF;
        assert!(c.decrypt(&ct, &nonce).is_err());
    }

    #[test]
    fn bad_nonce_len_is_rejected() {
        let c = AuditCipher::new(&key());
        let (ct, _) = c.encrypt(b"x").unwrap();
        assert!(matches!(
            c.decrypt(&ct, &[0u8; 8]),
            Err(CipherError::BadNonceLen(8))
        ));
    }
}
