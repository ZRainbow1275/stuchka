//! Repository layer (backend/02 §2.8 / data/01 §1.8).
//!
//! Each repo wraps the SQLite pool and performs hand-coded CRUD against the fallback-mapped
//! columns (see `crate::codec`). State-machine guards call `data-model::validate_*_transition`
//! (the in-process mirror of the SQL trigger whitelists) so illegal moves return
//! `E_INVALID_TRANSITION`; INV-04 (`kb_version_hash` freeze) is enforced in `CaseRepo::update`.
//!
//! Foreign-key RESTRICT / CASCADE is enforced by SQLite itself because the pool sets
//! `PRAGMA foreign_keys = ON` on every connection (`crate::pool`).

pub mod case;
pub mod claim;
pub mod evidence;
pub mod fact;
pub mod group;
pub mod law_ref;
pub mod user_setting;

pub use case::CaseRepo;
pub use claim::ClaimRepo;
pub use evidence::EvidenceRepo;
pub use fact::FactRepo;
pub use group::GroupRepo;
pub use law_ref::LawRefRepo;
pub use user_setting::UserSettingRepo;
