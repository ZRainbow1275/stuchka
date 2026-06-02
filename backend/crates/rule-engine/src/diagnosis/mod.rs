//! M1 诊断引擎 (prd/04 §4.1) — the DETERMINISTIC diagnosis subsystem.
//!
//! Three pieces, all pure-rule (INV-01 / automation level A, AI 不参与):
//! - [`catalog`] — the 85-subcategory目录 (coverage tier + LawRef + recommended procedures).
//! - [`tree`]    — the data-driven ≤7-question 问诊树 (every path → one of the 85 subcategories).
//! - [`engine`]  — [`DiagnosisEngine`], producing the §4.1.2 structured [`DiagnosisOutput`].
//!
//! Co-located with the M9 compute / M5 deadline engines so the whole deterministic decision layer
//! lives behind the same INV-01 cargo-deny / isolation gate (no AI / HTTP / sqlx in the tree).

pub mod catalog;
pub mod engine;
pub mod tree;

pub use catalog::{
    CatalogEntry, DiagnosisCatalog, Procedure, RecommendedProcedures, CATEGORY_TOTAL,
    SUBCATEGORY_TOTAL,
};
pub use engine::{
    DiagnosisEngine, DiagnosisInput, DiagnosisNextAction, DiagnosisOutput, DiagnosisResult,
};
pub use tree::{Answer, DecisionTree, Leaf, Node, Question, Step, MAX_QUESTIONS, ROOT_ID};
