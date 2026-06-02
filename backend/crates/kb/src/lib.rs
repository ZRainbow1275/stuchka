//! `kb` — knowledge base fetch + version hash + git diff (INV-04) + hybrid search.
//!
//! 0529 spec crate (master-index §0.4 / D6). This crate is the client-side KB subsystem:
//!
//! - [`freshness`] — Level-4 expiry gate (`freshness` / `ensure_calculable`), the M9 abort hook
//!   (data/03 §3.5, KB-01/02).
//! - [`manifest`] — `KbManifest` + the deterministic `global_hash` aggregate that anchors the
//!   INV-04 case freeze (data/03 §3.1, KBC-02).
//! - [`categories`] — `categories.yaml` 20-category / 85-subcategory parse + count assertion
//!   (data/01 §1.7, KBC-03), parsed with `serde_norway`.
//! - [`config`] — the four KB check-frequency strategies + sync-state snapshot (data/03 §3.3).
//! - [`version`] — `kb_version` row model + `is_active` singleton activation (data/03 §3.4).
//! - [`impact`] — impact-notice generation + persistence for non-frozen cases (data/03 §3.7).
//! - [`fetch`] — endpoint fallback resolution (GitHub Pages → jsDelivr → mirror → local cache).
//! - [`git_pull`] — git pull diff skeleton (gix) + the REAL checksums/global_hash integrity gates
//!   (KBC-02/04).
//! - [`gpg`] — the REAL KB-package detached-signature verify path (deploy/04 §4.4.3): shells out
//!   to the system `gpg` against the pre-deployed `stuchka-kb-pubkey.asc` keyring, returning
//!   `E_KB_SIGNATURE_INVALID` on a bad signature (the gate that must fire 100%).
//! - [`search`] — BM25 single-path hybrid search with Chinese segmentation (tantivy + jieba),
//!   reserving the vector leg behind the [`search::LawSearch`] trait (backend/01 §1.7.2).
//! - [`sample_data`] — 22 real PRC labour-law clauses across the five R1 deep categories, seeding
//!   the BM25 index so search genuinely hits.
//!
//! LawRef URN parsing + content-addressing hash live in `data-model` (D8); this crate re-exports
//! them via [`law_ref`] rather than re-implementing them.

pub mod categories;
pub mod config;
pub mod error;
pub mod fetch;
pub mod freshness;
pub mod git_pull;
pub mod gpg;
pub mod impact;
pub mod manifest;
pub mod sample_data;
pub mod search;
pub mod version;

/// Re-export of the data-model D8 LawRef parser / content hash (data/02). `kb` consumes these;
/// it never re-implements the URN grammar or the freeze hash (single source of truth, D8).
pub mod law_ref {
    pub use data_model::{
        content_hash, law_ref_to_stable_id as to_stable_id, parse_law_ref as parse, LawRef,
        LawRefError, LawRefParts,
    };
}

pub use categories::{Category, CategoryCatalog, Subcategory};
pub use config::{KbCheckFrequency, KbSyncConfig};
pub use error::KbError;
pub use fetch::{EndpointSet, FetchCandidate, DEFAULT_ENDPOINTS};
pub use freshness::{age_days, ensure_calculable, freshness, FreshnessLevel};
pub use git_pull::{
    diff_to_tag, parse_checksums, verify_checkout, verify_checksums, verify_manifest_global_hash,
    DiffSummary,
};
pub use gpg::{GpgVerifier, KbGpgError};
pub use impact::{changed_law_refs, ImpactNotice};
pub use manifest::{
    compute_global_hash, file_content_hash, KbManifest, CATEGORY_TOTAL, SUBCATEGORY_TOTAL,
};
pub use search::{Bm25Index, LawRefHit, LawSearch, SearchQuery};
pub use version::{activate, active_singleton_holds, KbVersion};

/// Crate identity for boot diagnostics and CI dependency-graph assertions.
pub const CRATE_NAME: &str = "kb";

#[cfg(test)]
mod tests {
    #[test]
    fn crate_name_is_stable() {
        assert_eq!(super::CRATE_NAME, "kb");
    }

    /// The crate re-exports the data-model D8 LawRef parser rather than re-defining it.
    #[test]
    fn reexports_data_model_law_ref_parser() {
        let parts =
            super::law_ref::parse("law:中华人民共和国劳动合同法/v2012-12-28/§39/¶1").unwrap();
        assert_eq!(parts.article, 39);
        assert_eq!(
            super::law_ref::to_stable_id(&parts),
            "law:中华人民共和国劳动合同法/v2012-12-28/§39/¶1"
        );
    }
}
