//! §4.2 Master key — Argon2id KEK derivation, DEK wrap/unwrap, unlock session, HKDF subkey.
//!
//! Layer-0 of the encryption model (backend/04 §4.1):
//! - The user password is stretched to a 32-byte **KEK** (key-encryption key) with Argon2id
//!   (m=64MB, t=3, p=4) — [`derive_kek`].
//! - A random 32-byte **DEK** (data-encryption key) is wrapped under the KEK with
//!   ChaCha20Poly1305 and persisted as an [`EncryptedDek`] — [`wrap_dek`] / [`unwrap_dek`].
//! - On boot, [`open_session`] re-derives the KEK from the password, unwraps the DEK, and hands
//!   back a [`SessionDek`] that lives only in process memory, is zeroized on drop, and (INV-05)
//!   cannot be serialized.
//! - [`hkdf_subkey`] is the single HKDF-SHA256 primitive reused by `age_blob` (age identity) and
//!   `crates/audit` (audit key) — see C-3.

use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::{AeadCore, AeadInPlace, KeyInit, OsRng};
use chacha20poly1305::{ChaCha20Poly1305, Nonce};
use chrono::{DateTime, Utc};
use hkdf::Hkdf;
use rand_core::RngCore;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::path::PathBuf;
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::{CryptoError, Result};

/// Argon2id memory cost: 64 MiB (`backend/04` §4.2.1; OWASP 2025 m≥19MiB lower bound — well above).
pub const ARGON2_M_COST: u32 = 64 * 1024;
/// Argon2id time cost (iterations) (`backend/04` §4.2.1).
pub const ARGON2_T_COST: u32 = 3;
/// Argon2id parallelism (`backend/04` §4.2.1).
pub const ARGON2_P_COST: u32 = 4;

/// 4-byte keystore magic prefixing the CBOR `EncryptedDek` in `dek.enc` (`backend/04` §4.2.2).
pub const DEK_MAGIC: &[u8; 4] = b"STKK";

/// HKDF `info` string for the age x25519 identity (I-9: single source of truth, no scattered
/// literals). Used by [`crate::age_blob::AgeStore::from_dek`].
pub const AGE_INFO: &[u8] = b"stuchka-age-identity-v1";
/// HKDF `info` string for the audit key (`backend/04` §4.8); exported for `crates/audit` (C-3).
pub const AUDIT_INFO: &[u8] = b"stuchka-audit-key-v1";
/// HKDF `info` string for the BIP39-derived KEK (`backend/04` §4.7.2).
pub const BIP39_INFO: &[u8] = b"stuchka-bip39-kek-v1";

/// HKDF-SHA256 subkey derivation — the single key-derivation primitive of the crate.
///
/// Deterministic: same `(dek, info)` always yields the same 32-byte subkey, which is exactly what
/// lets the age identity (§4.5.2) and audit key (§4.8) be re-derived across process restarts. Uses
/// no salt (the DEK is already a uniformly random 32-byte key, so HKDF-Expand-only semantics with a
/// zero salt are appropriate). C-3.
#[must_use]
pub fn hkdf_subkey(dek: &[u8; 32], info: &[u8]) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(None, dek);
    let mut out = [0u8; 32];
    hk.expand(info, &mut out)
        .expect("32 bytes is a valid HKDF-SHA256 output length");
    out
}

/// Derive the 32-byte KEK from the master password and a per-keystore salt (`backend/04` §4.2.1).
///
/// Argon2id, version 0x13, m=64MiB, t=3, p=4, 32-byte output. Deterministic for a given
/// `(password, salt)`.
pub fn derive_kek(password: &str, salt: &[u8; 16]) -> Result<[u8; 32]> {
    let params = Params::new(ARGON2_M_COST, ARGON2_T_COST, ARGON2_P_COST, Some(32))
        .map_err(|e| CryptoError::Argon2(e.to_string()))?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut out = [0u8; 32];
    argon
        .hash_password_into(password.as_bytes(), salt, &mut out)
        .map_err(|e| CryptoError::Argon2(e.to_string()))?;
    Ok(out)
}

/// The wrapped DEK as persisted to `<app_data>/keystore/dek.enc` (`backend/04` §4.2.1 / §4.2.2).
///
/// `created_at` uses `chrono::DateTime<Utc>` (W2 / I-1: the workspace is chrono-only; the spec's
/// `jiff::Timestamp` is overruled). CBOR-encoded behind a `STKK` magic — see [`FileKeystore`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EncryptedDek {
    /// Argon2id salt for re-deriving the KEK.
    pub salt: [u8; 16],
    /// ChaCha20Poly1305 nonce used to wrap the DEK (random per wrap).
    pub nonce: [u8; 12],
    /// `ChaCha20Poly1305(KEK, nonce, DEK)` — ciphertext with the 16-byte Poly1305 tag appended.
    pub ciphertext: Vec<u8>,
    /// Creation instant (chrono UTC, W2).
    pub created_at: DateTime<Utc>,
}

/// Wrap a random DEK under the KEK (`backend/04` §4.2.1).
///
/// A fresh random 12-byte nonce is drawn per wrap (no nonce reuse). The output `ciphertext`
/// contains the encrypted DEK followed by the Poly1305 tag.
pub fn wrap_dek(kek: &[u8; 32], dek: &[u8; 32]) -> Result<EncryptedDek> {
    let mut salt = [0u8; 16];
    OsRng.fill_bytes(&mut salt);
    wrap_dek_with_salt(kek, dek, salt)
}

/// Wrap a DEK under the KEK, recording the `salt` that produced the KEK so it can be re-derived on
/// unlock. Most callers want [`wrap_dek`]; this variant is for the bootstrap path that derived the
/// KEK from a known salt.
pub fn wrap_dek_with_salt(kek: &[u8; 32], dek: &[u8; 32], salt: [u8; 16]) -> Result<EncryptedDek> {
    let cipher = ChaCha20Poly1305::new_from_slice(kek).map_err(|_| CryptoError::Aead)?;
    let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng);
    let mut buf = dek.to_vec();
    cipher
        .encrypt_in_place(&nonce, b"", &mut buf)
        .map_err(|_| CryptoError::Aead)?;
    let nonce_bytes: [u8; 12] = nonce.into();
    Ok(EncryptedDek {
        salt,
        nonce: nonce_bytes,
        ciphertext: buf,
        created_at: Utc::now(),
    })
}

/// Unwrap the DEK using the KEK (`backend/04` §4.2.1). A failing AEAD tag (wrong KEK / tampered
/// blob) surfaces as [`CryptoError::Aead`]; the unlock path maps that to
/// [`CryptoError::WrongPassword`] — see [`open_session`].
pub fn unwrap_dek(kek: &[u8; 32], wrapped: &EncryptedDek) -> Result<[u8; 32]> {
    let cipher = ChaCha20Poly1305::new_from_slice(kek).map_err(|_| CryptoError::Aead)?;
    let nonce = Nonce::from_slice(&wrapped.nonce);
    let mut buf = wrapped.ciphertext.clone();
    cipher
        .decrypt_in_place(nonce, b"", &mut buf)
        .map_err(|_| CryptoError::Aead)?;
    let dek: [u8; 32] = buf.as_slice().try_into().map_err(|_| CryptoError::Aead)?;
    buf.zeroize();
    Ok(dek)
}

/// Session-resident data-encryption key (`backend/04` §4.4.3).
///
/// Held in memory only, zeroized on drop, and **never leaves the backend process** — it is not
/// passed to Flutter and never written to logs. INV-05 / C-7: this type deliberately does **not**
/// implement `Serialize`/`Deserialize`, so the compiler refuses any attempt to put the DEK into a
/// Bearer-HTTP JSON DTO.
///
/// CR-14 (compile-time INV-05): the following must NOT compile, because `SessionDek: !Serialize`.
///
/// ```compile_fail
/// fn assert_serialize<T: serde::Serialize>() {}
/// assert_serialize::<crypto::master_key::SessionDek>();
/// ```
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct SessionDek([u8; 32]);

impl SessionDek {
    /// Wrap raw DEK bytes into the zeroizing session holder.
    #[must_use]
    pub fn new(dek: [u8; 32]) -> Self {
        SessionDek(dek)
    }

    /// Borrow the raw key bytes for an AEAD operation. Callers must not copy these out of the
    /// process boundary (C-7); used internally by [`crate::field_cipher`] / [`crate::age_blob`].
    #[must_use]
    pub fn expose(&self) -> &[u8; 32] {
        &self.0
    }
}

impl std::fmt::Debug for SessionDek {
    /// Redacts the key material — the DEK must never appear in logs (§4.4.3).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SessionDek(<redacted>)")
    }
}

/// Re-derive the KEK from the password, unwrap the DEK, and return the session-resident key
/// (`backend/04` §4.2.3, split out of the api-side `unlock`). A wrong password / tampered blob maps
/// to [`CryptoError::WrongPassword`]. The 3-strike 5-minute cooldown (INV-10) belongs to the api
/// layer, not here.
pub fn open_session(wrapped: &EncryptedDek, password: &str) -> Result<SessionDek> {
    let kek = derive_kek(password, &wrapped.salt)?;
    let dek = unwrap_dek(&kek, wrapped).map_err(|_| CryptoError::WrongPassword)?;
    Ok(SessionDek::new(dek))
}

/// Keystore abstraction for `dek.enc` (`backend/04` §4.2.2 / §4.2.3). The api layer reads on boot to
/// drive the unlock flow; `exists()` decides first-run bootstrapping.
pub trait Keystore {
    /// Read the persisted wrapped DEK.
    fn read_dek(&self) -> Result<EncryptedDek>;
    /// Persist a wrapped DEK.
    fn write_dek(&self, dek: &EncryptedDek) -> Result<()>;
    /// Whether a keystore already exists (first boot → bootstrap wizard).
    fn exists(&self) -> bool;
}

/// File-backed keystore at `<app_data>/keystore/dek.enc` — `STKK` magic followed by CBOR-encoded
/// [`EncryptedDek`] (`backend/04` §4.2.2). ciborium (W7 / I-7: serde_cbor is archived).
pub struct FileKeystore {
    /// Path to `dek.enc`.
    pub path: PathBuf,
}

impl FileKeystore {
    /// Construct a keystore at the given `dek.enc` path.
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        FileKeystore { path }
    }

    /// Encode `STKK || CBOR(EncryptedDek)` into a byte vector (shared by [`Keystore::write_dek`] and
    /// tests for the CR-04 magic/round-trip assertion).
    pub fn encode(dek: &EncryptedDek) -> Result<Vec<u8>> {
        let mut out = DEK_MAGIC.to_vec();
        let mut body = Vec::new();
        ciborium::into_writer(dek, &mut body).map_err(|e| CryptoError::Cbor(e.to_string()))?;
        out.extend_from_slice(&body);
        Ok(out)
    }

    /// Decode `STKK || CBOR(EncryptedDek)`; a wrong magic is [`CryptoError::BadMagic`].
    pub fn decode(bytes: &[u8]) -> Result<EncryptedDek> {
        if bytes.len() < 4 || &bytes[..4] != DEK_MAGIC {
            return Err(CryptoError::BadMagic);
        }
        ciborium::from_reader(&bytes[4..]).map_err(|e| CryptoError::Cbor(e.to_string()))
    }
}

impl Keystore for FileKeystore {
    fn read_dek(&self) -> Result<EncryptedDek> {
        let bytes = std::fs::read(&self.path)?;
        Self::decode(&bytes)
    }

    fn write_dek(&self, dek: &EncryptedDek) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&self.path, Self::encode(dek)?)?;
        Ok(())
    }

    fn exists(&self) -> bool {
        self.path.exists()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixed_salt() -> [u8; 16] {
        [7u8; 16]
    }

    fn fixed_dek() -> [u8; 32] {
        [0xABu8; 32]
    }

    /// CR-03: Argon2id is deterministic for a given (password, salt), output is 32 bytes.
    #[test]
    fn derive_kek_is_deterministic_32_bytes() {
        let salt = fixed_salt();
        let k1 = derive_kek("correct horse battery staple", &salt).unwrap();
        let k2 = derive_kek("correct horse battery staple", &salt).unwrap();
        assert_eq!(k1, k2, "same password+salt must yield same KEK");
        assert_eq!(k1.len(), 32);
        let k3 = derive_kek("different password", &salt).unwrap();
        assert_ne!(k1, k3, "different password must yield different KEK");
    }

    /// CR-01: derive_kek → wrap_dek → unwrap_dek round-trips to the original DEK.
    #[test]
    fn kek_dek_round_trip_identity() {
        let salt = fixed_salt();
        let kek = derive_kek("pw-123", &salt).unwrap();
        let dek = fixed_dek();
        let wrapped = wrap_dek_with_salt(&kek, &dek, salt).unwrap();
        let back = unwrap_dek(&kek, &wrapped).unwrap();
        assert_eq!(back, dek);
    }

    /// CR-02: a wrong password unwraps to Err (AEAD tag fails) — open_session maps to WrongPassword.
    #[test]
    fn wrong_password_is_rejected() {
        let salt = fixed_salt();
        let kek = derive_kek("right-pw", &salt).unwrap();
        let dek = fixed_dek();
        let wrapped = wrap_dek_with_salt(&kek, &dek, salt).unwrap();

        // unwrap with the wrong KEK
        let wrong_kek = derive_kek("wrong-pw", &salt).unwrap();
        assert!(matches!(
            unwrap_dek(&wrong_kek, &wrapped),
            Err(CryptoError::Aead)
        ));

        // open_session surfaces WrongPassword
        assert!(matches!(
            open_session(&wrapped, "wrong-pw"),
            Err(CryptoError::WrongPassword)
        ));
        // and succeeds with the right one
        let sess = open_session(&wrapped, "right-pw").unwrap();
        assert_eq!(sess.expose(), &dek);
    }

    /// CR-04: STKK magic + CBOR round-trip; a bad magic is rejected.
    #[test]
    fn encrypted_dek_cbor_magic_round_trip() {
        let salt = fixed_salt();
        let kek = derive_kek("pw", &salt).unwrap();
        let wrapped = wrap_dek_with_salt(&kek, &fixed_dek(), salt).unwrap();

        let bytes = FileKeystore::encode(&wrapped).unwrap();
        assert_eq!(&bytes[..4], DEK_MAGIC, "STKK magic must prefix the blob");

        let decoded = FileKeystore::decode(&bytes).unwrap();
        assert_eq!(decoded, wrapped);

        // tamper the magic
        let mut bad = bytes.clone();
        bad[0] = b'X';
        assert!(matches!(
            FileKeystore::decode(&bad),
            Err(CryptoError::BadMagic)
        ));
    }

    /// hkdf_subkey is deterministic and info-separated (basis for CR-07 age identity reuse + C-3).
    #[test]
    fn hkdf_subkey_deterministic_and_separated() {
        let dek = fixed_dek();
        let a1 = hkdf_subkey(&dek, AGE_INFO);
        let a2 = hkdf_subkey(&dek, AGE_INFO);
        assert_eq!(a1, a2, "same (dek,info) → same subkey");
        let audit = hkdf_subkey(&dek, AUDIT_INFO);
        assert_ne!(a1, audit, "different info → different subkey");
    }

    /// FileKeystore write/read round-trips through a temp dir.
    #[test]
    fn file_keystore_write_read_round_trip() {
        let dir = std::env::temp_dir().join(format!("stuchka-ks-{}", uuid::Uuid::now_v7()));
        let ks = FileKeystore::new(dir.join("keystore").join("dek.enc"));
        assert!(!ks.exists());
        let salt = fixed_salt();
        let kek = derive_kek("pw", &salt).unwrap();
        let wrapped = wrap_dek_with_salt(&kek, &fixed_dek(), salt).unwrap();
        ks.write_dek(&wrapped).unwrap();
        assert!(ks.exists());
        let back = ks.read_dek().unwrap();
        assert_eq!(back, wrapped);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// CR-15 (best effort): the Debug impl never reveals key bytes.
    #[test]
    fn session_dek_debug_is_redacted() {
        let sess = SessionDek::new(fixed_dek());
        assert_eq!(format!("{sess:?}"), "SessionDek(<redacted>)");
    }
}
