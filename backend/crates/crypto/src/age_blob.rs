//! §4.5 High-sensitivity external blob store (age).
//!
//! Extremely sensitive content (audio transcripts, medical OCR, high-sensitivity evidence files —
//! §4.5.1) is never stored as plaintext in the DB. It is encrypted to a standalone `age` file under
//! `<app_data>/blobs/`; the DB keeps only the path + sha256 + size (§4.5.3).
//!
//! The age x25519 identity is **derived deterministically from the DEK** via HKDF-SHA256
//! (`info = AGE_INFO`), so it survives process restarts and never needs separate persistence
//! (§4.5.2, C-3). The derived 32 bytes are encoded as a standard `AGE-SECRET-KEY-…` bech32 string
//! (matching age's own `Identity::to_string`) and parsed back into [`age::x25519::Identity`].
//!
//! age 0.11 API (verified): `Encryptor::with_recipients(iter)` returns `Result`, and `Decryptor` is
//! a struct (no longer the old `Decryptor::Recipients(_)` enum) whose `decrypt(identities)` yields a
//! reader. INV-05 (I-3 of the brief) resolved.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::str::FromStr;

use age::x25519::Identity;
use bech32::{ToBase32, Variant};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::master_key::{hkdf_subkey, SessionDek, AGE_INFO};
use crate::{CryptoError, Result};

/// Reference to a stored age blob (`backend/04` §4.5.2 / §4.5.3).
///
/// `id` is `blb_{uuid_v7}` (I-2 / D9: ULID is abolished, so a v7 UUID is used as the blob file
/// name). `path` + `sha256` land in `evidence.file_path_enc` + `metadata.blob_sha256`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlobRef {
    /// `blb_{uuid_v7}` blob identifier / file name.
    pub id: String,
    /// Absolute path to the age-encrypted file under `<app_data>/blobs/`.
    pub path: String,
    /// Hex SHA-256 of the **plaintext** (integrity reference; the file on disk is ciphertext).
    pub sha256: String,
}

/// age-backed high-sensitivity blob store (`backend/04` §4.5.2).
pub struct AgeStore {
    /// `<app_data>/blobs/` directory.
    pub blob_dir: PathBuf,
    /// x25519 identity derived from the DEK (HKDF-SHA256, `AGE_INFO`).
    identity: Identity,
}

impl AgeStore {
    /// Build a store whose identity is derived from the session DEK (`backend/04` §4.5.2). The same
    /// DEK always yields the same identity (CR-07), so blobs are readable across restarts.
    pub fn from_dek(dek: &SessionDek, blob_dir: PathBuf) -> Result<Self> {
        let identity = derive_identity(dek.expose())?;
        Ok(AgeStore { blob_dir, identity })
    }

    /// Encrypt `plaintext` to a fresh age file and return its [`BlobRef`] (`backend/04` §4.5.2).
    pub fn write(&self, plaintext: &[u8]) -> Result<BlobRef> {
        std::fs::create_dir_all(&self.blob_dir)?;
        let blob_id = format!("blb_{}", Uuid::now_v7());
        let path = self.blob_dir.join(&blob_id);

        let recipient = self.identity.to_public();
        let encryptor =
            age::Encryptor::with_recipients(std::iter::once(&recipient as &dyn age::Recipient))
                .map_err(|e| CryptoError::Age(e.to_string()))?;

        let out = std::fs::File::create(&path)?;
        let mut writer = encryptor
            .wrap_output(out)
            .map_err(|e| CryptoError::Age(e.to_string()))?;
        writer.write_all(plaintext)?;
        writer
            .finish()
            .map_err(|e| CryptoError::Age(e.to_string()))?;

        Ok(BlobRef {
            id: blob_id,
            path: path.to_string_lossy().into_owned(),
            sha256: sha256_hex(plaintext),
        })
    }

    /// Decrypt a blob referenced by `blob_ref` (`backend/04` §4.5.2). A non-age / unknown header is
    /// [`CryptoError::AgeBadFormat`].
    pub fn read(&self, blob_ref: &BlobRef) -> Result<Vec<u8>> {
        let f = std::fs::File::open(&blob_ref.path)?;
        let decryptor = age::Decryptor::new(f).map_err(map_decrypt_err)?;
        let mut reader = decryptor
            .decrypt(std::iter::once(&self.identity as &dyn age::Identity))
            .map_err(map_decrypt_err)?;
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf)?;
        Ok(buf)
    }
}

/// Derive an age x25519 [`Identity`] deterministically from 32 DEK bytes (HKDF-SHA256, `AGE_INFO`).
///
/// The HKDF output is encoded as a standard `AGE-SECRET-KEY-…` bech32 string — exactly the form age
/// itself emits from `Identity::to_string` — and parsed back via `FromStr`, since age exposes no
/// "from raw bytes" constructor.
fn derive_identity(dek: &[u8; 32]) -> Result<Identity> {
    let sk = hkdf_subkey(dek, AGE_INFO);
    // age uses a lower-case HRP "age-secret-key-" then upper-cases the whole bech32 string.
    let encoded = bech32::encode("age-secret-key-", sk.to_base32(), Variant::Bech32)
        .map_err(|e| CryptoError::Age(format!("bech32 encode: {e}")))?
        .to_uppercase();
    Identity::from_str(&encoded).map_err(|e| CryptoError::Age(format!("identity parse: {e}")))
}

/// Map an age decrypt-side error: an unknown/garbage header is a "bad format", everything else is a
/// generic pipeline error.
fn map_decrypt_err(e: age::DecryptError) -> CryptoError {
    match e {
        age::DecryptError::InvalidHeader | age::DecryptError::UnknownFormat => {
            CryptoError::AgeBadFormat
        }
        other => CryptoError::Age(other.to_string()),
    }
}

/// Hex SHA-256 of `data` (`backend/04` §4.5.2 `BlobRef.sha256`).
#[must_use]
pub fn sha256_hex(data: &[u8]) -> String {
    let digest = Sha256::digest(data);
    let mut s = String::with_capacity(64);
    for b in digest {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store(dek_byte: u8) -> (AgeStore, PathBuf) {
        let dir = std::env::temp_dir().join(format!("stuchka-age-{}", Uuid::now_v7()));
        let dek = SessionDek::new([dek_byte; 32]);
        (AgeStore::from_dek(&dek, dir.clone()).unwrap(), dir)
    }

    /// CR-06: write → read round-trip; BlobRef.sha256 == sha256(plaintext).
    #[test]
    fn age_round_trip_and_sha256() {
        let (st, dir) = store(0x11);
        let plain = b"recording transcript: \xe5\xbd\x95\xe9\x9f\xb3\xe8\xbd\xac\xe5\x86\x99";
        let r = st.write(plain).unwrap();
        assert_eq!(r.sha256, sha256_hex(plain));
        assert!(r.id.starts_with("blb_"));
        let back = st.read(&r).unwrap();
        assert_eq!(back, plain);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// CR-08: the on-disk blob content is NOT the plaintext (it is age ciphertext).
    #[test]
    fn blob_file_is_not_plaintext() {
        let (st, dir) = store(0x22);
        let plain = b"the quick brown fox";
        let r = st.write(plain).unwrap();
        let on_disk = std::fs::read(&r.path).unwrap();
        assert_ne!(on_disk.as_slice(), plain.as_slice());
        // age files begin with the "age-encryption.org/v1" header.
        assert!(on_disk.starts_with(b"age-encryption.org/v1"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// CR-07: the identity is deterministic in the DEK — a store rebuilt from the same DEK reads
    /// blobs written by an earlier store (cross-process reproducibility).
    #[test]
    fn identity_is_deterministic_in_dek() {
        let dir = std::env::temp_dir().join(format!("stuchka-age-det-{}", Uuid::now_v7()));
        let dek = SessionDek::new([0x33; 32]);
        let r = {
            let st = AgeStore::from_dek(&dek, dir.clone()).unwrap();
            st.write(b"persist across restart").unwrap()
        };
        // A freshly derived store (simulating a restart) reads the same blob.
        let st2 = AgeStore::from_dek(&dek, dir.clone()).unwrap();
        assert_eq!(st2.read(&r).unwrap(), b"persist across restart");

        // A different DEK derives a different identity and cannot decrypt.
        let other = SessionDek::new([0x99; 32]);
        let st3 = AgeStore::from_dek(&other, dir.clone()).unwrap();
        assert!(st3.read(&r).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// CR-06 (bad format): a non-age file yields AgeBadFormat.
    #[test]
    fn bad_header_is_age_bad_format() {
        let (st, dir) = store(0x44);
        std::fs::create_dir_all(&dir).unwrap();
        let bad_path = dir.join("not-an-age-file");
        std::fs::write(&bad_path, b"this is plainly not age").unwrap();
        let bad_ref = BlobRef {
            id: "blb_x".into(),
            path: bad_path.to_string_lossy().into_owned(),
            sha256: String::new(),
        };
        assert!(matches!(st.read(&bad_ref), Err(CryptoError::AgeBadFormat)));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
