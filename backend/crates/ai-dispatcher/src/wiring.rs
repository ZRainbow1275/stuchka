//! Adapters wiring the real `hsd` / `kb` / `rule-engine` crates onto the [`crate::stage`] context
//! traits. These keep `stage::answer` decoupled (so it is mock-testable) while letting the live
//! `Dispatcher` drive the genuine subsystems.

use async_trait::async_trait;
use data_model::RouteHint;
use hsd::HsdDetector;
use kb::{Bm25Index, KbVersion, LawSearch, SearchQuery};
use rule_engine::{RuleEngine, RuleOutcome, RuleRequest};

use crate::error::DispatcherError;
use crate::routing::desensitize;
use crate::stage::{HsdContext, KbContext, RuleContext};

/// Adapts [`hsd::HsdDetector`] to [`HsdContext`].
pub struct HsdAdapter<'a> {
    /// The detector.
    pub detector: &'a HsdDetector,
}

impl HsdContext for HsdAdapter<'_> {
    fn scan(&self, text: &str) -> (RouteHint, bool) {
        let report = self.detector.scan(text);
        (report.route_hint, report.is_high_sensitive)
    }

    /// Real domestic-cloud desensitisation (`compliance/02` §3.4): runs the genuine
    /// [`crate::routing::desensitize`] pipeline over the SAME detector, so the export guard masks
    /// PII byte-exactly (`hsd::mask`) and re-scans for residuals on the LIVE path.
    fn desensitize(&self, text: &str) -> (String, bool) {
        let d = desensitize(self.detector, text);
        (d.masked_text, d.residual_pii)
    }
}

/// Adapts a [`kb::Bm25Index`] + active [`kb::KbVersion`] to [`KbContext`].
pub struct KbAdapter<'a> {
    /// The BM25 search index.
    pub index: &'a Bm25Index,
    /// The active KB version (its `generated_at` drives the age gate). `None` ⇒ age 0.
    pub version: Option<&'a KbVersion>,
    /// Whole-day age of the KB (computed by the caller from `version.generated_at`).
    pub age_days: u32,
}

#[async_trait]
impl KbContext for KbAdapter<'_> {
    async fn pull_incremental_if_due(&self) -> Result<(), DispatcherError> {
        // The real incremental pull (gix diff over GitHub Pages / jsDelivr) is owned by the kb
        // sync job; the in-process dispatcher does not trigger network pulls per request. No-op.
        Ok(())
    }

    fn age_days(&self) -> u32 {
        self.age_days
    }

    fn hybrid_query(&self, text: &str, top_k: usize) -> Vec<(String, f32)> {
        let q = SearchQuery {
            q: text.to_string(),
            limit: top_k as u32,
            ..Default::default()
        };
        match self.index.search(&q) {
            Ok(hits) => hits.into_iter().map(|h| (h.stable_id, h.score)).collect(),
            Err(_) => Vec::new(),
        }
    }

    fn version_label(&self) -> String {
        self.version
            .map(|v| v.version_label.clone())
            .unwrap_or_else(|| "本地知识库".to_string())
    }
}

/// Adapts [`rule_engine::RuleEngine`] to [`RuleContext`].
pub struct RuleAdapter<'a> {
    /// The pure rule engine.
    pub engine: &'a RuleEngine,
}

impl RuleContext for RuleAdapter<'_> {
    fn try_resolve(&self, req: &RuleRequest) -> Option<RuleOutcome> {
        self.engine.try_resolve(req)
    }
}
