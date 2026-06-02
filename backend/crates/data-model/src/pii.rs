//! High-sensitivity-detector (HSD) shared types (`ai/04`, `backend/04` §4.6).
//!
//! These value types are owned by `data-model` (the leaf crate) so they can be shared across
//! `crates/hsd` (detection logic), `crates/api` (`EvidenceDto.pii_hits`), and
//! `crates/ai-dispatcher` (export guard) without any of them depending on `hsd` directly. The
//! detection logic itself stays in `crates/hsd`.
//!
//! Naming follows `backend/04` §4.6 + the api DTO (`PiiHit` / `PiiKind` / `PiiLayer`), and the
//! [`Span`] uses explicit `{start, end}` fields (the cross-crate reconciliation B-4 ruling unifies
//! on this, abolishing `Range` / tuple). [`PiiHit`] is stored as JSON, so it needs serde but no
//! `sqlx::FromRow`.

use serde::{Deserialize, Serialize};

use crate::sensitivity::DataGrade;

/// Byte offsets `[start, end)` into the scanned text. Unified `{start, end}` form (B-4 ruling;
/// abolishes `Range` / tuple). Only ever stored inside [`PiiHit`] as JSON, so no `sqlx::Type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    /// Construct a span from inclusive-start / exclusive-end byte offsets.
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// Length of the span in bytes.
    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    /// Whether the span is empty (`start == end`).
    pub fn is_empty(&self) -> bool {
        self.start >= self.end
    }
}

/// Which detection layer produced a hit (`backend/04` §4.6.1 `PiiLayer`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
#[cfg_attr(
    feature = "sqlx",
    sqlx(type_name = "pii_layer", rename_all = "snake_case")
)]
pub enum PiiLayer {
    /// Layer 1 — regex strong-signal layer (R1a, 必死).
    Regex,
    /// Layer 2 — candle Chinese NER layer (R1b).
    Ner,
}

/// PII entity kind (`ai/04` §4.3/§4.4 + `backend/04` §4.6.1). The first five
/// (Phone / IdCard / BankCard / AudioPath / MedicalRecord) are strong signals that force local
/// routing on any single hit (`ai/04` §4.5 decision fusion).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
#[cfg_attr(
    feature = "sqlx",
    sqlx(type_name = "pii_kind", rename_all = "snake_case")
)]
pub enum PiiKind {
    Phone,
    IdCard,
    BankCard,
    AudioPath,
    MedicalRecord,
    Email,
    Address,
    PersonName,
    Organization,
    MedicalKeyword,
}

impl PiiKind {
    /// Whether this kind is a strong signal — any single hit forces local routing
    /// (`ai/04` §4.5: Phone / IdCard / BankCard / AudioPath / MedicalRecord).
    pub fn is_strong(&self) -> bool {
        matches!(
            self,
            PiiKind::Phone
                | PiiKind::IdCard
                | PiiKind::BankCard
                | PiiKind::AudioPath
                | PiiKind::MedicalRecord
        )
    }

    /// Map a PII kind to its data sensitivity grade (`compliance/02` §2). Strong identifiers and
    /// medical records are L4 (core privacy); phone / address / medical keywords are L3; the
    /// remaining person-identifying kinds are L2.
    pub fn data_grade(&self) -> DataGrade {
        match self {
            PiiKind::IdCard | PiiKind::BankCard | PiiKind::AudioPath | PiiKind::MedicalRecord => {
                DataGrade::L4
            }
            PiiKind::Phone | PiiKind::Address | PiiKind::MedicalKeyword => DataGrade::L3,
            PiiKind::Email | PiiKind::PersonName | PiiKind::Organization => DataGrade::L2,
        }
    }
}

/// A single PII detection hit (`backend/04` §4.6.1 `PiiHit`). Stored as JSON (no `sqlx::FromRow`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PiiHit {
    pub kind: PiiKind,
    pub span: Span,
    pub confidence: f32,
    pub layer: PiiLayer,
    /// Originating rule id (e.g. `PII-PHONE-CN` for regex, `NER-PER` for NER).
    pub rule_id: String,
}

/// Routing hint emitted by the HSD decision fusion (`ai/04` §4.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
#[cfg_attr(
    feature = "sqlx",
    sqlx(type_name = "route_hint", rename_all = "snake_case")
)]
pub enum RouteHint {
    /// Strong hit / composite score ≥ 1.5 → force local small steel cannon.
    ForceLocal,
    /// Weak signal (score ≥ 0.5) → extra UI confirmation but cloud allowed.
    WarnAndConfirm,
    /// No meaningful signal → route automatically.
    Auto,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn span_len_and_empty() {
        let s = Span::new(9, 20);
        assert_eq!(s.len(), 11);
        assert!(!s.is_empty());
        assert!(Span::new(5, 5).is_empty());
    }

    #[test]
    fn strong_signals_are_first_five() {
        for k in [
            PiiKind::Phone,
            PiiKind::IdCard,
            PiiKind::BankCard,
            PiiKind::AudioPath,
            PiiKind::MedicalRecord,
        ] {
            assert!(k.is_strong(), "{k:?} must be a strong signal");
        }
        for k in [
            PiiKind::Email,
            PiiKind::Address,
            PiiKind::PersonName,
            PiiKind::Organization,
            PiiKind::MedicalKeyword,
        ] {
            assert!(!k.is_strong(), "{k:?} must not be a strong signal");
        }
    }

    #[test]
    fn data_grade_mapping() {
        assert_eq!(PiiKind::IdCard.data_grade(), DataGrade::L4);
        assert_eq!(PiiKind::MedicalRecord.data_grade(), DataGrade::L4);
        assert_eq!(PiiKind::Phone.data_grade(), DataGrade::L3);
        assert_eq!(PiiKind::Address.data_grade(), DataGrade::L3);
        assert_eq!(PiiKind::PersonName.data_grade(), DataGrade::L2);
        assert_eq!(PiiKind::Email.data_grade(), DataGrade::L2);
    }

    #[test]
    fn pii_kind_serializes_snake_case() {
        assert_eq!(
            serde_json::to_string(&PiiKind::IdCard).unwrap(),
            "\"id_card\""
        );
        assert_eq!(
            serde_json::to_string(&PiiKind::MedicalRecord).unwrap(),
            "\"medical_record\""
        );
        assert_eq!(
            serde_json::to_string(&PiiKind::AudioPath).unwrap(),
            "\"audio_path\""
        );
    }

    #[test]
    fn pii_layer_and_route_hint_snake_case() {
        assert_eq!(serde_json::to_string(&PiiLayer::Ner).unwrap(), "\"ner\"");
        assert_eq!(
            serde_json::to_string(&RouteHint::ForceLocal).unwrap(),
            "\"force_local\""
        );
        assert_eq!(
            serde_json::to_string(&RouteHint::WarnAndConfirm).unwrap(),
            "\"warn_and_confirm\""
        );
    }

    #[test]
    fn pii_hit_json_roundtrip_camel_case() {
        let hit = PiiHit {
            kind: PiiKind::Phone,
            span: Span::new(9, 20),
            confidence: 1.0,
            layer: PiiLayer::Regex,
            rule_id: "PII-PHONE-CN".to_string(),
        };
        let j = serde_json::to_string(&hit).unwrap();
        assert!(j.contains("\"ruleId\":\"PII-PHONE-CN\""), "got {j}");
        assert!(j.contains("\"kind\":\"phone\""), "got {j}");
        let back: PiiHit = serde_json::from_str(&j).unwrap();
        assert_eq!(back, hit);
    }
}
