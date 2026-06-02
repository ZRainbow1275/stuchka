//! Layer-2 deterministic gazetteer NER (ai-12, R1b) — the GENUINE, zero-download, pure-Rust NER
//! backend that truly augments the regex strong-signal layer with the four entity kinds the regex
//! layer cannot catch: person names / organizations / medical keywords (address is already covered
//! by the regex layer). Every hit is traceable to a REAL dictionary entry / structural rule over
//! the actual input text — no probability is invented, no inference is faked.
//!
//! Why a gazetteer and not candle: the spec's illustrative Layer-2 (`ai/04` §4.4) is a candle
//! BertModel reading a ~400MB safetensors weight that is an OPTIONAL offline download and is not
//! shippable here (a network download is forbidden by D4 + `tests/isolation.rs`, and linking candle
//! breaks the zero-network/zero-Python isolation tree). Faking inference is forbidden. So the
//! candle/ONNX paths stay HONEST, explicitly-unlinked seams (see [`crate::ner_layer`]:
//! `NerBackend::{Candle, Onnx}` → `scan()` returns an explicit "not linked" error that downgrades
//! to R1a, mirroring Wave-2's cert/ISCC/GPG seams), and this gazetteer is the real R1b backend.
//! `ai/04` §4.10 already legitimises non-candle backends.
//!
//! Pure std (HashSet + char scan); NO new crate, so the isolation dependency tree stays byte-clean.

use std::collections::HashSet;
use std::sync::OnceLock;

use crate::types::{PiiHit, PiiKind, PiiLayer, Span};

/// Routing weights (NOT detection probabilities — a dictionary match is deterministic/certain).
/// Kept WEAK so a lone person/org never sets `is_high_sensitive` and a single hit never forces
/// local routing on its own (INC-7): person/org < 0.5 → Auto, medical 0.55 → WarnAndConfirm.
const W_PERSON: f32 = 0.45;
const W_ORG: f32 = 0.45;
const W_MEDICAL: f32 = 0.55;

/// Max characters of a person given-name (after the surname) the heuristic will absorb.
const MAX_GIVEN_NAME_CHARS: usize = 2;
/// Max characters of an organization name (prefix + suffix) to bound the backward scan.
const MAX_ORG_CHARS: usize = 30;
/// Minimum Han prefix chars before an org suffix to count as a NAMED organization (avoids bare
/// generic suffixes like "公司" / "集团" matching on their own).
const MIN_ORG_PREFIX_CHARS: usize = 2;

fn is_han(c: char) -> bool {
    ('\u{4E00}'..='\u{9FFF}').contains(&c)
}

/// The process-shared gazetteer, compiled once from the in-tree dictionaries.
pub fn global() -> &'static GazetteerNer {
    static G: OnceLock<GazetteerNer> = OnceLock::new();
    G.get_or_init(GazetteerNer::load)
}

/// Deterministic dictionary/structural NER over Chinese text.
pub struct GazetteerNer {
    single_surnames: HashSet<char>,
    compound_surnames: HashSet<[char; 2]>,
    /// Org suffixes as char vectors, sorted LONGEST first (longest-suffix-wins).
    org_suffixes: Vec<Vec<char>>,
    /// Medical keywords as char vectors, sorted LONGEST first.
    medical_keywords: Vec<Vec<char>>,
    /// Surname-initial common words that are NOT person names (real false-positive control).
    name_stoplist: HashSet<String>,
    /// Common given-name characters; the given name only extends over chars in this set so the
    /// person scan stops at following words (e.g. 张伟|昨天) instead of over-capturing.
    given_name_chars: HashSet<char>,
}

fn parse_lines(raw: &str) -> impl Iterator<Item = &str> {
    raw.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
}

impl GazetteerNer {
    fn load() -> Self {
        let surnames_raw = include_str!("ner_data/chinese_surnames.txt");
        let org_raw = include_str!("ner_data/org_suffixes.txt");
        let med_raw = include_str!("ner_data/medical_keywords.txt");
        let given_raw = include_str!("ner_data/given_name_chars.txt");

        let mut single_surnames = HashSet::new();
        let mut compound_surnames = HashSet::new();
        for line in parse_lines(surnames_raw) {
            let chars: Vec<char> = line.chars().collect();
            match chars.as_slice() {
                [c] => {
                    single_surnames.insert(*c);
                }
                [a, b] => {
                    compound_surnames.insert([*a, *b]);
                }
                _ => {} // ignore malformed entries (never panic)
            }
        }

        let mut org_suffixes: Vec<Vec<char>> =
            parse_lines(org_raw).map(|l| l.chars().collect()).collect();
        org_suffixes.sort_by_key(|v| std::cmp::Reverse(v.len()));

        let mut medical_keywords: Vec<Vec<char>> =
            parse_lines(med_raw).map(|l| l.chars().collect()).collect();
        medical_keywords.sort_by_key(|v| std::cmp::Reverse(v.len()));

        // Common surname-initial words that are NOT names (genuine FP control; the negative-control
        // unit tests assert these are NOT emitted as PersonName).
        let name_stoplist: HashSet<String> = [
            "马上", "黄牛", "张望", "王八", "高兴", "江山", "白天", "白菜", "朱红", "高度",
            "高速", "时间", "万一", "万分", "方便", "史上", "江湖", "何必", "何况", "毛病",
            "石头", "金子", "钱财", "汤水", "常常", "陶醉", "贾人", "曹操", // 曹操 is historical, suppress as common token
            "孔子", "孟子", "老板", "经理", // bare titles are not names on their own
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        let given_name_chars: HashSet<char> = parse_lines(given_raw)
            .flat_map(|line| line.chars().filter(|c| is_han(*c)))
            .collect();

        Self {
            single_surnames,
            compound_surnames,
            org_suffixes,
            medical_keywords,
            name_stoplist,
            given_name_chars,
        }
    }

    /// Scan ORIGINAL text (spans are returned in original byte offsets — exact, A11). Detection
    /// order: medical keywords, then organizations, then person names; person names are de-overlapped
    /// against the org/medical spans already taken so an org-internal Han run is not double-counted.
    pub fn scan(&self, text: &str) -> Vec<PiiHit> {
        // (byte_offset, char) for each char; byte end of char i is the next char's offset (or len).
        let idx: Vec<(usize, char)> = text.char_indices().collect();
        let n = idx.len();
        let byte_end = |char_pos: usize| -> usize {
            if char_pos < n {
                idx[char_pos].0
            } else {
                text.len()
            }
        };

        let mut hits: Vec<PiiHit> = Vec::new();
        // Track char positions already claimed by org/medical so person scan de-overlaps.
        let mut claimed = vec![false; n];

        // 1) Medical keywords (longest-first, left-to-right, non-overlapping).
        let mut i = 0;
        while i < n {
            let mut matched = false;
            for kw in &self.medical_keywords {
                let klen = kw.len();
                if i + klen <= n && (0..klen).all(|k| idx[i + k].1 == kw[k]) {
                    hits.push(PiiHit {
                        kind: PiiKind::MedicalKeyword,
                        span: Span::new(idx[i].0, byte_end(i + klen)),
                        confidence: W_MEDICAL,
                        layer: PiiLayer::Ner,
                        rule_id: "NER-MED".to_string(),
                    });
                    for c in claimed.iter_mut().skip(i).take(klen) {
                        *c = true;
                    }
                    i += klen;
                    matched = true;
                    break;
                }
            }
            if !matched {
                i += 1;
            }
        }

        // 2) Organizations: a Han run ENDING in an org suffix (≥ MIN_ORG_PREFIX_CHARS Han prefix).
        let mut i = 0;
        while i < n {
            let mut matched = false;
            for suf in &self.org_suffixes {
                let slen = suf.len();
                if i + slen <= n && (0..slen).all(|k| idx[i + k].1 == suf[k]) {
                    // Extend left over Han chars to capture the org name prefix.
                    let mut start = i;
                    let mut prefix = 0usize;
                    while start > 0
                        && is_han(idx[start - 1].1)
                        && (i - (start - 1)) <= MAX_ORG_CHARS
                    {
                        start -= 1;
                        prefix += 1;
                    }
                    if prefix >= MIN_ORG_PREFIX_CHARS {
                        let end_char = i + slen;
                        hits.push(PiiHit {
                            kind: PiiKind::Organization,
                            span: Span::new(idx[start].0, byte_end(end_char)),
                            confidence: W_ORG,
                            layer: PiiLayer::Ner,
                            rule_id: "NER-ORG".to_string(),
                        });
                        for c in claimed.iter_mut().take(end_char).skip(start) {
                            *c = true;
                        }
                        i = end_char;
                        matched = true;
                        break;
                    }
                }
            }
            if !matched {
                i += 1;
            }
        }

        // 3) Person names: surname (compound 2-char or single) + 1..=2 Han given-name chars,
        //    de-overlapped against org/medical spans, filtered by the non-name stoplist.
        let mut i = 0;
        while i < n {
            if claimed[i] {
                i += 1;
                continue;
            }
            // Compound surname (2 chars) first, then single.
            let surname_len = if i + 2 <= n
                && self
                    .compound_surnames
                    .contains(&[idx[i].1, idx[i + 1].1])
            {
                2
            } else if self.single_surnames.contains(&idx[i].1) {
                1
            } else {
                0
            };
            if surname_len == 0 {
                i += 1;
                continue;
            }
            // Given name = consecutive chars (after the surname) drawn from the common
            // given-name-character set, capped at MAX_GIVEN_NAME_CHARS. Bounding to real name
            // characters stops over-capture into the following word (张伟|昨天 → "张伟", not "张伟昨")
            // and correctly extends compound names (欧阳|娜娜 → "欧阳娜娜"). Most surname-initial
            // non-names (马上/黄牛/张望) are filtered here because their 2nd char is not a name char.
            let gstart = i + surname_len;
            let mut given = 0usize;
            while given < MAX_GIVEN_NAME_CHARS {
                let k = gstart + given;
                if k < n && !claimed[k] && self.given_name_chars.contains(&idx[k].1) {
                    given += 1;
                } else {
                    break;
                }
            }
            if given == 0 {
                i += 1;
                continue;
            }
            let end_char = gstart + given;
            let candidate: String = idx[i..end_char].iter().map(|(_, c)| *c).collect();
            if self.name_stoplist.contains(&candidate) {
                i += 1; // a known non-name bigram — skip this surname position.
                continue;
            }
            hits.push(PiiHit {
                kind: PiiKind::PersonName,
                span: Span::new(idx[i].0, byte_end(end_char)),
                confidence: W_PERSON,
                layer: PiiLayer::Ner,
                rule_id: "NER-PER".to_string(),
            });
            i = end_char;
        }

        hits
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(text: &str) -> Vec<(PiiKind, String)> {
        global()
            .scan(text)
            .into_iter()
            .map(|h| (h.kind, text[h.span.start..h.span.end].to_string()))
            .collect()
    }

    #[test]
    fn detects_single_surname_name() {
        let hits = kinds("我同事张伟昨天来了");
        assert!(
            hits.iter().any(|(k, s)| *k == PiiKind::PersonName && s == "张伟"),
            "got {hits:?}"
        );
    }

    #[test]
    fn detects_compound_surname_name() {
        let hits = kinds("联系人是欧阳娜娜");
        assert!(
            hits.iter().any(|(k, s)| *k == PiiKind::PersonName && s == "欧阳娜娜"),
            "got {hits:?}"
        );
    }

    #[test]
    fn detects_organization() {
        let hits = kinds("我在北京华夏科技有限公司上班");
        assert!(
            hits.iter()
                .any(|(k, s)| *k == PiiKind::Organization && s.ends_with("有限公司") && s.len() > "有限公司".len()),
            "got {hits:?}"
        );
    }

    #[test]
    fn detects_arbitration_committee_org() {
        let hits = kinds("向广州市劳动人事争议仲裁委员会申请仲裁");
        assert!(
            hits.iter().any(|(k, _)| *k == PiiKind::Organization),
            "got {hits:?}"
        );
    }

    #[test]
    fn detects_medical_keyword() {
        let hits = kinds("我有住院病历和工伤鉴定结论");
        assert!(hits.iter().any(|(k, s)| *k == PiiKind::MedicalKeyword && s == "住院病历"));
        assert!(hits.iter().any(|(k, s)| *k == PiiKind::MedicalKeyword && s == "工伤鉴定"));
    }

    #[test]
    fn negative_control_surname_initial_nonnames() {
        // Real discrimination: surname-initial common words are NOT emitted as person names.
        for w in ["马上", "黄牛", "张望", "王八"] {
            let sentence = format!("他{w}就走");
            let hits = kinds(&sentence);
            assert!(
                !hits.iter().any(|(k, s)| *k == PiiKind::PersonName && s == w),
                "{w} must NOT be a PersonName; got {hits:?}"
            );
        }
    }

    #[test]
    fn span_maps_back_exactly() {
        let text = "证人 张伟 出庭"; // includes spaces around the name
        for h in global().scan(text) {
            if h.kind == PiiKind::PersonName {
                assert_eq!(&text[h.span.start..h.span.end], "张伟");
            }
        }
    }
}
