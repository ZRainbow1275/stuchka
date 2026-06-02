//! Desensitization for UI display (`ai/04` §4.6, `compliance/02` §3.4).
//!
//! [`mask`] replaces the bytes covered by each hit span with a masked rendering:
//! - Phone: keep first 3 + last 4 digits → `138****8000`.
//! - IdCard: keep first 6 + last 4 → `110101********1234`.
//! - BankCard: keep last 4 → `************7894`.
//! - Other kinds: replace the whole span with `*` of equal display width.
//!
//! Spans are byte offsets into the ORIGINAL text. Hits are applied right-to-left so earlier
//! offsets stay valid while replacing.

use crate::types::{PiiHit, PiiKind};

/// Mask every hit span in `text`. Returns a new string with PII desensitized.
pub fn mask(text: &str, hits: &[PiiHit]) -> String {
    // Sort by start descending so replacing later spans first keeps earlier offsets valid.
    let mut ordered: Vec<&PiiHit> = hits.iter().collect();
    ordered.sort_by(|a, b| b.span.start.cmp(&a.span.start));

    let mut out = text.to_string();
    for hit in ordered {
        let start = hit.span.start;
        let end = hit.span.end.min(out.len());
        if start >= end || !out.is_char_boundary(start) || !out.is_char_boundary(end) {
            continue;
        }
        let original = &out[start..end];
        let masked = mask_value(hit.kind, original);
        out.replace_range(start..end, &masked);
    }
    out
}

/// Mask a single value according to its kind.
fn mask_value(kind: PiiKind, value: &str) -> String {
    match kind {
        PiiKind::Phone => mask_keep_ends(value, 3, 4),
        PiiKind::IdCard => mask_keep_ends(value, 6, 4),
        PiiKind::BankCard => mask_keep_ends(value, 0, 4),
        // Whole-span mask for everything else (path / email / address / NER kinds).
        _ => "*".repeat(value.chars().count().max(1)),
    }
}

/// Keep the first `head` and last `tail` characters, replacing the middle with `*` (one star per
/// hidden character). If the value is too short to keep both ends, mask everything.
fn mask_keep_ends(value: &str, head: usize, tail: usize) -> String {
    let chars: Vec<char> = value.chars().collect();
    let n = chars.len();
    if n <= head + tail {
        return "*".repeat(n.max(1));
    }
    let mut out = String::with_capacity(n);
    out.extend(chars[..head].iter());
    out.extend(std::iter::repeat_n('*', n - head - tail));
    out.extend(chars[n - tail..].iter());
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{PiiLayer, Span};

    fn hit(kind: PiiKind, start: usize, end: usize) -> PiiHit {
        PiiHit {
            kind,
            span: Span::new(start, end),
            confidence: 1.0,
            layer: PiiLayer::Regex,
            rule_id: "TEST".to_string(),
        }
    }

    #[test]
    fn a21_mask_phone() {
        let text = "13800138000";
        let masked = mask(text, &[hit(PiiKind::Phone, 0, 11)]);
        assert_eq!(masked, "138****8000");
    }

    #[test]
    fn a21_mask_idcard() {
        let text = "110101199001011234";
        let masked = mask(text, &[hit(PiiKind::IdCard, 0, 18)]);
        assert_eq!(masked, "110101********1234");
    }

    #[test]
    fn mask_bankcard_keeps_last_four() {
        let text = "6222021234567894";
        let masked = mask(text, &[hit(PiiKind::BankCard, 0, 16)]);
        assert_eq!(masked, "************7894");
    }

    #[test]
    fn mask_multiple_hits_in_sentence() {
        let text = "电话13800138000和邮箱a@b.com";
        let phone_start = text.find("13800138000").unwrap();
        let email_start = text.find("a@b.com").unwrap();
        let hits = vec![
            hit(PiiKind::Phone, phone_start, phone_start + 11),
            hit(PiiKind::Email, email_start, email_start + "a@b.com".len()),
        ];
        let masked = mask(text, &hits);
        assert!(masked.contains("138****8000"));
        assert!(!masked.contains("a@b.com"));
    }
}
