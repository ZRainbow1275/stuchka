//! D8 URN parser (`data/02` §2.4). Splits a stable_id into its six locating dimensions.
//!
//! Grammar (`data/02` §2.2):
//!
//! ```text
//! law:<法规全称>/v<版本日期>/§<条号>/¶<款序号>/(<项序号>)/§<段序号>
//! ```
//!
//! - `law:` literal prefix is mandatory.
//! - first segment after the prefix is the (Chinese, full-width allowed) statute title.
//! - `v<date>` parses as a chrono `NaiveDate`; failure → [`LawRefError::InvalidVersionDate`].
//! - `§<n>` (article) is mandatory (missing → [`LawRefError::MissingArticle`]).
//! - `¶<n>` (paragraph), `(<项>)` (item, Chinese or Arabic numerals), `§<n>` (section) are optional.
//!
//! The parser is hand-written (no nom / pest dependency) so the leaf crate stays dependency-light
//! and the build is hermetic. It refuses any malformed segment rather than degrading (avoids dirty
//! data per §2.4). `to_stable_id(parse(x)) == x` round-trips for every well-formed URN (LR-04).

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// Bracket form used for the 项 (item) segment of a D8 URN. The spec (`data/02` §2.2) writes the
/// item example #6 with **half-width** `(三)` (U+0028/U+0029), while statute *titles* may carry
/// full-width `（）` (U+FF08/U+FF09) verbatim. Recording the form lets [`to_stable_id`] reproduce
/// the parsed input byte-for-byte (LR-04 round-trip).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemBracket {
    /// Half-width `(` `)` (U+0028 / U+0029) — the form the §2.2 spec example uses.
    HalfWidth,
    /// Full-width `（` `）` (U+FF08 / U+FF09).
    FullWidth,
}

impl ItemBracket {
    fn open(self) -> char {
        match self {
            ItemBracket::HalfWidth => '(',
            ItemBracket::FullWidth => '\u{FF08}',
        }
    }
    fn close(self) -> char {
        match self {
            ItemBracket::HalfWidth => ')',
            ItemBracket::FullWidth => '\u{FF09}',
        }
    }
}

/// The six locating dimensions of a D8 URN (`data/02` §2.4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LawRefParts {
    /// Official Chinese full title (full-width characters allowed; parentheses not stripped).
    pub title: String,
    /// Promulgation / revision date (ISO `v2025-09-01`).
    pub version_date: NaiveDate,
    /// 条 (article) — mandatory.
    pub article: u32,
    /// 款 (paragraph) — optional.
    pub paragraph: Option<u32>,
    /// 项 (item) — optional; Chinese numerals (一/二/三) or Arabic allowed, kept verbatim (inner
    /// text only, without its brackets).
    pub item: Option<String>,
    /// Bracket form of the 项 (item) segment, captured at parse time so [`to_stable_id`] round-trips
    /// the exact spec form (half-width `(三)` per §2.2). `None` iff `item` is `None`; when an item is
    /// built programmatically without a recorded form, [`to_stable_id`] defaults to half-width to
    /// match the §2.2 grammar `(<项序号>)`.
    #[serde(default)]
    pub item_bracket: Option<ItemBracket>,
    /// 段 (section) — optional, rarely used.
    pub section: Option<u32>,
}

/// Parse failure for a D8 URN (`data/02` §2.4). Carries the precise reason; the parser never
/// silently degrades.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LawRefError {
    #[error("missing law: prefix")]
    MissingPrefix,
    #[error("invalid version date: {0}")]
    InvalidVersionDate(String),
    #[error("article required")]
    MissingArticle,
    #[error("malformed segment: {0}")]
    MalformedSegment(String),
}

const PREFIX: &str = "law:";
const ARTICLE_MARK: char = '\u{00A7}'; // §
const PARAGRAPH_MARK: char = '\u{00B6}'; // ¶

/// Parse a D8 URN into [`LawRefParts`] (`data/02` §2.4). Returns a precise [`LawRefError`] on any
/// malformed input.
pub fn parse(stable_id: &str) -> Result<LawRefParts, LawRefError> {
    // 1. Mandatory `law:` literal prefix.
    let rest = stable_id
        .strip_prefix(PREFIX)
        .ok_or(LawRefError::MissingPrefix)?;

    // 2. Split the remainder on `/`. First segment is the statute title.
    let mut segments = rest.split('/');
    let title = segments
        .next()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| LawRefError::MalformedSegment("missing title".to_string()))?
        .to_string();

    // 3. `v<date>` is mandatory and must be the second segment.
    let version_seg = segments
        .next()
        .ok_or_else(|| LawRefError::MalformedSegment("missing version date".to_string()))?;
    let date_str = version_seg
        .strip_prefix('v')
        .ok_or_else(|| LawRefError::MalformedSegment(version_seg.to_string()))?;
    let version_date = NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
        .map_err(|_| LawRefError::InvalidVersionDate(date_str.to_string()))?;

    // 4. Remaining optional segments: §<article> (mandatory), ¶<paragraph>, (<item>), §<section>.
    let mut article: Option<u32> = None;
    let mut paragraph: Option<u32> = None;
    let mut item: Option<String> = None;
    let mut item_bracket: Option<ItemBracket> = None;
    let mut section: Option<u32> = None;

    for seg in segments {
        if seg.is_empty() {
            return Err(LawRefError::MalformedSegment("empty segment".to_string()));
        }
        let mut chars = seg.chars();
        let lead = chars.next().expect("non-empty segment");
        match lead {
            ARTICLE_MARK => {
                // §<n> — the first occurrence is the article, a later one is the section.
                let n = parse_u32(&seg[ARTICLE_MARK.len_utf8()..], seg)?;
                if article.is_none() {
                    article = Some(n);
                } else if section.is_none() {
                    section = Some(n);
                } else {
                    return Err(LawRefError::MalformedSegment(seg.to_string()));
                }
            }
            PARAGRAPH_MARK => {
                if paragraph.is_some() {
                    return Err(LawRefError::MalformedSegment(seg.to_string()));
                }
                paragraph = Some(parse_u32(&seg[PARAGRAPH_MARK.len_utf8()..], seg)?);
            }
            '(' | '\u{FF08}' => {
                // (<item>) — half-width `(` or full-width `（`; content kept verbatim. The exact
                // bracket form is recorded so to_stable_id round-trips byte-for-byte (LR-04).
                if item.is_some() {
                    return Err(LawRefError::MalformedSegment(seg.to_string()));
                }
                let (inner, bracket) = strip_item_brackets(seg)
                    .ok_or_else(|| LawRefError::MalformedSegment(seg.to_string()))?;
                if inner.is_empty() {
                    return Err(LawRefError::MalformedSegment(seg.to_string()));
                }
                item = Some(inner.to_string());
                item_bracket = Some(bracket);
            }
            _ => return Err(LawRefError::MalformedSegment(seg.to_string())),
        }
    }

    let article = article.ok_or(LawRefError::MissingArticle)?;

    Ok(LawRefParts {
        title,
        version_date,
        article,
        paragraph,
        item,
        item_bracket,
        section,
    })
}

/// Rebuild the canonical D8 URN from [`LawRefParts`] (`data/02` §2.4). Inverse of [`parse`];
/// `to_stable_id(&parse(x)?) == x` byte-for-byte for every well-formed URN (LR-04). The item
/// segment is re-emitted with the **exact** bracket form captured at parse time
/// ([`LawRefParts::item_bracket`]); programmatically-built parts with no recorded form default to
/// half-width `(三)` to match the §2.2 grammar `(<项序号>)`. §/¶ stay full-width per §0.5.
pub fn to_stable_id(parts: &LawRefParts) -> String {
    let mut out = String::with_capacity(64);
    out.push_str(PREFIX);
    out.push_str(&parts.title);
    out.push_str("/v");
    out.push_str(&parts.version_date.format("%Y-%m-%d").to_string());
    out.push('/');
    out.push(ARTICLE_MARK);
    out.push_str(&parts.article.to_string());
    if let Some(p) = parts.paragraph {
        out.push('/');
        out.push(PARAGRAPH_MARK);
        out.push_str(&p.to_string());
    }
    if let Some(it) = &parts.item {
        let bracket = parts.item_bracket.unwrap_or(ItemBracket::HalfWidth);
        out.push('/');
        out.push(bracket.open());
        out.push_str(it);
        out.push(bracket.close());
    }
    if let Some(s) = parts.section {
        out.push('/');
        out.push(ARTICLE_MARK);
        out.push_str(&s.to_string());
    }
    out
}

/// Parse the numeric tail of a `§`/`¶` segment, attributing parse errors to the whole segment.
fn parse_u32(digits: &str, full_segment: &str) -> Result<u32, LawRefError> {
    digits
        .parse::<u32>()
        .map_err(|_| LawRefError::MalformedSegment(full_segment.to_string()))
}

/// Strip the surrounding `()` / `（）` from an item segment, returning the inner text together with
/// the exact bracket form so the round-trip can reproduce it (LR-04). Mismatched open/close
/// brackets (e.g. `(三）`) are rejected as malformed (returns `None`).
fn strip_item_brackets(seg: &str) -> Option<(&str, ItemBracket)> {
    if let Some(inner) = seg.strip_prefix('(') {
        inner.strip_suffix(')').map(|s| (s, ItemBracket::HalfWidth))
    } else if let Some(inner) = seg.strip_prefix('\u{FF08}') {
        inner
            .strip_suffix('\u{FF09}')
            .map(|s| (s, ItemBracket::FullWidth))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The seven §2.2 example URNs (LR-04 table), copied **verbatim** from
    /// `prompts/0529/spec/data/02-law-ref-namespace.md` §2.2. The item example #6 uses **half-width**
    /// brackets `(三)` (U+0028/U+0029) exactly as the spec writes it; the full-width `（二）` in the
    /// §19/§2/§11 entries are part of the statute *title*, not the item segment. The §/¶ symbols are
    /// full-width per §0.5. Round-trip fidelity here is byte-for-byte (LR-04).
    const EXAMPLES: [&str; 7] = [
        "law:中华人民共和国劳动合同法/v2012-12-28/§39/¶1",
        "law:中华人民共和国社会保险法/v2018-12-29/§63",
        "law:最高人民法院关于审理劳动争议案件适用法律问题的解释（二）/v2025-09-01/§19",
        "law:最高人民法院关于审理劳动争议案件适用法律问题的解释（二）/v2025-09-01/§2",
        "law:最高人民法院关于审理劳动争议案件适用法律问题的解释（二）/v2025-09-01/§11",
        "law:工伤保险条例/v2010-12-20/§14/¶1/(三)",
        "law:广东省工资支付条例/v2016-09-29/§44",
    ];

    /// LR-04: all 7 §2.2 examples parse and round-trip **byte-for-byte** via
    /// `to_stable_id(parse(x)) == x`, including the half-width item bracket form `(三)` and the
    /// full-width `§` `¶` symbols, exactly as the spec writes them (no normalization).
    #[test]
    fn lr04_all_examples_parse_and_roundtrip() {
        for urn in EXAMPLES {
            let parts = parse(urn).unwrap_or_else(|e| panic!("parse failed for {urn}: {e}"));
            let rebuilt = to_stable_id(&parts);
            assert_eq!(rebuilt, urn, "round-trip mismatch for {urn}");
            // Stronger than string equality: assert byte-for-byte identity.
            assert_eq!(
                rebuilt.as_bytes(),
                urn.as_bytes(),
                "byte-level round-trip mismatch for {urn}"
            );
        }
    }

    /// LR-04 detail: example #6 keeps the spec's **half-width** item brackets `(三)` verbatim and
    /// does NOT get normalized to full-width `（三）`.
    #[test]
    fn lr04_item_bracket_form_is_preserved_verbatim() {
        let half = "law:工伤保险条例/v2010-12-20/§14/¶1/(三)";
        let parts = parse(half).unwrap();
        assert_eq!(parts.item.as_deref(), Some("三"));
        assert_eq!(parts.item_bracket, Some(ItemBracket::HalfWidth));
        assert_eq!(to_stable_id(&parts), half);

        // A full-width item segment also round-trips as full-width.
        let full = "law:某法/v2020-01-01/§14/¶1/（三）";
        let fparts = parse(full).unwrap();
        assert_eq!(fparts.item_bracket, Some(ItemBracket::FullWidth));
        assert_eq!(to_stable_id(&fparts), full);
    }

    #[test]
    fn lr04_parses_each_dimension() {
        let p = parse("law:中华人民共和国劳动合同法/v2012-12-28/§39/¶1").unwrap();
        assert_eq!(p.title, "中华人民共和国劳动合同法");
        assert_eq!(
            p.version_date,
            NaiveDate::from_ymd_opt(2012, 12, 28).unwrap()
        );
        assert_eq!(p.article, 39);
        assert_eq!(p.paragraph, Some(1));
        assert_eq!(p.item, None);
        assert_eq!(p.item_bracket, None);
        assert_eq!(p.section, None);

        let q = parse("law:工伤保险条例/v2010-12-20/§14/¶1/(三)").unwrap();
        assert_eq!(q.article, 14);
        assert_eq!(q.paragraph, Some(1));
        assert_eq!(q.item.as_deref(), Some("三"));
        assert_eq!(q.item_bracket, Some(ItemBracket::HalfWidth));
    }

    /// LR-05: missing `law:` prefix → MissingPrefix.
    #[test]
    fn lr05_missing_prefix() {
        assert_eq!(
            parse("中华人民共和国劳动合同法/v2012-12-28/§39"),
            Err(LawRefError::MissingPrefix)
        );
    }

    /// LR-06: missing article → MissingArticle.
    #[test]
    fn lr06_missing_article() {
        assert_eq!(
            parse("law:中华人民共和国社会保险法/v2018-12-29"),
            Err(LawRefError::MissingArticle)
        );
        // a paragraph without an article is also MissingArticle
        assert_eq!(
            parse("law:某法/v2020-01-01/¶1"),
            Err(LawRefError::MissingArticle)
        );
    }

    /// LR-07: invalid version date → InvalidVersionDate.
    #[test]
    fn lr07_invalid_version_date() {
        assert_eq!(
            parse("law:某法/v2020-13-40/§1"),
            Err(LawRefError::InvalidVersionDate("2020-13-40".to_string()))
        );
        // missing the leading `v` is a malformed segment, not an invalid date
        assert!(matches!(
            parse("law:某法/2020-01-01/§1"),
            Err(LawRefError::MalformedSegment(_))
        ));
    }

    #[test]
    fn rejects_malformed_article_number() {
        assert!(matches!(
            parse("law:某法/v2020-01-01/§abc"),
            Err(LawRefError::MalformedSegment(_))
        ));
    }

    #[test]
    fn half_width_item_brackets_parse() {
        let p = parse("law:某法/v2020-01-01/§14/¶1/(3)").unwrap();
        assert_eq!(p.item.as_deref(), Some("3"));
        assert_eq!(p.item_bracket, Some(ItemBracket::HalfWidth));
        assert_eq!(to_stable_id(&p), "law:某法/v2020-01-01/§14/¶1/(3)");
    }

    /// Mismatched open/close item brackets (half-open + full-close) are malformed, not silently
    /// normalized.
    #[test]
    fn rejects_mismatched_item_brackets() {
        assert!(matches!(
            parse("law:某法/v2020-01-01/§14/¶1/(三）"),
            Err(LawRefError::MalformedSegment(_))
        ));
        assert!(matches!(
            parse("law:某法/v2020-01-01/§14/¶1/（三)"),
            Err(LawRefError::MalformedSegment(_))
        ));
    }
}
