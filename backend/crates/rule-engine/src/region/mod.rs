//! 31-province parameter loader (ai/02 §2.5, ai/06 §6.6).
//!
//! All YAMLs are embedded at compile time with `include_str!` so the leaf crate never reads the
//! filesystem or a DB at run time (INV-01 boundary). `load_embedded` parses + schema-checks all
//! 31 provinces; deep provinces (11/31/32/33/44) must carry full injury arrays.

pub mod coverage_meta;
pub mod schema;

pub use schema::RegionParams;

use std::collections::BTreeMap;

use crate::error::RuleError;
use coverage_meta::{is_deep, ALL_PROVINCE_CODES};

/// `(province_code, raw_yaml)` for each embedded province. Five deep provinces first.
const EMBEDDED: &[(&str, &str)] = &[
    ("11", include_str!("../../data/11_beijing.yaml")),
    ("31", include_str!("../../data/31_shanghai.yaml")),
    ("32", include_str!("../../data/32_jiangsu.yaml")),
    ("33", include_str!("../../data/33_zhejiang.yaml")),
    ("44", include_str!("../../data/44_guangdong.yaml")),
    ("12", include_str!("../../data/12_tianjin.yaml")),
    ("13", include_str!("../../data/13_hebei.yaml")),
    ("14", include_str!("../../data/14_shanxi.yaml")),
    ("15", include_str!("../../data/15_neimenggu.yaml")),
    ("21", include_str!("../../data/21_liaoning.yaml")),
    ("22", include_str!("../../data/22_jilin.yaml")),
    ("23", include_str!("../../data/23_heilongjiang.yaml")),
    ("34", include_str!("../../data/34_anhui.yaml")),
    ("35", include_str!("../../data/35_fujian.yaml")),
    ("36", include_str!("../../data/36_jiangxi.yaml")),
    ("37", include_str!("../../data/37_shandong.yaml")),
    ("41", include_str!("../../data/41_henan.yaml")),
    ("42", include_str!("../../data/42_hubei.yaml")),
    ("43", include_str!("../../data/43_hunan.yaml")),
    ("45", include_str!("../../data/45_guangxi.yaml")),
    ("46", include_str!("../../data/46_hainan.yaml")),
    ("50", include_str!("../../data/50_chongqing.yaml")),
    ("51", include_str!("../../data/51_sichuan.yaml")),
    ("52", include_str!("../../data/52_guizhou.yaml")),
    ("53", include_str!("../../data/53_yunnan.yaml")),
    ("54", include_str!("../../data/54_xizang.yaml")),
    ("61", include_str!("../../data/61_shaanxi.yaml")),
    ("62", include_str!("../../data/62_gansu.yaml")),
    ("63", include_str!("../../data/63_qinghai.yaml")),
    ("64", include_str!("../../data/64_ningxia.yaml")),
    ("65", include_str!("../../data/65_xinjiang.yaml")),
];

/// In-memory table of all 31 provinces, keyed by GB/T 2260 province code.
#[derive(Debug, Clone)]
pub struct RegionTable {
    by_code: BTreeMap<String, RegionParams>,
}

impl RegionTable {
    /// Parse + schema-check every embedded YAML. Fails fast (`RuleError`) if any province is
    /// missing, malformed, or violates a deep-coverage invariant.
    pub fn load_embedded() -> Result<Self, RuleError> {
        let mut by_code = BTreeMap::new();
        for (code, raw) in EMBEDDED {
            let params: RegionParams = serde_norway::from_str(raw)
                .map_err(|e| RuleError::RegionLoad(format!("{code}: {e}")))?;
            if params.region_code != *code {
                return Err(RuleError::Schema(format!(
                    "{code}: region_code 字段 {} 与文件名不一致",
                    params.region_code
                )));
            }
            validate(&params)?;
            by_code.insert((*code).to_string(), params);
        }
        // All 31 codes must be present.
        for code in ALL_PROVINCE_CODES {
            if !by_code.contains_key(code) {
                return Err(RuleError::Schema(format!("缺少省份 yaml：{code}")));
            }
        }
        Ok(Self { by_code })
    }

    /// Look up a province by its GB/T 2260 first-two-digit code.
    pub fn get(&self, province: &str) -> Option<&RegionParams> {
        self.by_code.get(province)
    }

    /// Number of loaded provinces (must be 31).
    pub fn len(&self) -> usize {
        self.by_code.len()
    }

    /// Always false for a successfully loaded table; provided to satisfy clippy.
    pub fn is_empty(&self) -> bool {
        self.by_code.is_empty()
    }

    /// Iterate over `(code, params)` pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &RegionParams)> {
        self.by_code.iter()
    }
}

/// Enforce schema invariants: deep provinces carry full injury arrays + assessment window;
/// LawRef URNs validate; `deep_coverage` flag matches the deep-province set.
fn validate(p: &RegionParams) -> Result<(), RuleError> {
    let deep = is_deep(&p.region_code);
    if p.deep_coverage != deep {
        return Err(RuleError::Schema(format!(
            "{}: deep_coverage={} 与深度省份集合不一致",
            p.region_code, p.deep_coverage
        )));
    }
    // LawRef URNs must validate via the D8 parser.
    p.law_refs()?;
    if deep {
        for (name, arr) in [
            ("lump_sum", &p.injury.lump_sum_months_by_grade),
            ("medical_subsidy", &p.injury.medical_subsidy_months_by_grade),
            (
                "employment_subsidy",
                &p.injury.employment_subsidy_months_by_grade,
            ),
        ] {
            if arr.len() != 10 {
                return Err(RuleError::Schema(format!(
                    "{}: injury.{name}_months_by_grade 长度 {} != 10",
                    p.region_code,
                    arr.len()
                )));
            }
        }
        if p.deadline.injury_assessment_days.is_none() {
            return Err(RuleError::Schema(format!(
                "{}: 深度省份必须填写 deadline.injury_assessment_days",
                p.region_code
            )));
        }
    }
    Ok(())
}
