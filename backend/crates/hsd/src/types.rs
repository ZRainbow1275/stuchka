//! HSD output types (`ai/04` §2.2). The shared value types (`PiiHit` / `PiiKind` / `PiiLayer` /
//! `Span` / `RouteHint`) are owned by `crates/data-model` (B-4 reconciliation: types live in
//! data-model, detection logic lives here) and re-exported so downstream `api`
//! (`EvidenceDto.pii_hits`) consumes the same definitions with zero conversion (A22).

// Re-export the shared types so callers can `use hsd::{PiiHit, PiiKind, ...}`.
pub use data_model::{DataGrade, PiiHit, PiiKind, PiiLayer, RouteHint, Span};

use serde::{Deserialize, Serialize};

/// Result of a single `HsdDetector::scan(&str)` (`ai/04` §4.5 `HsdReport`, renamed `ScanReport`
/// per backend/04 §4.6 + the api DTO so `is_high_sensitive` aligns one-to-one).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanReport {
    /// All PII hits, byte spans mapped back to the *original* text (not the normalized text).
    pub hits: Vec<PiiHit>,
    /// Number of hits (convenience; equals `hits.len()`).
    pub hit_count: usize,
    /// `true` iff any strong-signal kind hit — drives `422 E_PII_BLOCKED` in api `/llm/query`.
    pub is_high_sensitive: bool,
    /// Routing hint from decision fusion (`ai/04` §4.5).
    pub route_hint: RouteHint,
    /// Weak-signal composite score (sum of hit weights).
    pub composite_score: f32,
    /// When the scan ran (UTC; chrono per workspace W2).
    pub scanned_at: chrono::DateTime<chrono::Utc>,
}

impl ScanReport {
    /// An empty report (no hits) stamped at the current instant.
    pub fn empty() -> Self {
        Self {
            hits: Vec::new(),
            hit_count: 0,
            is_high_sensitive: false,
            route_hint: RouteHint::Auto,
            composite_score: 0.0,
            scanned_at: chrono::Utc::now(),
        }
    }
}
