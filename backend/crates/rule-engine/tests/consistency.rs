//! 31-province YAML schema completeness (ai/02 §2.5, ai/06 §6.6 → RE-02 / DL-07).

use rule_engine::region::coverage_meta::{ALL_PROVINCE_CODES, DEEP_PROVINCE_CODES};
use rule_engine::RegionTable;

#[test]
fn all_31_provinces_load() {
    let table = RegionTable::load_embedded().expect("all 31 province YAMLs load + schema-check");
    assert_eq!(table.len(), 31, "must embed exactly 31 provinces");
    for code in ALL_PROVINCE_CODES {
        assert!(table.get(code).is_some(), "province {code} missing");
    }
}

#[test]
fn deep_provinces_have_full_injury_arrays() {
    let table = RegionTable::load_embedded().unwrap();
    for code in DEEP_PROVINCE_CODES {
        let p = table.get(code).unwrap();
        assert!(p.deep_coverage, "{code} must be deep_coverage=true");
        assert_eq!(
            p.injury.lump_sum_months_by_grade.len(),
            10,
            "{code} lump_sum array length"
        );
        assert_eq!(
            p.injury.medical_subsidy_months_by_grade.len(),
            10,
            "{code} medical array length"
        );
        assert_eq!(
            p.injury.employment_subsidy_months_by_grade.len(),
            10,
            "{code} employment array length"
        );
        assert!(
            p.deadline.injury_assessment_days.is_some(),
            "{code} deep province must set injury_assessment_days"
        );
    }
}

#[test]
fn other_provinces_are_make_usable() {
    let table = RegionTable::load_embedded().unwrap();
    for code in ALL_PROVINCE_CODES {
        if DEEP_PROVINCE_CODES.contains(&code) {
            continue;
        }
        let p = table.get(code).unwrap();
        assert!(
            !p.deep_coverage,
            "{code} must be deep_coverage=false (make-usable)"
        );
    }
}

#[test]
fn lump_sum_months_match_national_table() {
    // 工伤保险条例 §35~§37 一次性伤残补助金月数全国统一：1级27 … 10级7。
    let expected = [27u8, 25, 23, 21, 18, 16, 13, 11, 9, 7];
    let table = RegionTable::load_embedded().unwrap();
    for code in DEEP_PROVINCE_CODES {
        let p = table.get(code).unwrap();
        assert_eq!(
            p.injury.lump_sum_months_by_grade.as_slice(),
            expected.as_slice(),
            "{code} lump-sum months must match national table"
        );
    }
}

#[test]
fn region_law_refs_are_valid_urns() {
    // RE-05: every region LawRef parses via the D8 URN parser (no short codes).
    let table = RegionTable::load_embedded().unwrap();
    for (code, p) in table.iter() {
        p.law_refs()
            .unwrap_or_else(|e| panic!("{code} has an invalid LawRef URN: {e}"));
    }
}
