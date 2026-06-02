//! Evidence five-dimension scoring (M2 · backend/01 §1.5.1, INV-01 / INV-03).
//!
//! A **pure, deterministic** scorer (zero AI / zero HTTP, like the rest of `rule-engine`): it maps
//! observable evidence attributes onto the five legal-weight dimensions
//! `source / temporal / integrity / relevance / authenticity` (each in `0.0..=1.0`) and combines
//! them with statute-aligned weights into a single `effective_score`. The api layer
//! (`crates/api`) feeds in the attributes it collected during upload (category, integrity check,
//! whether collection time / device / GPS metadata are present, how many facts the evidence links
//! to) and persists the resulting [`ScoreBreakdown`] + `effective_score`.
//!
//! This is the legal-credibility heuristic of 证据三性 (真实性 / 关联性 / 合法性) extended to the
//! five operational dimensions the product tracks; it is intentionally explainable and stable, not
//! a learned model.

use serde::{Deserialize, Serialize};

use data_model::EvidenceCategory;

/// Observable inputs the scorer consumes (collected by the api upload handler). Everything here is
/// derivable without AI: the evidence class, whether the file passed an integrity hash, and which
/// provenance-metadata fields the uploader supplied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceScoreInputs {
    /// Evidence class (drives the `source` + baseline `authenticity` weights).
    pub category: EvidenceCategory,
    /// File SHA-256 was computed and stored (integrity anchor present).
    pub integrity_hash_present: bool,
    /// A collection timestamp (`collected_at`) was supplied.
    pub has_collected_at: bool,
    /// A device id was supplied (chain-of-custody signal).
    pub has_device_id: bool,
    /// GPS coordinates were supplied (corroborating provenance).
    pub has_gps: bool,
    /// Number of case facts this evidence is linked to (relevance signal).
    pub linked_fact_count: usize,
}

/// The five-dimension breakdown (backend/01 §1.5.1 `ScoreBreakdown`); each field is `0.0..=1.0`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoreBreakdown {
    /// 来源 — credibility of the evidence source/class.
    pub source: f32,
    /// 时间 — whether/when the evidence was time-anchored.
    pub temporal: f32,
    /// 完整性 — integrity (hash present, not tampered).
    pub integrity: f32,
    /// 关联性 — relevance to the case facts.
    pub relevance: f32,
    /// 真实性 — authenticity (class-baseline + corroborating metadata).
    pub authenticity: f32,
}

impl ScoreBreakdown {
    /// Statute-aligned dimension weights (sum = 1.0): authenticity and source carry the most legal
    /// weight (证据三性), relevance next, then integrity and temporal.
    const W_SOURCE: f32 = 0.25;
    const W_TEMPORAL: f32 = 0.15;
    const W_INTEGRITY: f32 = 0.15;
    const W_RELEVANCE: f32 = 0.20;
    const W_AUTHENTICITY: f32 = 0.25;

    /// The weighted `effective_score` in `0.0..=1.0`.
    pub fn effective_score(&self) -> f32 {
        let s = self.source * Self::W_SOURCE
            + self.temporal * Self::W_TEMPORAL
            + self.integrity * Self::W_INTEGRITY
            + self.relevance * Self::W_RELEVANCE
            + self.authenticity * Self::W_AUTHENTICITY;
        s.clamp(0.0, 1.0)
    }
}

/// Per-class `source` credibility weight (objective documentary / third-party records rank highest;
/// witness statements lowest). 证据法 admissibility ordering.
fn source_weight(category: EvidenceCategory) -> f32 {
    match category {
        EvidenceCategory::ThirdPartyData => 0.95, // 银行流水 / 税单 / 社保 — objective records
        EvidenceCategory::Appraisal => 0.9,       // 鉴定 / 评估
        EvidenceCategory::DocumentaryContract => 0.85, // 书证 / 合同
        EvidenceCategory::AudioVideo => 0.75,     // 录音 / 录像
        EvidenceCategory::DigitalCommunication => 0.7, // 聊天 / 通讯
        EvidenceCategory::ScenePhotoVideo => 0.65, // 现场照片 / 视频
        EvidenceCategory::WitnessStatement => 0.5, // 证人证言
    }
}

/// Per-class baseline `authenticity` weight before corroborating-metadata adjustments.
fn authenticity_baseline(category: EvidenceCategory) -> f32 {
    match category {
        EvidenceCategory::ThirdPartyData | EvidenceCategory::Appraisal => 0.85,
        EvidenceCategory::DocumentaryContract => 0.8,
        EvidenceCategory::AudioVideo | EvidenceCategory::DigitalCommunication => 0.65,
        EvidenceCategory::ScenePhotoVideo => 0.6,
        EvidenceCategory::WitnessStatement => 0.45,
    }
}

/// Compute the five-dimension breakdown from the observable inputs. Pure + deterministic.
pub fn score(inputs: &EvidenceScoreInputs) -> ScoreBreakdown {
    let source = source_weight(inputs.category);

    // Temporal: a supplied collection timestamp is the anchor; a device id corroborates it.
    let temporal = match (inputs.has_collected_at, inputs.has_device_id) {
        (true, true) => 0.95,
        (true, false) => 0.8,
        (false, true) => 0.5,
        (false, false) => 0.3,
    };

    // Integrity: a stored SHA-256 means the artefact is tamper-evident.
    let integrity = if inputs.integrity_hash_present {
        1.0
    } else {
        0.4
    };

    // Relevance: linked to at least one fact lifts relevance; more links saturate toward 1.0.
    let relevance = match inputs.linked_fact_count {
        0 => 0.4,
        1 => 0.7,
        2 => 0.85,
        _ => 0.95,
    };

    // Authenticity: class baseline, lifted by corroborating provenance metadata (device / GPS).
    let mut authenticity = authenticity_baseline(inputs.category);
    if inputs.has_device_id {
        authenticity += 0.05;
    }
    if inputs.has_gps {
        authenticity += 0.05;
    }
    let authenticity = authenticity.clamp(0.0, 1.0);

    ScoreBreakdown {
        source,
        temporal,
        integrity,
        relevance,
        authenticity,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base(category: EvidenceCategory) -> EvidenceScoreInputs {
        EvidenceScoreInputs {
            category,
            integrity_hash_present: true,
            has_collected_at: true,
            has_device_id: false,
            has_gps: false,
            linked_fact_count: 0,
        }
    }

    #[test]
    fn weights_sum_to_one() {
        let w = ScoreBreakdown::W_SOURCE
            + ScoreBreakdown::W_TEMPORAL
            + ScoreBreakdown::W_INTEGRITY
            + ScoreBreakdown::W_RELEVANCE
            + ScoreBreakdown::W_AUTHENTICITY;
        assert!((w - 1.0).abs() < 1e-6, "dimension weights must sum to 1.0");
    }

    #[test]
    fn effective_score_in_unit_range() {
        for cat in [
            EvidenceCategory::ThirdPartyData,
            EvidenceCategory::WitnessStatement,
            EvidenceCategory::DocumentaryContract,
        ] {
            let b = score(&base(cat));
            let e = b.effective_score();
            assert!(
                (0.0..=1.0).contains(&e),
                "score {e} out of range for {cat:?}"
            );
        }
    }

    #[test]
    fn third_party_outscores_witness() {
        let third = score(&base(EvidenceCategory::ThirdPartyData)).effective_score();
        let witness = score(&base(EvidenceCategory::WitnessStatement)).effective_score();
        assert!(
            third > witness,
            "objective third-party records must outscore witness statements: {third} vs {witness}"
        );
    }

    #[test]
    fn deterministic() {
        let inputs = base(EvidenceCategory::AudioVideo);
        assert_eq!(score(&inputs), score(&inputs));
    }

    #[test]
    fn integrity_and_links_lift_score() {
        let mut a = base(EvidenceCategory::DocumentaryContract);
        a.integrity_hash_present = false;
        a.linked_fact_count = 0;
        let low = score(&a).effective_score();

        let mut b = base(EvidenceCategory::DocumentaryContract);
        b.integrity_hash_present = true;
        b.linked_fact_count = 3;
        b.has_device_id = true;
        b.has_gps = true;
        let high = score(&b).effective_score();

        assert!(
            high > low,
            "integrity + links + provenance must raise the score"
        );
    }
}
