//! `crypto` — FS encryption detect + age/pgcrypto + master key backup (INV-05)
//! 0529 spec crate (master-index §0.4 / D6, backend/04 §4.2–§4.5, §4.7).
//!
//! Module map (backend/04 §0.4):
//! - [`master_key`]  — §4.2 Argon2id KEK / DEK wrap-unwrap / unlock session + HKDF subkey primitive.
//! - [`fs_detect`]   — §4.3 file-system encryption probe (Windows BitLocker deep; macOS/Linux skeleton).
//! - [`field_cipher`]— §4.4.2 SQLite-fallback ChaCha20Poly1305 application-layer field cipher.
//! - [`pgcrypto`]    — §4.4.2 Postgres `pgp_sym_*` interface, gated behind `cfg(feature = "pg")` (L0-03).
//! - [`age_blob`]    — §4.5 high-sensitivity age external blob store; identity derived from DEK via HKDF.
//! - [`recovery`]    — §4.7 three-way password backup (keychain / BIP39 / USB recovery file).
//!
//! INV-05 / C-7 (negative contract): the session DEK wrapper [`master_key::SessionDek`] deliberately
//! does **not** implement `Serialize`, guaranteeing at compile time that the data-encryption key can
//! never be serialized into a Bearer-HTTP JSON DTO.
//!
//! INV-01: this crate depends only on `data-model` and pure crypto crates — never sqlx/tokio/
//! reqwest/axum/AI crates.

pub mod age_blob;
pub mod field_cipher;
pub mod fs_detect;
pub mod master_key;
#[cfg(feature = "pg")]
pub mod pgcrypto;
pub mod recovery;

// Re-exports for ergonomic downstream use.
pub use age_blob::{AgeStore, BlobRef};
pub use field_cipher::ChaChaFieldCipher;
pub use fs_detect::detect_fs_encryption;
pub use master_key::{
    derive_kek, hkdf_subkey, open_session, unwrap_dek, wrap_dek, EncryptedDek, FileKeystore,
    Keystore, SessionDek, AGE_INFO, ARGON2_M_COST, ARGON2_P_COST, ARGON2_T_COST, AUDIT_INFO,
    BIP39_INFO, DEK_MAGIC,
};
pub use recovery::{
    bip39_to_kek, generate_bip39, read_recovery_file, write_recovery_file, Bip39Strength,
    RecoveryFile, RECOVERY_MAGIC,
};
// Re-export the data-model FS/recovery enums so downstream crates (db/api) get one source of truth.
pub use data_model::user_setting::{FsEncryptionStatus, RecoveryMethod};

/// Crate identity for boot diagnostics and CI dependency-graph assertions.
pub const CRATE_NAME: &str = "crypto";

/// Crate-wide result alias (`backend/04` uses `Result<_>` throughout without defining the error).
pub type Result<T> = std::result::Result<T, CryptoError>;

/// Unified crypto error (`backend/04` references `Result<_>`/`Error::WrongPassword`/`AgeBadFormat`
/// without a definition; this is the canonical set).
#[derive(thiserror::Error, Debug)]
pub enum CryptoError {
    /// §4.2.3 unlock failure — the DEK AEAD tag did not verify (mapped from the AEAD error so the
    /// caller never learns whether it was a tampered blob or a wrong password).
    #[error("wrong master password")]
    WrongPassword,
    /// Argon2id KEK derivation failed (§4.2.1).
    #[error("argon2 derivation failed: {0}")]
    Argon2(String),
    /// ChaCha20Poly1305 seal/open failed (tag mismatch or bad input) (§4.2.1 / §4.4.2).
    #[error("AEAD seal/open failed")]
    Aead,
    /// age blob had a bad/unknown header (§4.5.2 read path).
    #[error("age blob bad format")]
    AgeBadFormat,
    /// age encrypt/decrypt pipeline error (§4.5.2).
    #[error("age pipeline error: {0}")]
    Age(String),
    /// Filesystem IO on a keystore/blob/recovery file.
    #[error("keystore io: {0}")]
    KeystoreIo(#[from] std::io::Error),
    /// Bad 4-byte magic (`STKK` keystore §4.2.2 / `STKR` recovery §4.7.3).
    #[error("keystore corrupt: bad magic")]
    BadMagic,
    /// CBOR (de)serialization of `EncryptedDek` failed (§4.2.2).
    #[error("cbor codec: {0}")]
    Cbor(String),
    /// System keychain backend error (§4.7.1).
    #[error("keychain backend: {0}")]
    Keychain(String),
    /// BIP39 mnemonic generation/parse error (§4.7.2).
    #[error("bip39: {0}")]
    Bip39(String),
    /// `.stuchka-recovery` file corrupt — bad magic, short read, or footer hash mismatch (§4.7.3).
    #[error("recovery file corrupt")]
    RecoveryCorrupt,
    /// FS detection command unavailable / unsupported platform (§4.3).
    #[error("fs detection unavailable")]
    FsDetectUnavailable,
}

#[cfg(test)]
mod tests {
    #[test]
    fn crate_name_is_stable() {
        assert_eq!(super::CRATE_NAME, "crypto");
    }
}
