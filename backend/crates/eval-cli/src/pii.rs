//! pii-200 gate (ai/05 §5.7 + ai/04 §4.7): 高敏检测 召回 >= 95% / 误报 <= 5%.
//!
//! Drives the REAL `hsd::HsdDetector::new_regex_only().scan(text)` over a labelled corpus. Every
//! positive embeds a GENUINE valid token (phone matching the production regex, ISO-7064 id card,
//! Luhn bank card, audio/medical path) so "recall" measures real detection; every negative is
//! benign or a checksum-failing near-miss the validators must reject, so "fp_rate" measures real
//! false positives. Ground truth is the embedded token's known kind — never fabricated.

use hsd::{HsdDetector, PiiKind, RouteHint};
use serde::Deserialize;

use crate::common::read_json;
use crate::report::{CaseResult, GateReport};

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Label {
    Positive,
    Negative,
}

#[derive(Debug, Deserialize)]
struct PiiCase {
    id: String,
    text: String,
    label: Label,
    /// Expected detected kinds, e.g. ["phone"], ["id_card"]. For negatives this is empty.
    #[serde(default)]
    expect_kinds: Vec<String>,
    #[serde(default)]
    expect_high_sensitive: Option<bool>,
    /// "force_local" | "warn_and_confirm" | "auto".
    #[serde(default)]
    expect_route_hint: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PiiDataset {
    cases: Vec<PiiCase>,
}

fn kind_str(k: PiiKind) -> &'static str {
    match k {
        PiiKind::Phone => "phone",
        PiiKind::IdCard => "id_card",
        PiiKind::BankCard => "bank_card",
        PiiKind::AudioPath => "audio_path",
        PiiKind::MedicalRecord => "medical_record",
        PiiKind::Email => "email",
        PiiKind::Address => "address",
        PiiKind::PersonName => "person_name",
        PiiKind::Organization => "organization",
        PiiKind::MedicalKeyword => "medical_keyword",
    }
}

fn route_str(r: RouteHint) -> &'static str {
    match r {
        RouteHint::ForceLocal => "force_local",
        RouteHint::WarnAndConfirm => "warn_and_confirm",
        RouteHint::Auto => "auto",
    }
}

pub fn run(dataset: &std::path::Path) -> Result<GateReport, String> {
    let data: PiiDataset = read_json(dataset)?;
    let detector = HsdDetector::new_regex_only();

    let mut cases = Vec::with_capacity(data.cases.len());
    let (mut positives, mut recalled, mut negatives, mut false_positives) = (0usize, 0usize, 0usize, 0usize);

    for case in &data.cases {
        let report = detector.scan(&case.text);
        let detected: Vec<&str> = report.hits.iter().map(|h| kind_str(h.kind)).collect();

        let kinds_ok = case
            .expect_kinds
            .iter()
            .all(|want| detected.iter().any(|d| d == want));
        let hs_ok = case
            .expect_high_sensitive
            .is_none_or(|b| report.is_high_sensitive == b);
        let route_ok = case
            .expect_route_hint
            .as_deref()
            .is_none_or(|r| route_str(report.route_hint) == r);

        let pass = match case.label {
            Label::Positive => {
                positives += 1;
                let ok = kinds_ok && hs_ok && route_ok;
                if ok {
                    recalled += 1;
                }
                ok
            }
            Label::Negative => {
                negatives += 1;
                // A negative is a false positive iff the detector raised a strong-signal hit.
                let triggered = report.is_high_sensitive;
                if triggered {
                    false_positives += 1;
                }
                // Pass = not triggered AND (any explicit route/hs/kind expectation holds).
                !triggered && hs_ok && route_ok && kinds_ok
            }
        };

        cases.push(CaseResult::new(
            &case.id,
            pass,
            format!(
                "{:?} kinds={:?} hs={:?} route={:?}",
                case.label, case.expect_kinds, case.expect_high_sensitive, case.expect_route_hint
            ),
            format!(
                "detected={:?} hs={} route={}",
                detected,
                report.is_high_sensitive,
                route_str(report.route_hint)
            ),
        ));
    }

    // Fail-safe: an empty positive corpus cannot demonstrate recall -> 0.0 (never a free pass);
    // an empty negative corpus cannot demonstrate a low false-positive rate -> 1.0.
    let recall = if positives == 0 {
        0.0
    } else {
        recalled as f64 / positives as f64
    };
    let fp_rate = if negatives == 0 {
        1.0
    } else {
        false_positives as f64 / negatives as f64
    };

    let report = GateReport::from_cases("pii", cases)
        .with_metric("positives", positives.into())
        .with_metric("recalled", recalled.into())
        .with_metric("recall", recall.into())
        .with_metric("negatives", negatives.into())
        .with_metric("false_positives", false_positives.into())
        .with_metric("fp_rate", fp_rate.into());
    Ok(report)
}
