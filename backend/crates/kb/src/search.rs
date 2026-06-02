//! Hybrid law-ref search (backend/01 §1.7.2).
//!
//! R1a ships the **deep** BM25 single path: a `tantivy 0.26` in-memory index with a Chinese
//! word-segmentation tokenizer (`tantivy-jieba` bridging `jieba-rs`), so search genuinely hits on
//! Chinese statutory text (KB-01/02, LR integration). The vector backend (`sqlite-vec` / `pgvector`)
//! is blocked on the L0-03 DB-selection spike, so it is reserved behind the [`LawSearch`] trait and
//! an optional application-layer `hnsw_rs` leg (the `hnsw` cargo feature) — a make-usable hook, not
//! a stub on the live path. The default [`Bm25Index`] needs no vector backend.

use serde::{Deserialize, Serialize};
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{Field, Schema, Value, STORED, STRING, TEXT};
use tantivy::{doc, Index, TantivyDocument};

use crate::error::KbError;
use crate::sample_data::SampleLaw;

/// Tokenizer name registered for the Chinese-segmented full-text body field.
const JIEBA_TOKENIZER: &str = "jieba";

/// A single search hit (backend/01 §1.7.2): the law-ref URN, its BM25 score, and (best-effort)
/// matched spans for highlighting.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LawRefHit {
    pub stable_id: String,
    pub score: f32,
    /// Byte-offset spans within the body that matched (best-effort; empty when not computed).
    pub matched_spans: Vec<(usize, usize)>,
}

/// A `/law-ref` search query (backend/01 §1.7.2). `category` filters by `dispute_category` prefix
/// (LD-NN or LD-NN-NN); `province`/`city` map to region codes (D9, make-usable).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchQuery {
    pub q: String,
    pub category: Option<String>,
    pub province: Option<String>,
    pub city: Option<String>,
    pub limit: u32,
    pub offset: u32,
}

impl SearchQuery {
    /// Convenience constructor for a keyword query with a default page size of 20.
    pub fn keyword(q: impl Into<String>) -> Self {
        Self {
            q: q.into(),
            limit: 20,
            ..Default::default()
        }
    }
}

/// The hybrid search abstraction (backend/01 §1.7.2). R1a's concrete impl is [`Bm25Index`]; the
/// vector / hybrid legs plug in here once L0-03 lands.
pub trait LawSearch {
    /// Run a search, returning hits ordered by descending relevance.
    fn search(&self, q: &SearchQuery) -> Result<Vec<LawRefHit>, KbError>;
}

/// In-memory BM25 index over law-ref bodies with Chinese word segmentation (R1a deep path).
pub struct Bm25Index {
    index: Index,
    f_stable_id: Field,
    f_category: Field,
    f_region: Field,
    f_body: Field,
}

impl Bm25Index {
    /// Build an empty index whose `body` field uses the jieba Chinese tokenizer.
    pub fn new() -> Result<Self, KbError> {
        let mut builder = Schema::builder();
        let f_stable_id = builder.add_text_field("stable_id", STRING | STORED);
        let f_category = builder.add_text_field("category", STRING | STORED);
        let f_region = builder.add_text_field("region", STRING | STORED);
        // The body uses the registered jieba tokenizer for Chinese word segmentation.
        let body_indexing = tantivy::schema::TextFieldIndexing::default()
            .set_tokenizer(JIEBA_TOKENIZER)
            .set_index_option(tantivy::schema::IndexRecordOption::WithFreqsAndPositions);
        let body_options = tantivy::schema::TextOptions::default()
            .set_indexing_options(body_indexing)
            .set_stored();
        let _ = TEXT; // (TEXT default kept available for non-Chinese fields if needed.)
        let f_body = builder.add_text_field("body", body_options);
        let schema = builder.build();

        let index = Index::create_in_ram(schema);
        index
            .tokenizers()
            .register(JIEBA_TOKENIZER, tantivy_jieba::JiebaTokenizer::new());

        Ok(Self {
            index,
            f_stable_id,
            f_category,
            f_region,
            f_body,
        })
    }

    /// Build an index pre-loaded with the real sample corpus (KB integration test fixture).
    pub fn with_sample_data() -> Result<Self, KbError> {
        let mut idx = Self::new()?;
        idx.add_laws(crate::sample_data::SAMPLE_LAWS)?;
        Ok(idx)
    }

    /// Index a batch of sample laws (commits once at the end).
    pub fn add_laws(&mut self, laws: &[SampleLaw]) -> Result<(), KbError> {
        let mut writer = self
            .index
            .writer(50_000_000)
            .map_err(|e| KbError::Search(format!("writer: {e}")))?;
        for law in laws {
            writer
                .add_document(doc!(
                    self.f_stable_id => law.stable_id,
                    self.f_category => law.category,
                    self.f_region => law.region_code,
                    self.f_body => law.body,
                ))
                .map_err(|e| KbError::Search(format!("add_document: {e}")))?;
        }
        writer
            .commit()
            .map_err(|e| KbError::Search(format!("commit: {e}")))?;
        Ok(())
    }

    /// Number of indexed documents.
    pub fn doc_count(&self) -> Result<u64, KbError> {
        let reader = self
            .index
            .reader()
            .map_err(|e| KbError::Search(format!("reader: {e}")))?;
        Ok(reader.searcher().num_docs())
    }
}

impl LawSearch for Bm25Index {
    fn search(&self, q: &SearchQuery) -> Result<Vec<LawRefHit>, KbError> {
        let reader = self
            .index
            .reader()
            .map_err(|e| KbError::Search(format!("reader: {e}")))?;
        let searcher = reader.searcher();

        let parser = QueryParser::for_index(&self.index, vec![self.f_body]);
        // Disjunction (OR) default: BM25 ranks docs matching more query terms higher while still
        // surfacing partial matches — the right recall behaviour for free-text Chinese queries
        // where jieba segmentation of the query and the corpus may not produce identical token sets.

        // Tantivy's query parser treats many punctuation chars specially; for a free-text Chinese
        // query we lean on the field's jieba tokenizer and strip parser metacharacters.
        let sanitized = sanitize_query(&q.q);
        if sanitized.trim().is_empty() {
            return Ok(Vec::new());
        }
        let query = parser
            .parse_query(&sanitized)
            .map_err(|e| KbError::Search(format!("parse_query: {e}")))?;

        let limit = if q.limit == 0 { 20 } else { q.limit as usize };
        let want = limit + q.offset as usize;
        // In tantivy 0.26 `TopDocs::with_limit` is not itself a scoring `Collector`; `order_by_score`
        // yields the `Vec<(Score, DocAddress)>` BM25-ranked fruit.
        let collector = TopDocs::with_limit(want.max(1)).order_by_score();
        let top: Vec<(f32, tantivy::DocAddress)> = searcher
            .search(&query, &collector)
            .map_err(|e| KbError::Search(format!("search: {e}")))?;

        let mut hits = Vec::new();
        for (score, addr) in top.into_iter().skip(q.offset as usize) {
            let stored: TantivyDocument = searcher
                .doc(addr)
                .map_err(|e| KbError::Search(format!("doc: {e}")))?;
            let stable_id = stored
                .get_first(self.f_stable_id)
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();

            // Optional category / region post-filter (make-usable; D9 region routing).
            if let Some(cat) = &q.category {
                let doc_cat = stored
                    .get_first(self.f_category)
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                if !doc_cat.starts_with(cat.as_str()) {
                    continue;
                }
            }
            if let Some(prov) = &q.province {
                if let Some(code) = province_region_code(prov) {
                    let doc_region = stored
                        .get_first(self.f_region)
                        .and_then(|v| v.as_str())
                        .unwrap_or("00");
                    // National (00) clauses always pass; otherwise province code must match.
                    if doc_region != "00" && doc_region != code {
                        continue;
                    }
                }
            }

            hits.push(LawRefHit {
                stable_id,
                score,
                matched_spans: Vec::new(),
            });
        }
        Ok(hits)
    }
}

/// Strip tantivy query-parser metacharacters so a free-text Chinese query is treated as plain
/// terms (the jieba tokenizer then segments it). Keeps CJK, ASCII alphanumerics and spaces.
fn sanitize_query(q: &str) -> String {
    q.chars()
        .map(|c| match c {
            '+' | '-' | '!' | '(' | ')' | '{' | '}' | '[' | ']' | '^' | '"' | '~' | '*' | '?'
            | ':' | '\\' | '/' => ' ',
            other => other,
        })
        .collect()
}

/// Map a province name to its GB/T 2260 two-digit code for the five R1 deep provinces
/// (粤京沪苏浙, data/02 §2.5). Returns `None` for the 26 make-usable provinces (no region filter).
fn province_region_code(province: &str) -> Option<&'static str> {
    match province {
        "广东" | "广东省" => Some("44"),
        "北京" | "北京市" => Some("11"),
        "上海" | "上海市" => Some("31"),
        "江苏" | "江苏省" => Some("32"),
        "浙江" | "浙江省" => Some("33"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index() -> Bm25Index {
        Bm25Index::with_sample_data().expect("build sample index")
    }

    #[test]
    fn index_loads_all_sample_docs() {
        let idx = index();
        assert_eq!(
            idx.doc_count().unwrap() as usize,
            crate::sample_data::SAMPLE_LAWS.len()
        );
    }

    /// KB-01: a Chinese wage-arrears query hits the relevant 劳动合同法 / 工资 clauses.
    #[test]
    fn bm25_hits_wage_arrears() {
        let idx = index();
        let hits = idx
            .search(&SearchQuery::keyword("拖欠 工资 赔偿金"))
            .unwrap();
        assert!(!hits.is_empty(), "wage-arrears query must hit");
        // §85 (恶意欠薪加付赔偿金) or 广东条例 §44 should rank in the result set.
        assert!(hits
            .iter()
            .any(|h| h.stable_id.contains("§85") || h.stable_id.contains("广东省工资支付条例")));
        // scores are positive and descending.
        assert!(hits[0].score > 0.0);
        for w in hits.windows(2) {
            assert!(w[0].score >= w[1].score, "hits must be score-descending");
        }
    }

    /// KB-02: overtime query segments Chinese and hits 劳动法 §44 (150%/200%/300%).
    #[test]
    fn bm25_hits_overtime() {
        let idx = index();
        let hits = idx.search(&SearchQuery::keyword("加班 工资 报酬")).unwrap();
        assert!(hits.iter().any(|h| h.stable_id.contains("劳动法")));
    }

    #[test]
    fn bm25_hits_work_injury() {
        let idx = index();
        let hits = idx.search(&SearchQuery::keyword("工伤 认定 事故")).unwrap();
        assert!(hits.iter().any(|h| h.stable_id.contains("工伤保险条例")));
    }

    /// Category post-filter narrows to a dispute subtype prefix.
    #[test]
    fn category_filter_narrows_results() {
        let idx = index();
        let q = SearchQuery {
            q: "工资 拖欠".to_string(),
            category: Some("LD-03".to_string()),
            limit: 50,
            ..Default::default()
        };
        let hits = idx.search(&q).unwrap();
        assert!(!hits.is_empty());
        // every returned doc is an LD-03 wage clause (verified via known LD-03 URNs).
        for h in &hits {
            assert!(
                crate::sample_data::SAMPLE_LAWS
                    .iter()
                    .find(|l| l.stable_id == h.stable_id)
                    .map(|l| l.category.starts_with("LD-03"))
                    .unwrap_or(false),
                "non LD-03 hit leaked: {}",
                h.stable_id
            );
        }
    }

    /// Province filter keeps national clauses and Guangdong-coded clauses, drops other provinces.
    #[test]
    fn province_filter_keeps_national_and_matching() {
        let idx = index();
        let q = SearchQuery {
            q: "工资 拖欠 赔偿金".to_string(),
            province: Some("广东".to_string()),
            limit: 50,
            ..Default::default()
        };
        let hits = idx.search(&q).unwrap();
        // Guangdong clause (region 44) is allowed; no clause from a different province exists in
        // the corpus, so we only assert it can appear and national clauses still pass.
        assert!(hits.iter().any(|h| h.stable_id.contains("中华人民共和国"))); // national kept
    }

    #[test]
    fn empty_query_returns_empty() {
        let idx = index();
        assert!(idx.search(&SearchQuery::keyword("   ")).unwrap().is_empty());
        assert!(idx
            .search(&SearchQuery::keyword("()[]"))
            .unwrap()
            .is_empty());
    }

    #[test]
    fn offset_and_limit_paginate() {
        let idx = index();
        let page1 = idx
            .search(&SearchQuery {
                q: "劳动 合同".to_string(),
                limit: 2,
                offset: 0,
                ..Default::default()
            })
            .unwrap();
        let page2 = idx
            .search(&SearchQuery {
                q: "劳动 合同".to_string(),
                limit: 2,
                offset: 2,
                ..Default::default()
            })
            .unwrap();
        assert!(page1.len() <= 2);
        // pages do not overlap when both non-empty
        if !page1.is_empty() && !page2.is_empty() {
            assert_ne!(page1[0].stable_id, page2[0].stable_id);
        }
    }
}
