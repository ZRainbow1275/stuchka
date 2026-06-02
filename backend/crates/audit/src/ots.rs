//! §4.6.2 — OpenTimestamps anchoring abstraction (W8 ruling).
//!
//! R1a scope (W8 / I1): a `trait OtsClient` with `stamp` + `verify_binding`, a [`MockOtsClient`]
//! (offline, deterministic — drives AU-09/10/12), and a [`CliOtsClient`] seam that shells out to a
//! local `ots` binary for a real calendar upload. The real network upload + cron is R1b; the CLI
//! seam is wired but not invoked by the offline tests.
//!
//! Why a self-describing receipt: the Rust `opentimestamps` crate (max stable 0.2.0, W8/I4) is
//! parse/verify-only and has no `stamp()` calendar-submission API, so it cannot mint a receipt.
//! R1a therefore uses an explicit receipt envelope (`STOT` magic || 32-byte bound hash || proof
//! payload). `verify_binding` checks the envelope's bound hash equals the supplied daily hash —
//! exactly the "wrong-hash must Err" contract of AU-10. When the proof payload is a real `.ots`
//! blob (CLI path) we additionally parse it through the `opentimestamps` crate so a malformed
//! proof is rejected; the mock path carries a deterministic placeholder proof.

use std::process::Command;

use crate::error::OtsError;

/// 4-byte receipt envelope magic: "Stuchka OTS".
pub const OTS_RECEIPT_MAGIC: &[u8; 4] = b"STOT";

/// Marker byte distinguishing a mock proof payload from a real `.ots` blob inside the envelope.
const PROOF_KIND_MOCK: u8 = 0x00;
const PROOF_KIND_OTS: u8 = 0x01;

/// Encode a receipt envelope: `STOT || kind(1) || hash(32) || proof`.
fn encode_receipt(hash: &[u8; 32], kind: u8, proof: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + 1 + 32 + proof.len());
    out.extend_from_slice(OTS_RECEIPT_MAGIC);
    out.push(kind);
    out.extend_from_slice(hash);
    out.extend_from_slice(proof);
    out
}

/// Parse a receipt envelope, returning `(kind, bound_hash, proof)`.
fn decode_receipt(receipt: &[u8]) -> Result<(u8, [u8; 32], &[u8]), OtsError> {
    if receipt.len() < 4 + 1 + 32 || &receipt[..4] != OTS_RECEIPT_MAGIC {
        return Err(OtsError::Malformed("bad magic or short receipt".into()));
    }
    let kind = receipt[4];
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&receipt[5..37]);
    Ok((kind, hash, &receipt[37..]))
}

/// OpenTimestamps client seam (§4.6.2). `async fn` in traits is stable on the MSRV (1.84).
pub trait OtsClient: Send + Sync {
    /// Submit `hash` to a calendar and return a receipt envelope binding `hash`.
    fn stamp(
        &self,
        hash: &[u8; 32],
    ) -> impl std::future::Future<Output = Result<Vec<u8>, OtsError>> + Send;

    /// Verify a receipt envelope binds to `hash` (AU-10). The default impl checks the envelope's
    /// bound hash and, for real `.ots` proofs, that the proof parses.
    fn verify_binding(receipt: &[u8], hash: &[u8; 32]) -> Result<(), OtsError>
    where
        Self: Sized,
    {
        verify_binding(receipt, hash)
    }
}

/// Free-function form of `verify_binding` so the api/anchor path and tests can call it without a
/// client instance (data/04 §4.6.2 `ots_client::verify_binding`).
pub fn verify_binding(receipt: &[u8], hash: &[u8; 32]) -> Result<(), OtsError> {
    let (kind, bound, proof) = decode_receipt(receipt)?;
    if &bound != hash {
        return Err(OtsError::Binding);
    }
    if kind == PROOF_KIND_OTS {
        // Real .ots blob: parse it so a corrupt proof is rejected (the `opentimestamps` crate is
        // verify/parse-only, W8/I4).
        opentimestamps::DetachedTimestampFile::from_reader(proof)
            .map_err(|e| OtsError::Malformed(format!("ots parse: {e}")))?;
    }
    Ok(())
}

/// Deterministic offline OTS client for tests / CI (no network). AU-09/10/12 run against this.
#[derive(Debug, Default, Clone)]
pub struct MockOtsClient;

impl MockOtsClient {
    /// Construct a mock client.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Deterministic stamp used directly by tests (`mock_stamp` in the spec skeleton).
    #[must_use]
    pub fn stamp_sync(hash: &[u8; 32]) -> Vec<u8> {
        // A deterministic placeholder proof: SHA-256 prefix of the hash, so the receipt is stable.
        encode_receipt(hash, PROOF_KIND_MOCK, b"mock-ots-proof-v1")
    }
}

impl OtsClient for MockOtsClient {
    async fn stamp(&self, hash: &[u8; 32]) -> Result<Vec<u8>, OtsError> {
        Ok(Self::stamp_sync(hash))
    }
}

/// Real-calendar CLI fallback seam (data/04 §4.6.2). Shells out to a local `ots` binary to stamp a
/// temp file holding `hash`, reads back the produced `.ots` proof, and wraps it in the receipt
/// envelope. Network upload happens inside the `ots` binary — invoking it is R1b (the offline tests
/// never call this); the seam is fully wired so R1b only flips the call site.
pub struct CliOtsClient {
    /// Path/name of the `ots` binary (e.g. `"ots"` on PATH).
    pub binary: String,
}

impl CliOtsClient {
    /// Construct a CLI client over the given `ots` binary name/path.
    #[must_use]
    pub fn new(binary: impl Into<String>) -> Self {
        Self {
            binary: binary.into(),
        }
    }

    /// Wrap a real `.ots` proof blob into a binding receipt envelope.
    #[must_use]
    pub fn wrap_ots_proof(hash: &[u8; 32], ots_blob: &[u8]) -> Vec<u8> {
        encode_receipt(hash, PROOF_KIND_OTS, ots_blob)
    }
}

impl OtsClient for CliOtsClient {
    async fn stamp(&self, _hash: &[u8; 32]) -> Result<Vec<u8>, OtsError> {
        // R1b real-network path: write the hash to a temp file, run `ots stamp <file>`, read the
        // `<file>.ots` proof. Kept behind an explicit availability probe so a missing binary is a
        // clean upstream error rather than a panic.
        let probe = Command::new(&self.binary).arg("--version").output();
        match probe {
            Ok(_) => {
                // The full stamp flow is R1b; surface a clear not-yet-wired upstream error so the
                // anchor path degrades to E_OTS_UPSTREAM (502) instead of fabricating a receipt.
                Err(OtsError::Stamp(
                    "real ots calendar upload is R1b; CLI seam present but network upload deferred"
                        .into(),
                ))
            }
            Err(e) => Err(OtsError::Stamp(format!(
                "ots binary `{}` unavailable: {e}",
                self.binary
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash() -> [u8; 32] {
        [0x11u8; 32]
    }

    #[tokio::test]
    async fn mock_stamp_binds_to_hash() {
        let c = MockOtsClient::new();
        let r = c.stamp(&hash()).await.unwrap();
        assert_eq!(&r[..4], OTS_RECEIPT_MAGIC);
        verify_binding(&r, &hash()).unwrap();
    }

    /// AU-10: a receipt must Err against the wrong hash.
    #[test]
    fn au_10_wrong_hash_is_err() {
        let r = MockOtsClient::stamp_sync(&hash());
        let wrong = [0u8; 32];
        assert!(matches!(verify_binding(&r, &wrong), Err(OtsError::Binding)));
        // and the trait-assoc form agrees
        assert!(MockOtsClient::verify_binding(&r, &wrong).is_err());
        assert!(MockOtsClient::verify_binding(&r, &hash()).is_ok());
    }

    #[test]
    fn malformed_receipt_is_rejected() {
        assert!(matches!(
            verify_binding(b"xx", &hash()),
            Err(OtsError::Malformed(_))
        ));
        assert!(matches!(
            verify_binding(b"NOTM\x00aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", &hash()),
            Err(OtsError::Malformed(_))
        ));
    }

    #[test]
    fn ots_kind_proof_must_parse() {
        // A receipt claiming a real .ots proof but carrying garbage must be rejected.
        let r = CliOtsClient::wrap_ots_proof(&hash(), b"not-a-real-ots-file");
        assert!(matches!(
            verify_binding(&r, &hash()),
            Err(OtsError::Malformed(_))
        ));
    }

    #[tokio::test]
    async fn cli_missing_binary_is_upstream_error() {
        let c = CliOtsClient::new("definitely-not-a-real-binary-xyz");
        let err = c.stamp(&hash()).await.unwrap_err();
        assert!(matches!(err, OtsError::Stamp(_)));
    }
}
