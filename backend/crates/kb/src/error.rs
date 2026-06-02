//! Unified KB error type (maps to backend/01 §1.12 error codes).
//!
//! The error codes referenced in the doc comments below are the on-the-wire `code` values the
//! api layer attaches when it surfaces a [`KbError`] over the D1 Bearer-HTTP contract.

use std::path::PathBuf;

/// Errors raised by the knowledge-base fetch / version / search subsystem.
#[derive(Debug, thiserror::Error)]
pub enum KbError {
    /// KB age >= 30 days; compensation calculation (M9) must abort (data/03 §3.5).
    /// → `E_KB_OUTDATED` (422).
    #[error("kb outdated: age {age_days}d >= 30d, calculation refused (Level 4)")]
    Outdated { age_days: i64 },

    /// Every distribution endpoint failed (GitHub Pages → jsDelivr → mirror → local cache).
    #[error("all kb endpoints failed for path: {0}")]
    AllEndpointsFailed(String),

    /// GPG signature verification failed for a fetched package (deploy/04 §4.4.3).
    #[error("gpg verify failed for {0}")]
    GpgVerifyFailed(String),

    /// A file's SHA-256 did not match its `checksums.txt` entry (KBC-04).
    #[error("sha256 mismatch for {path}: expected {expected}, got {actual}")]
    ChecksumMismatch {
        path: String,
        expected: String,
        actual: String,
    },

    /// The recomputed aggregate `global_hash` did not match the manifest (KBC-02 / INV-04).
    #[error("global_hash mismatch: manifest {expected}, recomputed {actual}")]
    GlobalHashMismatch { expected: String, actual: String },

    /// `categories.yaml` did not contain exactly 20 categories / 85 subcategories (KBC-03).
    #[error("category count invalid: got {categories} categories / {subcategories} subcategories (want 20/85)")]
    CategoryCountInvalid {
        categories: usize,
        subcategories: usize,
    },

    /// A `dispute_category` code did not match `^LD-[0-9]{2}-[0-9]{2}$` (data/01 §1.7).
    #[error("invalid dispute_category code: {0}")]
    InvalidCategoryCode(String),

    /// An underlying LawRef URN failed to parse (delegated to data-model, data/02 §2.4).
    #[error("law-ref parse failed: {0}")]
    LawRefParse(#[from] data_model::LawRefError),

    /// YAML deserialisation of a KB static file failed.
    #[error("yaml parse error: {0}")]
    Yaml(String),

    /// JSON (de)serialisation of a manifest failed.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    /// A git operation (clone / fetch / checkout / diff) failed (data/03 §3.2.1).
    #[error("git operation failed: {0}")]
    Git(String),

    /// The search index could not be built or queried (tantivy).
    #[error("search index error: {0}")]
    Search(String),

    /// A local filesystem operation failed.
    #[error("io error at {path:?}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

impl KbError {
    /// The backend/01 §1.12 wire error code this error maps to (for the api layer).
    pub fn code(&self) -> &'static str {
        match self {
            KbError::Outdated { .. } => "E_KB_OUTDATED",
            KbError::AllEndpointsFailed(_) => "E_KB_FETCH_FAILED",
            KbError::GpgVerifyFailed(_) => "E_KB_SIGNATURE_INVALID",
            KbError::ChecksumMismatch { .. } | KbError::GlobalHashMismatch { .. } => {
                "E_KB_CHECKSUM_MISMATCH"
            }
            KbError::CategoryCountInvalid { .. } | KbError::InvalidCategoryCode(_) => {
                "E_KB_CATEGORY_INVALID"
            }
            KbError::LawRefParse(_) => "E_LAW_REF_PARSE",
            KbError::Yaml(_) | KbError::Json(_) => "E_KB_PARSE",
            KbError::Git(_) => "E_KB_GIT",
            KbError::Search(_) => "E_KB_SEARCH",
            KbError::Io { .. } => "E_KB_IO",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outdated_maps_to_e_kb_outdated() {
        let e = KbError::Outdated { age_days: 31 };
        assert_eq!(e.code(), "E_KB_OUTDATED");
        assert!(e.to_string().contains("31d"));
    }

    #[test]
    fn law_ref_parse_error_converts_from_data_model() {
        let pe = data_model::LawRefError::MissingPrefix;
        let e: KbError = pe.into();
        assert_eq!(e.code(), "E_LAW_REF_PARSE");
    }
}
