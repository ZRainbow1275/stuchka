//! Decision fusion + routing (`ai/04` §4.5).
//!
//! Strong signal (phone / id card / bank card / audio path / medical-record path) → any single hit
//! sets `is_high_sensitive = true` and `route_hint = ForceLocal`. Weak signals (email / address /
//! person / org / medical keyword) accumulate a composite score:
//! - score ≥ 1.5 → ForceLocal
//! - 0.5 ≤ score < 1.5 → WarnAndConfirm
//! - score < 0.5 → Auto
//!
//! Per reconciliation INC-7, weak signals alone never set `is_high_sensitive` (which gates
//! `422 E_PII_BLOCKED`); they only influence `route_hint`.

use crate::types::{PiiHit, PiiKind, RouteHint, ScanReport};

/// Fuse regex-layer and NER-layer hits into a [`ScanReport`].
pub fn decide(regex_hits: Vec<PiiHit>, ner_hits: Vec<PiiHit>) -> ScanReport {
    let merged = merge_overlapping(regex_hits, ner_hits);

    let strong_hit = merged.iter().any(|h| h.kind.is_strong());
    let composite_score: f32 = merged.iter().map(|h| h.confidence).sum();

    let route_hint = if strong_hit || composite_score >= 1.5 {
        RouteHint::ForceLocal
    } else if composite_score >= 0.5 {
        RouteHint::WarnAndConfirm
    } else {
        RouteHint::Auto
    };

    ScanReport {
        hit_count: merged.len(),
        is_high_sensitive: strong_hit,
        route_hint,
        composite_score,
        hits: merged,
        scanned_at: chrono::Utc::now(),
    }
}

/// Merge two hit lists, dropping fully-contained duplicate spans of the same kind and sorting by
/// start offset. When two hits of the same kind overlap, the one with higher confidence wins; a
/// strong-signal hit always beats a weak-signal hit covering the same region.
fn merge_overlapping(a: Vec<PiiHit>, b: Vec<PiiHit>) -> Vec<PiiHit> {
    let mut all: Vec<PiiHit> = a;
    all.extend(b);
    // Stable order: by start, then by descending confidence so the stronger hit is kept first.
    all.sort_by(|x, y| {
        x.span
            .start
            .cmp(&y.span.start)
            .then(y.confidence.total_cmp(&x.confidence))
    });

    let mut out: Vec<PiiHit> = Vec::with_capacity(all.len());
    for hit in all {
        // Drop a hit whose span is contained in / identical to an already-kept same-kind hit.
        let dominated = out.iter().any(|kept| {
            same_or_related_kind(kept.kind, hit.kind)
                && kept.span.start <= hit.span.start
                && kept.span.end >= hit.span.end
        });
        if !dominated {
            out.push(hit);
        }
    }
    out
}

/// Whether two kinds should be deduplicated against each other (same kind, or address/location
/// emitted by both regex and NER).
fn same_or_related_kind(a: PiiKind, b: PiiKind) -> bool {
    a == b || matches!((a, b), (PiiKind::Address, PiiKind::Address))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{PiiLayer, Span};

    fn hit(kind: PiiKind, start: usize, end: usize, conf: f32) -> PiiHit {
        PiiHit {
            kind,
            span: Span::new(start, end),
            confidence: conf,
            layer: PiiLayer::Regex,
            rule_id: "TEST".to_string(),
        }
    }

    #[test]
    fn a9_strong_signal_forces_local() {
        let rep = decide(vec![hit(PiiKind::Phone, 0, 11, 1.0)], vec![]);
        assert!(rep.is_high_sensitive);
        assert_eq!(rep.route_hint, RouteHint::ForceLocal);
    }

    #[test]
    fn a10_weak_only_not_high_sensitive() {
        // Single email (0.6) → WarnAndConfirm, not high sensitive.
        let rep = decide(vec![hit(PiiKind::Email, 0, 10, 0.6)], vec![]);
        assert!(!rep.is_high_sensitive);
        assert_ne!(rep.route_hint, RouteHint::ForceLocal);
        assert_eq!(rep.route_hint, RouteHint::WarnAndConfirm);
    }

    #[test]
    fn weak_score_over_1_5_forces_local_without_high_sensitive() {
        // Email 0.6 + Address 0.7 + Address 0.7 = 2.0 ≥ 1.5 → ForceLocal but NOT high-sensitive.
        let rep = decide(
            vec![
                hit(PiiKind::Email, 0, 5, 0.6),
                hit(PiiKind::Address, 10, 20, 0.7),
                hit(PiiKind::Address, 30, 40, 0.7),
            ],
            vec![],
        );
        assert_eq!(rep.route_hint, RouteHint::ForceLocal);
        assert!(!rep.is_high_sensitive);
    }

    #[test]
    fn no_hits_is_auto() {
        let rep = decide(vec![], vec![]);
        assert_eq!(rep.route_hint, RouteHint::Auto);
        assert!(!rep.is_high_sensitive);
        assert_eq!(rep.hit_count, 0);
    }

    #[test]
    fn contained_duplicate_dropped() {
        let rep = decide(
            vec![
                hit(PiiKind::Phone, 0, 11, 1.0),
                hit(PiiKind::Phone, 0, 11, 0.9),
            ],
            vec![],
        );
        assert_eq!(rep.hit_count, 1);
    }
}
