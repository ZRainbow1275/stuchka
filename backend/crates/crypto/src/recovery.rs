//! §4.7 Three-way password backup (C-C-14) — exactly one is mandatory before normal operation
//! (§4.7.4); the boot wizard cannot be skipped.
//!
//! - **Path A · keychain** (R1a deep, §4.7.1): store/load the recovery key in the OS credential
//!   store. Windows Credential Manager via keyring 3.x (`windows-native` backend).
//! - **Path B · BIP39** (R1a logic, full UI R1b, §4.7.2): generate a Simplified-Chinese mnemonic and
//!   derive a KEK from it (HKDF-SHA256, `BIP39_INFO`).
//! - **Path C · USB recovery file** (R1a codec, full UI R1b, §4.7.3): `.stuchka-recovery` with a
//!   `STKR` magic, version, recovery DEK, salt, and a SHA-256 footer over everything preceding it.
//!
//! The chosen method is recorded in `data_model::user_setting::RecoveryMethod`.

use std::io::Read;
use std::path::Path;

use bip39::{Language, Mnemonic};
use rand_core::{OsRng, RngCore};
use sha2::{Digest, Sha256};

use crate::master_key::{hkdf_subkey, BIP39_INFO};
use crate::{CryptoError, Result};

/// Service name for the OS credential store entry (`backend/04` §4.7.1).
pub const KEYCHAIN_SERVICE: &str = "stuchka";
/// Account/user name for the OS credential store entry (`backend/04` §4.7.1).
pub const KEYCHAIN_ACCOUNT: &str = "master_key";

// ---------------------------------------------------------------------------------------------
// Path A · system keychain (Windows Credential Manager — R1a deep)
// ---------------------------------------------------------------------------------------------

/// Store the 32-byte recovery key in the Windows Credential Manager (`backend/04` §4.7.1).
///
/// keyring 3.x: `Entry::new(service, user)` returns a `Result`, and the secret API is
/// `set_secret(&[u8])` (the spec's `keyring 0.10` form is overruled — I-4).
#[cfg(target_os = "windows")]
pub fn store_keychain(key: &[u8; 32]) -> Result<()> {
    let entry = keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)
        .map_err(|e| CryptoError::Keychain(e.to_string()))?;
    entry
        .set_secret(key)
        .map_err(|e| CryptoError::Keychain(e.to_string()))
}

/// Load the 32-byte recovery key from the Windows Credential Manager (recovery path).
#[cfg(target_os = "windows")]
pub fn load_keychain() -> Result<[u8; 32]> {
    let entry = keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)
        .map_err(|e| CryptoError::Keychain(e.to_string()))?;
    let secret = entry
        .get_secret()
        .map_err(|e| CryptoError::Keychain(e.to_string()))?;
    secret
        .as_slice()
        .try_into()
        .map_err(|_| CryptoError::Keychain("stored secret is not 32 bytes".into()))
}

/// Delete the keychain entry (cleanup / re-enrolment).
#[cfg(target_os = "windows")]
pub fn delete_keychain() -> Result<()> {
    let entry = keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)
        .map_err(|e| CryptoError::Keychain(e.to_string()))?;
    entry
        .delete_credential()
        .map_err(|e| CryptoError::Keychain(e.to_string()))
}

// macOS / Linux: R1 ships Windows-only (§0.1); the credential-store backends land in R2. The
// function names exist so cross-platform callers compile, but report that the backend is absent.
#[cfg(not(target_os = "windows"))]
pub fn store_keychain(_key: &[u8; 32]) -> Result<()> {
    Err(CryptoError::Keychain(
        "keychain backend not available on this platform (R2)".into(),
    ))
}

#[cfg(not(target_os = "windows"))]
pub fn load_keychain() -> Result<[u8; 32]> {
    Err(CryptoError::Keychain(
        "keychain backend not available on this platform (R2)".into(),
    ))
}

#[cfg(not(target_os = "windows"))]
pub fn delete_keychain() -> Result<()> {
    Err(CryptoError::Keychain(
        "keychain backend not available on this platform (R2)".into(),
    ))
}

// ---------------------------------------------------------------------------------------------
// Path B · BIP39 mnemonic (R1a logic; full UI R1b)
// ---------------------------------------------------------------------------------------------

/// BIP39 entropy strength (`backend/04` §4.7.2): 12 words (128-bit) or 24 words (256-bit).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bip39Strength {
    /// 12 words — 16 bytes of entropy.
    Words12,
    /// 24 words — 32 bytes of entropy.
    Words24,
}

impl Bip39Strength {
    /// Entropy length in bytes.
    #[must_use]
    pub fn entropy_bytes(self) -> usize {
        match self {
            Bip39Strength::Words12 => 16,
            Bip39Strength::Words24 => 32,
        }
    }
}

/// Generate a Simplified-Chinese BIP39 mnemonic (`backend/04` §4.7.2). Entropy is drawn from
/// `OsRng` (W6 / I-6: a cryptographic OS source, not `thread_rng`).
pub fn generate_bip39(strength: Bip39Strength) -> Result<Mnemonic> {
    let mut entropy = vec![0u8; strength.entropy_bytes()];
    OsRng.fill_bytes(&mut entropy);
    Mnemonic::from_entropy_in(Language::SimplifiedChinese, &entropy)
        .map_err(|e| CryptoError::Bip39(e.to_string()))
}

/// Derive a 32-byte KEK from a mnemonic + passphrase (`backend/04` §4.7.2). Deterministic — the same
/// `(mnemonic, passphrase)` always yields the same KEK (CR-10). HKDF-SHA256 over the BIP39 seed with
/// `BIP39_INFO`.
#[must_use]
pub fn bip39_to_kek(mnemonic: &Mnemonic, passphrase: &str) -> [u8; 32] {
    let seed = mnemonic.to_seed(passphrase); // [u8; 64]
    let seed32: [u8; 32] = seed[..32]
        .try_into()
        .expect("BIP39 seed is 64 bytes, the first 32 always exist");
    hkdf_subkey(&seed32, BIP39_INFO)
}

/// `sha256(mnemonic.to_seed(""))` hex — written to `user_setting.bip39_check_hash` after the user
/// re-enters the words for verification (`backend/04` §4.7.2).
#[must_use]
pub fn bip39_check_hash(mnemonic: &Mnemonic) -> String {
    let seed = mnemonic.to_seed("");
    let digest = Sha256::digest(seed);
    hex(&digest)
}

// ---------------------------------------------------------------------------------------------
// Path C · USB recovery file `.stuchka-recovery` (R1a codec; full UI R1b)
// ---------------------------------------------------------------------------------------------

/// 4-byte magic for the `.stuchka-recovery` file (`backend/04` §4.7.3).
pub const RECOVERY_MAGIC: &[u8; 4] = b"STKR";
/// Current recovery-file format version.
pub const RECOVERY_VERSION: u16 = 1;

/// Decoded `.stuchka-recovery` payload (`backend/04` §4.7.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryFile {
    /// Format version.
    pub version: u16,
    /// The recovery DEK, stored in the clear (the user physically safeguards the medium — §4.7.3).
    pub recovery_dek: [u8; 32],
    /// Argon2id salt that pairs with the recovery DEK.
    pub salt: [u8; 16],
}

impl RecoveryFile {
    /// Construct a recovery file at the current version.
    #[must_use]
    pub fn new(recovery_dek: [u8; 32], salt: [u8; 16]) -> Self {
        RecoveryFile {
            version: RECOVERY_VERSION,
            recovery_dek,
            salt,
        }
    }

    /// Encode the on-disk layout: `STKR(4) || version(2 BE) || dek(32) || salt(16) || sha256(54)`.
    /// The footer SHA-256 covers everything preceding it.
    fn encode(&self) -> Vec<u8> {
        let mut body = Vec::with_capacity(4 + 2 + 32 + 16 + 32);
        body.extend_from_slice(RECOVERY_MAGIC);
        body.extend_from_slice(&self.version.to_be_bytes());
        body.extend_from_slice(&self.recovery_dek);
        body.extend_from_slice(&self.salt);
        let footer = Sha256::digest(&body);
        body.extend_from_slice(&footer);
        body
    }
}

/// Body length before the 32-byte footer: magic(4) + version(2) + dek(32) + salt(16).
const RECOVERY_BODY_LEN: usize = 4 + 2 + 32 + 16;
/// Full file length including the SHA-256 footer.
const RECOVERY_FILE_LEN: usize = RECOVERY_BODY_LEN + 32;

/// Write a `.stuchka-recovery` file (`backend/04` §4.7.3).
pub fn write_recovery_file(path: &Path, f: &RecoveryFile) -> Result<()> {
    std::fs::write(path, f.encode())?;
    Ok(())
}

/// Read and validate a `.stuchka-recovery` file (`backend/04` §4.7.3). A bad magic, a short file, or
/// a footer mismatch (any tampered byte) is [`CryptoError::RecoveryCorrupt`].
pub fn read_recovery_file(path: &Path) -> Result<RecoveryFile> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?.read_to_end(&mut bytes)?;
    decode_recovery(&bytes)
}

/// Decode + validate the recovery-file bytes (extracted so tests can tamper in memory).
pub fn decode_recovery(bytes: &[u8]) -> Result<RecoveryFile> {
    if bytes.len() != RECOVERY_FILE_LEN {
        return Err(CryptoError::RecoveryCorrupt);
    }
    if &bytes[..4] != RECOVERY_MAGIC {
        return Err(CryptoError::BadMagic);
    }
    let (body, footer) = bytes.split_at(RECOVERY_BODY_LEN);
    let expected = Sha256::digest(body);
    if footer != expected.as_slice() {
        return Err(CryptoError::RecoveryCorrupt);
    }
    let version = u16::from_be_bytes([body[4], body[5]]);
    let recovery_dek: [u8; 32] = body[6..38].try_into().expect("fixed offset");
    let salt: [u8; 16] = body[38..54].try_into().expect("fixed offset");
    Ok(RecoveryFile {
        version,
        recovery_dek,
        salt,
    })
}

// ---------------------------------------------------------------------------------------------
// §4.7.4 recovery-method validation (the boot wizard cannot be skipped)
// ---------------------------------------------------------------------------------------------

/// Validate the chosen recovery method (`backend/04` §4.7.4). `None` means the boot wizard has not
/// completed, which must block normal operation; any value other than the three known methods is
/// rejected. Reuses `data_model::user_setting::RecoveryMethod` so unknown strings are already
/// impossible to construct — this function additionally enforces the "must be set" rule.
pub fn validate_recovery_method(
    method: Option<data_model::user_setting::RecoveryMethod>,
) -> Result<data_model::user_setting::RecoveryMethod> {
    method.ok_or_else(|| {
        CryptoError::Keychain("recovery_method unset — boot wizard cannot be skipped".into())
    })
}

/// Hex-encode bytes.
fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use data_model::user_setting::RecoveryMethod;

    /// CR-10: 12/24 word entropy lengths are correct.
    #[test]
    fn bip39_strength_entropy_lengths() {
        assert_eq!(Bip39Strength::Words12.entropy_bytes(), 16);
        assert_eq!(Bip39Strength::Words24.entropy_bytes(), 32);
        let m12 = generate_bip39(Bip39Strength::Words12).unwrap();
        assert_eq!(m12.word_count(), 12);
        let m24 = generate_bip39(Bip39Strength::Words24).unwrap();
        assert_eq!(m24.word_count(), 24);
    }

    /// CR-10: bip39_to_kek is deterministic in (mnemonic, passphrase).
    #[test]
    fn bip39_to_kek_is_deterministic() {
        let m = generate_bip39(Bip39Strength::Words24).unwrap();
        let k1 = bip39_to_kek(&m, "pp");
        let k2 = bip39_to_kek(&m, "pp");
        assert_eq!(k1, k2);
        let k3 = bip39_to_kek(&m, "different");
        assert_ne!(k1, k3, "passphrase must affect the derived KEK");

        // a different mnemonic derives a different KEK
        let m2 = generate_bip39(Bip39Strength::Words24).unwrap();
        assert_ne!(bip39_to_kek(&m2, "pp"), k1);
    }

    /// The check hash is stable and 64 hex chars (sha256).
    #[test]
    fn bip39_check_hash_is_stable_sha256() {
        let m = generate_bip39(Bip39Strength::Words12).unwrap();
        let h1 = bip39_check_hash(&m);
        let h2 = bip39_check_hash(&m);
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64);
    }

    /// CR-11: STKR magic + SHA-256 footer round-trip; tampering any byte fails.
    #[test]
    fn recovery_file_round_trip_and_tamper() {
        let f = RecoveryFile::new([0x5Au8; 32], [0xC3u8; 16]);
        let dir = std::env::temp_dir().join(format!("stuchka-rec-{}", uuid::Uuid::now_v7()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(".stuchka-recovery");
        write_recovery_file(&path, &f).unwrap();

        let back = read_recovery_file(&path).unwrap();
        assert_eq!(back, f);

        // the encoded blob begins with STKR
        let raw = std::fs::read(&path).unwrap();
        assert_eq!(&raw[..4], RECOVERY_MAGIC);
        assert_eq!(raw.len(), RECOVERY_FILE_LEN);

        // tamper a DEK byte → footer mismatch → RecoveryCorrupt
        let mut bad = raw.clone();
        bad[10] ^= 0x01;
        assert!(matches!(
            decode_recovery(&bad),
            Err(CryptoError::RecoveryCorrupt)
        ));

        // tamper the magic → BadMagic
        let mut bad_magic = raw.clone();
        bad_magic[0] = b'Z';
        assert!(matches!(
            decode_recovery(&bad_magic),
            Err(CryptoError::BadMagic)
        ));

        // truncated → RecoveryCorrupt
        assert!(matches!(
            decode_recovery(&raw[..raw.len() - 1]),
            Err(CryptoError::RecoveryCorrupt)
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// CR-13: an unset recovery method blocks the wizard; a set one is returned.
    #[test]
    fn recovery_method_must_be_set() {
        assert!(validate_recovery_method(None).is_err());
        assert_eq!(
            validate_recovery_method(Some(RecoveryMethod::Keychain)).unwrap(),
            RecoveryMethod::Keychain
        );
    }

    /// CR-12 (Windows): keychain store → load round-trips. Skipped on non-Windows (no backend).
    #[cfg(target_os = "windows")]
    #[test]
    fn keychain_store_load_round_trip() {
        let key = [0x7Eu8; 32];
        // Best-effort: a CI runner without Credential Manager will surface a Keychain error rather
        // than panic; assert the round-trip only when the backend is actually present.
        if store_keychain(&key).is_ok() {
            let loaded = load_keychain().expect("load after successful store");
            assert_eq!(loaded, key);
            let _ = delete_keychain();
        }
    }
}
