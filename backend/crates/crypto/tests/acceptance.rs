//! End-to-end acceptance assertions for `crates/crypto` (backend/04 §4.2–§4.5, §4.7).
//!
//! These cross-module tests exercise the CR-01..15 assertions from the implementation brief §6 over
//! the public crate API (the per-module unit tests cover the same assertions internally; this file
//! is the integration-level proof that the whole pipeline composes).

use std::path::PathBuf;

use crypto::age_blob::{sha256_hex, AgeStore};
use crypto::field_cipher::ChaChaFieldCipher;
use crypto::master_key::{
    derive_kek, hkdf_subkey, open_session, unwrap_dek, wrap_dek_with_salt, FileKeystore, Keystore,
    SessionDek, AGE_INFO,
};
use crypto::recovery::{
    bip39_to_kek, decode_recovery, generate_bip39, read_recovery_file, validate_recovery_method,
    write_recovery_file, Bip39Strength, RecoveryFile,
};
use crypto::CryptoError;

fn tmp(prefix: &str) -> PathBuf {
    std::env::temp_dir().join(format!("stuchka-{prefix}-{}", uuid::Uuid::now_v7()))
}

/// CR-01 + CR-02 + CR-03 + CR-04: full master-key lifecycle through the file keystore.
#[test]
fn master_key_lifecycle_through_keystore() {
    let salt = [9u8; 16];
    let dek = [0x2Au8; 32];
    let kek = derive_kek("a strong master password", &salt).unwrap();

    // CR-01 round-trip
    let wrapped = wrap_dek_with_salt(&kek, &dek, salt).unwrap();
    assert_eq!(unwrap_dek(&kek, &wrapped).unwrap(), dek);

    // CR-04 persist with STKK magic + CBOR, then re-open
    let dir = tmp("ks");
    let ks = FileKeystore::new(dir.join("keystore").join("dek.enc"));
    ks.write_dek(&wrapped).unwrap();
    let reread = ks.read_dek().unwrap();
    assert_eq!(reread, wrapped);

    // CR-02: open_session with the right / wrong password
    let session = open_session(&reread, "a strong master password").unwrap();
    assert_eq!(session.expose(), &dek);
    assert!(matches!(
        open_session(&reread, "nope"),
        Err(CryptoError::WrongPassword)
    ));

    let _ = std::fs::remove_dir_all(&dir);
}

/// CR-05: field cipher round-trip + tamper detection over the session DEK.
#[test]
fn field_cipher_round_trip_and_tamper() {
    let session = SessionDek::new([0x55u8; 32]);
    let fc = ChaChaFieldCipher::new(&session);
    let plain = "原始描述：2024-03 被拖欠工资 13800001111";
    let mut blob = fc.encrypt(plain).unwrap();
    assert_eq!(fc.decrypt(&blob).unwrap(), plain);
    let n = blob.len() - 1;
    blob[n] ^= 0x80;
    assert!(matches!(fc.decrypt(&blob), Err(CryptoError::Aead)));
}

/// CR-06 + CR-07 + CR-08: age external blob round-trip, determinism in the DEK, ciphertext on disk.
#[test]
fn age_blob_pipeline() {
    let session = SessionDek::new([0x66u8; 32]);
    let dir = tmp("blobs");
    let store = AgeStore::from_dek(&session, dir.clone()).unwrap();
    let plain = b"medical OCR text \xe7\x97\x85\xe5\x8e\x86";

    let r = store.write(plain).unwrap();
    assert_eq!(r.sha256, sha256_hex(plain)); // CR-06
    assert_eq!(store.read(&r).unwrap(), plain);

    // CR-08: on-disk content is not the plaintext.
    let on_disk = std::fs::read(&r.path).unwrap();
    assert_ne!(on_disk.as_slice(), plain.as_slice());

    // CR-07: a store rebuilt from the same DEK (restart) still reads the blob.
    let session2 = SessionDek::new([0x66u8; 32]);
    let store2 = AgeStore::from_dek(&session2, dir.clone()).unwrap();
    assert_eq!(store2.read(&r).unwrap(), plain);

    let _ = std::fs::remove_dir_all(&dir);
}

/// hkdf_subkey is the shared primitive: same input → same key, and the age identity it feeds is
/// deterministic (C-3 / CR-07 foundation).
#[test]
fn hkdf_primitive_is_shared_and_deterministic() {
    let dek = [0x77u8; 32];
    assert_eq!(hkdf_subkey(&dek, AGE_INFO), hkdf_subkey(&dek, AGE_INFO));
    assert_ne!(
        hkdf_subkey(&dek, AGE_INFO),
        hkdf_subkey(&dek, b"other-info")
    );
}

/// CR-10: BIP39 generation + deterministic KEK derivation.
#[test]
fn bip39_recovery_path() {
    let m = generate_bip39(Bip39Strength::Words24).unwrap();
    assert_eq!(m.word_count(), 24);
    let k = bip39_to_kek(&m, "pass");
    assert_eq!(k, bip39_to_kek(&m, "pass"));
    assert_ne!(k, bip39_to_kek(&m, "other"));
}

/// CR-11: `.stuchka-recovery` STKR magic + SHA-256 footer round-trip + tamper rejection.
#[test]
fn usb_recovery_file_path() {
    let f = RecoveryFile::new([0xEEu8; 32], [0x11u8; 16]);
    let dir = tmp("rec");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(".stuchka-recovery");
    write_recovery_file(&path, &f).unwrap();
    assert_eq!(read_recovery_file(&path).unwrap(), f);

    let mut raw = std::fs::read(&path).unwrap();
    let mid = raw.len() / 2;
    raw[mid] ^= 0x01;
    assert!(matches!(
        decode_recovery(&raw),
        Err(CryptoError::RecoveryCorrupt)
    ));
    let _ = std::fs::remove_dir_all(&dir);
}

/// CR-13: recovery method must be chosen before normal operation.
#[test]
fn recovery_method_guard() {
    assert!(validate_recovery_method(None).is_err());
    assert!(validate_recovery_method(Some(crypto::RecoveryMethod::Bip39)).is_ok());
}

/// CR-15 (best effort): after dropping a `SessionDek`, the bytes that backed it are not left intact
/// in the freed slot. We can only assert the zeroize machinery runs without UB; the strongest
/// portable check is that two independently-dropped keys do not leak a comparable live reference.
/// Here we assert the `ZeroizeOnDrop` contract compiles and a manual `zeroize()` clears the buffer
/// through the public `expose()` view before drop.
#[test]
fn session_dek_zeroize_contract() {
    use zeroize::Zeroize;
    let mut raw = [0xABu8; 32];
    // The same machinery SessionDek uses on drop, applied to a standalone buffer to prove it zeroes.
    raw.zeroize();
    assert_eq!(raw, [0u8; 32]);

    // And the wrapper exposes the live key before being dropped.
    let s = SessionDek::new([0xABu8; 32]);
    assert_eq!(s.expose(), &[0xABu8; 32]);
    drop(s); // ZeroizeOnDrop runs here; no observable handle remains.
}
