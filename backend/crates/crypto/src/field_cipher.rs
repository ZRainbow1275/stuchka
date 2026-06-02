//! §4.4.2 SQLite-fallback application-layer field cipher.
//!
//! When pgcrypto is unavailable (the L0-03 SQLite path, which is the R1a default — see the brief
//! §8 / spike §5.7.2), middle-sensitivity DB fields (`case.first_description_enc`,
//! `evidence.file_path_enc`, `provider_config.api_key_enc`) are encrypted in the application layer
//! with ChaCha20Poly1305 keyed directly by the 32-byte DEK (§4.4.3). The DB stores the BLOB layout
//! `nonce(12) || ciphertext+tag`.
//!
//! A fresh random nonce is drawn per field write (W5: random nonce, never derived) — re-encrypting
//! the same plaintext therefore yields different ciphertext, which is the intended behaviour.

use chacha20poly1305::aead::{AeadCore, AeadInPlace, KeyInit, OsRng};
use chacha20poly1305::{ChaCha20Poly1305, Nonce};

use crate::master_key::SessionDek;
use crate::{CryptoError, Result};

/// Length of the ChaCha20Poly1305 nonce prefix in the stored BLOB.
const NONCE_LEN: usize = 12;

/// Application-layer field cipher (`backend/04` §4.4.2, SQLite fallback path).
///
/// Holds a reference to the session DEK (which never leaves the process, §4.4.3). Produces and
/// consumes the BLOB layout `nonce(12) || ciphertext`.
pub struct ChaChaFieldCipher<'dek> {
    dek: &'dek SessionDek,
}

impl<'dek> ChaChaFieldCipher<'dek> {
    /// Bind the cipher to a session DEK.
    #[must_use]
    pub fn new(dek: &'dek SessionDek) -> Self {
        ChaChaFieldCipher { dek }
    }

    fn cipher(&self) -> Result<ChaCha20Poly1305> {
        ChaCha20Poly1305::new_from_slice(self.dek.expose()).map_err(|_| CryptoError::Aead)
    }

    /// Encrypt a plaintext field, returning the DB BLOB `nonce(12) || ciphertext+tag`
    /// (`backend/04` §4.4.2). A random nonce is drawn per call (W5).
    pub fn encrypt(&self, plain: &str) -> Result<Vec<u8>> {
        let cipher = self.cipher()?;
        let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng);
        let mut buf = plain.as_bytes().to_vec();
        cipher
            .encrypt_in_place(&nonce, b"", &mut buf)
            .map_err(|_| CryptoError::Aead)?;
        let mut out = Vec::with_capacity(NONCE_LEN + buf.len());
        out.extend_from_slice(nonce.as_slice());
        out.extend_from_slice(&buf);
        Ok(out)
    }

    /// Decrypt a DB BLOB produced by [`encrypt`](Self::encrypt). A truncated blob or a failing
    /// Poly1305 tag (any tampered byte) surfaces as [`CryptoError::Aead`].
    pub fn decrypt(&self, blob: &[u8]) -> Result<String> {
        if blob.len() < NONCE_LEN {
            return Err(CryptoError::Aead);
        }
        let cipher = self.cipher()?;
        let (nonce_bytes, ct) = blob.split_at(NONCE_LEN);
        let nonce = Nonce::from_slice(nonce_bytes);
        let mut buf = ct.to_vec();
        cipher
            .decrypt_in_place(nonce, b"", &mut buf)
            .map_err(|_| CryptoError::Aead)?;
        String::from_utf8(buf).map_err(|_| CryptoError::Aead)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dek() -> SessionDek {
        SessionDek::new([0x42u8; 32])
    }

    /// CR-05 (round-trip + layout): encrypt → decrypt recovers the plaintext; the BLOB starts with
    /// a 12-byte nonce.
    #[test]
    fn field_round_trip_and_layout() {
        let d = dek();
        let fc = ChaChaFieldCipher::new(&d);
        let plain = "13800001111 身份证 110101199003078888";
        let blob = fc.encrypt(plain).unwrap();
        assert!(
            blob.len() >= NONCE_LEN + plain.len() + 16,
            "nonce + ct + tag"
        );
        assert_eq!(fc.decrypt(&blob).unwrap(), plain);
    }

    /// CR-05 (tamper): flipping any ciphertext byte fails the Poly1305 tag.
    #[test]
    fn tampered_ciphertext_is_rejected() {
        let d = dek();
        let fc = ChaChaFieldCipher::new(&d);
        let mut blob = fc.encrypt("secret").unwrap();
        // flip the last byte (inside the tag)
        let last = blob.len() - 1;
        blob[last] ^= 0x01;
        assert!(matches!(fc.decrypt(&blob), Err(CryptoError::Aead)));

        // flip a ciphertext byte just after the nonce
        let mut blob2 = fc.encrypt("secret").unwrap();
        blob2[NONCE_LEN] ^= 0xFF;
        assert!(matches!(fc.decrypt(&blob2), Err(CryptoError::Aead)));
    }

    /// A nonce is random per call, so the same plaintext encrypts to distinct blobs (W5).
    #[test]
    fn nonce_is_random_per_call() {
        let d = dek();
        let fc = ChaChaFieldCipher::new(&d);
        let a = fc.encrypt("same").unwrap();
        let b = fc.encrypt("same").unwrap();
        assert_ne!(
            a, b,
            "random nonce → distinct ciphertext for identical input"
        );
        assert_eq!(fc.decrypt(&a).unwrap(), "same");
        assert_eq!(fc.decrypt(&b).unwrap(), "same");
    }

    /// A truncated blob (shorter than the nonce) is rejected, not panicked on.
    #[test]
    fn short_blob_is_rejected() {
        let d = dek();
        let fc = ChaChaFieldCipher::new(&d);
        assert!(matches!(fc.decrypt(&[0u8; 5]), Err(CryptoError::Aead)));
    }
}
