//! M1 诊断子类目录 (prd/04 §4.1.2 + data/01 §1.7).
//!
//! Embeds `data/diagnosis/catalog.yaml` (85 subcategories) at compile time and parses it with
//! `serde_norway` (W6, the maintained YAML fork already used for the 31-province region data). The
//! catalog carries, per subcategory: the `LD-NN-NN` code, the Chinese name, the coverage tier
//! (做深 / 做能用), the primary LawRef URN (§0.5, D8), and the recommended 主-并行-备用 procedures
//! (prd §4.1.2 `recommended_procedures`).
//!
//! INV-01 isolation: this module depends only on `serde` / `serde_norway` / `data_model`
//! (`CoverageTier`). It pulls in no AI / HTTP / sqlx / tantivy crate, so the diagnosis engine can
//! live in `rule-engine` without breaking `tests/isolation.rs`. The 20/85 count is asserted at
//! load time and cross-checked against the kb `categories.yaml` shape by `tests/diagnosis_tree.rs`.

use std::collections::BTreeMap;

use data_model::CoverageTier;
use serde::{Deserialize, Serialize};

use crate::error::RuleError;

/// The canonical diagnosis catalog YAML bundled with the crate (85 subcategories).
pub const DIAGNOSIS_CATALOG_YAML: &str = include_str!("../../data/diagnosis/catalog.yaml");

/// Total subcategory count (data/01 §1.7 / KBC-03).
pub const SUBCATEGORY_TOTAL: usize = 85;
/// Total top-level category count (data/01 §1.7 / KBC-03).
pub const CATEGORY_TOTAL: usize = 20;

/// A recommended legal procedure (prd §4.1.2 `recommended_procedures`). The `main` / `parallel` /
/// `fallback` slots (主路径 / 并行 / 备用) each carry one of these.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Procedure {
    /// 劳动仲裁.
    Arbitration,
    /// 法院诉讼.
    Litigation,
    /// 劳动监察投诉.
    Inspection,
    /// 调解.
    Mediation,
    /// 工伤认定.
    Recognition,
    /// 劳动能力 / 职业病鉴定.
    Appraisal,
    /// 协商.
    Negotiation,
    /// 刑事报案（拒不支付劳动报酬罪）.
    CriminalReport,
}

/// The 主-并行-备用 recommended-procedure triplet for a subcategory (prd §4.1.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecommendedProcedures {
    /// 主路径程序.
    pub main: Procedure,
    /// 并行程序（可空）.
    #[serde(default)]
    pub parallel: Vec<Procedure>,
    /// 备用程序（可选）.
    #[serde(default)]
    pub fallback: Option<Procedure>,
}

/// One catalog entry — a fully-described `LD-NN-NN` subcategory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogEntry {
    /// `LD-NN-NN` code (matches `case.dispute_category`).
    pub code: String,
    /// Chinese subcategory name (matches kb `categories.yaml`).
    pub name_zh: String,
    /// Coverage tier (做深 / 做能用).
    pub coverage_tier: CoverageTier,
    /// Primary LawRef URN (§0.5, D8).
    pub law_ref: String,
    /// Recommended 主-并行-备用 procedures.
    pub procedures: RecommendedProcedures,
}

/// The raw YAML document shape.
#[derive(Debug, Deserialize)]
struct CatalogDoc {
    subcategories: Vec<CatalogEntry>,
}

/// The parsed diagnosis catalog (85 subcategories), indexed by code for O(log n) lookup.
#[derive(Debug, Clone)]
pub struct DiagnosisCatalog {
    by_code: BTreeMap<String, CatalogEntry>,
    order: Vec<String>,
}

impl DiagnosisCatalog {
    /// Parse a catalog YAML and run the count + code-format + LawRef-URN assertions.
    pub fn parse(yaml: &str) -> Result<Self, RuleError> {
        let doc: CatalogDoc =
            serde_norway::from_str(yaml).map_err(|e| RuleError::Schema(e.to_string()))?;
        let mut by_code = BTreeMap::new();
        let mut order = Vec::with_capacity(doc.subcategories.len());
        for entry in doc.subcategories {
            if !is_subcategory_code(&entry.code) {
                return Err(RuleError::Schema(format!(
                    "catalog code is not LD-NN-NN: {}",
                    entry.code
                )));
            }
            // D8: every LawRef must be a valid §0.5 URN (delegates to the data-model parser; no
            // second parser is built here, INC-2).
            data_model::parse_law_ref(&entry.law_ref).map_err(|e| {
                RuleError::BadLawRef(format!("{} -> {}: {e}", entry.code, entry.law_ref))
            })?;
            order.push(entry.code.clone());
            if by_code.insert(entry.code.clone(), entry.clone()).is_some() {
                return Err(RuleError::Schema(format!(
                    "duplicate catalog code: {}",
                    entry.code
                )));
            }
        }
        let catalog = Self { by_code, order };
        catalog.validate()?;
        Ok(catalog)
    }

    /// Load the bundled default catalog (always valid; the engine's single source of truth).
    pub fn bundled() -> Result<Self, RuleError> {
        Self::parse(DIAGNOSIS_CATALOG_YAML)
    }

    /// Number of subcategories (must be 85).
    pub fn len(&self) -> usize {
        self.by_code.len()
    }

    /// Whether the catalog has no entries (always false for the bundled catalog).
    pub fn is_empty(&self) -> bool {
        self.by_code.is_empty()
    }

    /// Look up a subcategory by `LD-NN-NN` code.
    pub fn get(&self, code: &str) -> Option<&CatalogEntry> {
        self.by_code.get(code)
    }

    /// All entries in declaration order.
    pub fn entries(&self) -> impl Iterator<Item = &CatalogEntry> {
        self.order.iter().filter_map(move |c| self.by_code.get(c))
    }

    /// Distinct top-level category codes (`LD-NN`) present in the catalog.
    pub fn category_codes(&self) -> Vec<String> {
        let mut cats: Vec<String> = self
            .by_code
            .keys()
            .map(|c| c[..5].to_string())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        cats.sort();
        cats
    }

    /// KBC-03 assertion: exactly 85 subcategories across 20 categories, every code well-formed.
    pub fn validate(&self) -> Result<(), RuleError> {
        if self.by_code.len() != SUBCATEGORY_TOTAL {
            return Err(RuleError::Schema(format!(
                "catalog must have {SUBCATEGORY_TOTAL} subcategories, got {}",
                self.by_code.len()
            )));
        }
        let cat_count = self.category_codes().len();
        if cat_count != CATEGORY_TOTAL {
            return Err(RuleError::Schema(format!(
                "catalog must span {CATEGORY_TOTAL} categories, got {cat_count}"
            )));
        }
        Ok(())
    }
}

/// `^LD-[0-9]{2}-[0-9]{2}$` — a subcategory / `dispute_category` code (data/01 §1.7).
pub(crate) fn is_subcategory_code(code: &str) -> bool {
    let bytes = code.as_bytes();
    bytes.len() == 8
        && &bytes[0..3] == b"LD-"
        && bytes[3].is_ascii_digit()
        && bytes[4].is_ascii_digit()
        && bytes[5] == b'-'
        && bytes[6].is_ascii_digit()
        && bytes[7].is_ascii_digit()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_catalog_is_20_85() {
        let cat = DiagnosisCatalog::bundled().expect("bundled catalog parses");
        assert_eq!(cat.len(), 85, "must have 85 subcategories");
        assert_eq!(cat.category_codes().len(), 20, "must span 20 categories");
    }

    #[test]
    fn every_entry_has_valid_lawref_and_code() {
        let cat = DiagnosisCatalog::bundled().unwrap();
        for e in cat.entries() {
            assert!(is_subcategory_code(&e.code), "bad code {}", e.code);
            assert!(
                e.law_ref.starts_with("law:"),
                "lawref must be a URN: {}",
                e.law_ref
            );
            data_model::parse_law_ref(&e.law_ref)
                .unwrap_or_else(|err| panic!("{} bad lawref: {err}", e.code));
        }
    }

    #[test]
    fn deep_categories_are_make_deep() {
        let cat = DiagnosisCatalog::bundled().unwrap();
        // R1 做深 5 大类 (categories.yaml header): LD-01 / LD-02 / LD-03 / LD-05 / LD-09.
        for deep in ["LD-01", "LD-02", "LD-03", "LD-05", "LD-09"] {
            for e in cat.entries().filter(|e| e.code.starts_with(deep)) {
                assert_eq!(
                    e.coverage_tier,
                    CoverageTier::MakeDeep,
                    "{} must be make_deep",
                    e.code
                );
            }
        }
    }

    #[test]
    fn non_deep_category_is_make_usable() {
        let cat = DiagnosisCatalog::bundled().unwrap();
        let e = cat.get("LD-13-02").unwrap();
        assert_eq!(e.coverage_tier, CoverageTier::MakeUsable);
    }

    #[test]
    fn lookup_returns_procedures() {
        let cat = DiagnosisCatalog::bundled().unwrap();
        let e = cat.get("LD-01-01").unwrap();
        assert_eq!(e.procedures.main, Procedure::Arbitration);
        assert!(e.procedures.parallel.contains(&Procedure::Inspection));
        assert_eq!(e.procedures.fallback, Some(Procedure::Litigation));
    }
}
