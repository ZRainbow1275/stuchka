//! `.stuchka-patch` binary codec (03-sync-yjs §3.7.1). The on-disk layout is **frozen in R1a** so
//! R1b can ship import/export UX without a format migration:
//!
//! ```text
//! MAGIC "STKP" (4) | VERSION u16 BE | header_len u32 BE | header_json (utf-8)
//!                  | payload_len u32 BE | payload (Y.UpdateV2) | footer SHA-256(header||payload) (32)
//! ```
//!
//! I-4 ruling: `PatchHeader.case_id` carries the **bare UUID v7** (matching `case.id`), not the old
//! `case_...` short-code. The Y.Doc *guid* keeps the `case:<uuid>` namespace (that is a doc handle,
//! not a primary key). Time is `chrono` (W2): `issued_at` is RFC3339.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::str::FromStr;

use crate::auth::AuthLink;
use crate::error::{SyncError, SyncResult};
use crate::yjs::CaseDoc;

/// File magic.
pub const MAGIC: &[u8; 4] = b"STKP";
/// File format version (frozen).
pub const VERSION: u16 = 1;

/// Patch scope (03-sync-yjs §3.7.2). Determines which subset of the case Y.Doc the payload carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatchScope {
    /// Only fact entries.
    FactsOnly,
    /// Facts plus evidence metadata.
    FactsAndEvidences,
    /// The whole case document.
    Full,
}

impl FromStr for PatchScope {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "facts_only" => Ok(PatchScope::FactsOnly),
            "facts_and_evidences" => Ok(PatchScope::FactsAndEvidences),
            "full" => Ok(PatchScope::Full),
            _ => Err(()),
        }
    }
}

/// The signed, plaintext header of a `.stuchka-patch` file (§3.7.1). All reserved data keys are
/// present in R1a even though deeper import flows land in R1b.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchHeader {
    /// Bare UUID v7 of the target case (I-4: no `case_` prefix).
    pub case_id: String,
    /// Contributor peer id / UUID v7 (string form, §2.5).
    pub contributor_id: String,
    /// M15 authorization chain.
    pub authorization_chain: Vec<AuthLink>,
    /// Source KB version hash (INV-04 provenance).
    pub from_kb_version: String,
    /// Patch scope.
    pub scope: PatchScope,
    /// RFC3339 issue time (chrono, W2).
    pub issued_at: String,
    /// ed25519 signature (base64) over `signed_fields`.
    pub signature: String,
    /// Documents which fields the signature covers (`case_id|contributor_id|payload_sha256`).
    pub signed_fields: String,
}

/// SHA-256 of `header_json || payload`, the footer integrity check.
fn footer_hash(header_json: &[u8], payload: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(header_json);
    h.update(payload);
    h.finalize().into()
}

/// Encode a patch from a case document's full update and a signed header (§3.7.2).
///
/// R1a encodes the full update for every scope (the subset projection by scope is an R1b
/// refinement); the `scope` is recorded in the header so the importer knows the contract.
pub fn encode(
    case_doc: &CaseDoc,
    scope: PatchScope,
    mut header: PatchHeader,
) -> SyncResult<Vec<u8>> {
    header.scope = scope;
    let payload = case_doc.encode_full();
    encode_with_payload(&header, &payload)
}

/// Encode a patch from an already-built header + payload (used by export + tests).
pub fn encode_with_payload(header: &PatchHeader, payload: &[u8]) -> SyncResult<Vec<u8>> {
    let header_json = serde_json::to_vec(header)?;
    let footer = footer_hash(&header_json, payload);

    let mut out = Vec::with_capacity(4 + 2 + 4 + header_json.len() + 4 + payload.len() + 32);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&VERSION.to_be_bytes());
    out.extend_from_slice(&(header_json.len() as u32).to_be_bytes());
    out.extend_from_slice(&header_json);
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(payload);
    out.extend_from_slice(&footer);
    Ok(out)
}

/// Decode a patch: validate magic / version / lengths / footer SHA-256 (§3.7.3 step 1).
/// Any structural failure -> `E_PATCH_INVALID`.
pub fn decode(bytes: &[u8]) -> SyncResult<(PatchHeader, Vec<u8>)> {
    let mut cursor = 0usize;

    let take = |cursor: &mut usize, n: usize| -> SyncResult<&[u8]> {
        if *cursor + n > bytes.len() {
            return Err(SyncError::PatchInvalid("truncated".into()));
        }
        let slice = &bytes[*cursor..*cursor + n];
        *cursor += n;
        Ok(slice)
    };

    if take(&mut cursor, 4)? != MAGIC {
        return Err(SyncError::PatchInvalid("bad magic".into()));
    }
    let version = u16::from_be_bytes(take(&mut cursor, 2)?.try_into().unwrap());
    if version != VERSION {
        return Err(SyncError::PatchInvalid(format!(
            "unsupported version {version}"
        )));
    }
    let header_len = u32::from_be_bytes(take(&mut cursor, 4)?.try_into().unwrap()) as usize;
    let header_json = take(&mut cursor, header_len)?.to_vec();
    let payload_len = u32::from_be_bytes(take(&mut cursor, 4)?.try_into().unwrap()) as usize;
    let payload = take(&mut cursor, payload_len)?.to_vec();
    let footer = take(&mut cursor, 32)?;

    if cursor != bytes.len() {
        return Err(SyncError::PatchInvalid("trailing bytes".into()));
    }
    let expected = footer_hash(&header_json, &payload);
    if footer != expected {
        return Err(SyncError::PatchInvalid("footer hash mismatch".into()));
    }

    let header: PatchHeader = serde_json::from_slice(&header_json)
        .map_err(|e| SyncError::PatchInvalid(format!("header json: {e}")))?;
    Ok((header, payload))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn sample_header() -> PatchHeader {
        PatchHeader {
            case_id: "0190b2aa-0000-7000-8000-000000000001".into(),
            contributor_id: "0190b2aa-0000-7000-8000-000000000002".into(),
            authorization_chain: vec![],
            from_kb_version: "kbhash".into(),
            scope: PatchScope::Full,
            issued_at: Utc::now().to_rfc3339(),
            signature: "sig".into(),
            signed_fields: "case_id|contributor_id|payload_sha256".into(),
        }
    }

    #[test]
    fn scope_serde_is_snake_case() {
        let j = serde_json::to_string(&PatchScope::FactsAndEvidences).unwrap();
        assert_eq!(j, "\"facts_and_evidences\"");
        assert_eq!("full".parse::<PatchScope>(), Ok(PatchScope::Full));
    }

    #[test]
    fn round_trip_preserves_header_and_payload() {
        // SY-07: encode then decode restores header + payload.
        let doc = CaseDoc::new("0190b2aa-0000-7000-8000-000000000001");
        let bytes = encode(&doc, PatchScope::Full, sample_header()).unwrap();
        let (header, payload) = decode(&bytes).unwrap();
        assert_eq!(header.case_id, sample_header().case_id);
        assert_eq!(header.scope, PatchScope::Full);
        assert_eq!(payload, doc.encode_full());
    }

    #[test]
    fn tampered_footer_byte_is_rejected() {
        // SY-07: flip one footer byte -> E_PATCH_INVALID.
        let doc = CaseDoc::new("c1");
        let mut bytes = encode(&doc, PatchScope::Full, sample_header()).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 0xff;
        let err = decode(&bytes).unwrap_err();
        assert!(matches!(err, SyncError::PatchInvalid(_)));
        assert_eq!(err.error_code(), data_model::ErrorCode::PatchInvalid);
    }

    #[test]
    fn bad_magic_is_rejected() {
        let bad = b"XXXX\x00\x01".to_vec();
        assert!(matches!(decode(&bad), Err(SyncError::PatchInvalid(_))));
    }
}
