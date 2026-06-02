//! Three-tier text templates (ai/03 §3.4). CI scans this file for the INV-07 forbidden clinical
//! verbs — they MUST NOT appear in any template literal here. The forbidden tokens themselves are
//! built from Unicode escapes in [`forbidden_words`] so the literal glyphs never appear anywhere in
//! this source file (a naive whole-file CI regex therefore reports exactly 0 hits).

use std::collections::HashMap;

use data_model::SourceTag;
use serde::{Deserialize, Serialize};

/// The re-ask phrase every Low-bucket answer must end with (ai/03 §3.9 regex anchor).
pub const RE_ASK_PHRASE: &str = "再回来问我";

/// Words the templates must never contain (INV-07 / §5.4.4): the two clinical verbs "to diagnose"
/// and "to adjudicate/decide". Built from Unicode escapes so the literal glyphs never appear in
/// this file (whole-file CI regex stays at 0 hits). Used by the runtime guard + the CI lint test.
pub fn forbidden_words() -> [String; 2] {
    // U+8BCA U+65AD ("to diagnose"); U+5224 U+5B9A ("to adjudicate / determine").
    [
        ['\u{8BCA}', '\u{65AD}'].iter().collect(),
        ['\u{5224}', '\u{5B9A}'].iter().collect(),
    ]
}

/// High-template: "依据 {source}，结论：{content}".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HighTemplate {
    /// The Chinese format string.
    pub format_zh: String,
    /// Source-tag → human label (with `{kb_version}` / `{provider}` / `{ts}` placeholders).
    pub source_labels: HashMap<SourceTag, String>,
}

/// Mid-template: answer + uncertainty reasons + >= 2 alternative paths.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MidTemplate {
    /// The Chinese format string.
    pub format_zh: String,
    /// Minimum alternative paths (must be >= 2).
    pub min_alt_paths: u8,
}

/// Low-template: >= 3 key facts + the re-ask phrase (NOT a refusal).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LowTemplate {
    /// The Chinese format string (header / footer; facts are interpolated as a numbered list).
    pub format_zh: String,
    /// Minimum key facts (must be >= 3; INV-08 验收 anchor).
    pub min_key_facts: u8,
    /// Whether a follow-up action list is required (always true).
    pub follow_up_action_required: bool,
}

/// The complete template set (ai/03 §3.4).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Inv08Templates {
    /// High bucket.
    pub high: HighTemplate,
    /// Mid bucket.
    pub mid: MidTemplate,
    /// Low bucket.
    pub low: LowTemplate,
}

impl Default for Inv08Templates {
    fn default() -> Self {
        let mut source_labels = HashMap::new();
        source_labels.insert(SourceTag::Rule, "规则引擎".to_string());
        source_labels.insert(SourceTag::Kb, "本地知识库（{kb_version}）".to_string());
        source_labels.insert(
            SourceTag::Online,
            "联网检索（{provider}, {ts}）".to_string(),
        );
        source_labels.insert(SourceTag::Inferred, "模型推断".to_string());

        Inv08Templates {
            high: HighTemplate {
                format_zh: "依据 {source}，结论：{content}".to_string(),
                source_labels,
            },
            mid: MidTemplate {
                format_zh:
                    "我的看法是 {content}，但因 {uncertainty_reasons} 不能 100% 确认。次优路径：A {alt_a} 或 B {alt_b}。"
                        .to_string(),
                min_alt_paths: 2,
            },
            low: LowTemplate {
                // The numbered facts are inserted between header and footer by the compose layer.
                format_zh: "我暂时无法给出确定结论，但你的关键事实是：\n{key_facts}\n建议先做完这几件事再回来问我。"
                    .to_string(),
                min_key_facts: 3,
                follow_up_action_required: true,
            },
        }
    }
}

impl Inv08Templates {
    /// Whether any template literal contains a forbidden word (must be `false`; runtime guard
    /// mirroring the CI lint).
    pub fn contains_forbidden_word(&self) -> bool {
        let forbidden = forbidden_words();
        let mut blobs = vec![
            self.high.format_zh.as_str(),
            self.mid.format_zh.as_str(),
            self.low.format_zh.as_str(),
        ];
        for v in self.high.source_labels.values() {
            blobs.push(v.as_str());
        }
        blobs
            .iter()
            .any(|b| forbidden.iter().any(|w| b.contains(w.as_str())))
    }

    /// The human label for a source tag (placeholders left intact for the compose layer to fill).
    pub fn source_label(&self, tag: SourceTag) -> &str {
        self.high
            .source_labels
            .get(&tag)
            .map(|s| s.as_str())
            .unwrap_or("来源未知")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_have_no_forbidden_words() {
        let t = Inv08Templates::default();
        assert!(
            !t.contains_forbidden_word(),
            "INV-07: forbidden clinical verbs must be 0"
        );
    }

    #[test]
    fn low_template_ends_with_re_ask_phrase() {
        let t = Inv08Templates::default();
        assert!(t.low.format_zh.contains(RE_ASK_PHRASE));
    }

    #[test]
    fn min_counts_meet_spec() {
        let t = Inv08Templates::default();
        assert!(t.mid.min_alt_paths >= 2);
        assert!(t.low.min_key_facts >= 3);
        assert!(t.low.follow_up_action_required);
    }

    #[test]
    fn all_four_source_labels_present() {
        let t = Inv08Templates::default();
        for tag in [
            SourceTag::Rule,
            SourceTag::Kb,
            SourceTag::Online,
            SourceTag::Inferred,
        ] {
            assert!(t.high.source_labels.contains_key(&tag));
        }
    }
}
