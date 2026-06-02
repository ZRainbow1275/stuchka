//! M1 诊断引擎集成测试 (prd/04 §4.1) — 问诊树全覆盖 + 子类正确性 + 目录与 kb 一致。
//!
//! - 决策树覆盖: 每条 root→leaf 路径终止于一个有效子类 (tree::validate 已在加载时强制, 此处
//!   穷举枚举每条路径再次断言, 并校验全部 85 子类可达)。
//! - 代表性 case → 子类正确性 (做深 5 类 + 做能用样本)。
//! - 目录与 `crates/kb/assets/categories.yaml` 的 20/85 + 子类编码集一致 (单一真相源)。
//! - INV-01 隔离断言仍成立 (诊断引擎并入 rule-engine 不引入 AI/HTTP/sqlx) 见 tests/isolation.rs。

use std::collections::BTreeSet;

use rule_engine::diagnosis::tree::{Node, ROOT_ID};
use rule_engine::{DecisionTree, DiagnosisCatalog, DiagnosisEngine, DiagnosisOutput};

fn engine() -> DiagnosisEngine {
    DiagnosisEngine::new().expect("diagnosis engine loads (catalog + tree validate)")
}

/// Enumerate every root→leaf path; assert each ends at a valid catalog subcategory within 7 Qs.
#[test]
fn every_decision_path_reaches_a_valid_subcategory() {
    let catalog = DiagnosisCatalog::bundled().unwrap();
    let tree = DecisionTree::bundled(&catalog).unwrap();

    let mut reached: BTreeSet<String> = BTreeSet::new();
    enumerate(&tree, ROOT_ID, 1, &mut reached, &catalog);

    // All 85 subcategories must be reachable (做能用以上全量覆盖, prd §4.1.5).
    assert_eq!(reached.len(), 85, "all 85 subcategories must be reachable");
    for e in catalog.entries() {
        assert!(
            reached.contains(&e.code),
            "subcategory {} unreachable",
            e.code
        );
    }
}

/// Recursive enumeration: bounded at 7 questions; every leaf must be a real subcategory.
fn enumerate(
    tree: &DecisionTree,
    node_id: &str,
    depth: usize,
    reached: &mut BTreeSet<String>,
    catalog: &DiagnosisCatalog,
) {
    match tree.node(node_id) {
        Some(Node::Leaf(l)) => {
            assert!(
                catalog.get(&l.subcategory).is_some(),
                "leaf {node_id} -> unknown subcategory {}",
                l.subcategory
            );
            reached.insert(l.subcategory.clone());
        }
        Some(Node::Question(q)) => {
            assert!(
                depth <= 7,
                "path through {node_id} exceeds 7 questions (prd §4.1.4)"
            );
            for a in &q.answers {
                enumerate(tree, &a.next, depth + 1, reached, catalog);
            }
        }
        None => panic!("dangling node id {node_id}"),
    }
}

/// Representative case → subcategory correctness across the 5 做深 categories + make-usable samples.
#[test]
fn representative_cases_diagnose_correctly() {
    let eng = engine();
    let cases: &[(&[&str], &str)] = &[
        // 做深 5 类.
        (&["contract_formation", "no_written"], "LD-01-01"),
        (&["termination", "illegal_termination"], "LD-02-01"),
        (&["wage", "malicious"], "LD-03-05"),
        (&["work_injury", "grading"], "LD-05-02"),
        (&["rest_leave", "holiday_ot"], "LD-09-04"),
        // 做能用 样本 + 最深路径.
        (&["social_insurance", "unpaid"], "LD-04-02"),
        (&["new_employment", "income"], "LD-13-04"),
        (&["work_injury", "occupational", "suspected"], "LD-05-06"),
        (&["livestream", "penalty"], "LD-20-05"),
    ];
    for (path, expect) in cases {
        let input = rule_engine::DiagnosisInput {
            identity_type: data_model::IdentityType::StandardFullTime,
            dispute_subtype: data_model::DisputeSubtype::SocialInsArrears,
            answer_path: path.iter().map(|s| s.to_string()).collect(),
            skipped_questions: 0,
        };
        match eng.diagnose(&input) {
            DiagnosisOutput::Ok(r) => assert_eq!(
                &r.dispute_category, expect,
                "path {path:?} should diagnose {expect}"
            ),
            other => panic!("path {path:?} expected Ok({expect}), got {other:?}"),
        }
    }
}

/// The diagnosis catalog's subcategory code set equals the kb `categories.yaml` code set (single
/// source of truth — the two files describe the same 20/85 taxonomy, data/01 §1.7).
#[test]
fn catalog_codes_match_kb_categories_yaml() {
    let catalog = DiagnosisCatalog::bundled().unwrap();
    let our_codes: BTreeSet<String> = catalog.entries().map(|e| e.code.clone()).collect();

    // Read the kb canonical categories.yaml relative to this crate.
    let kb_yaml = include_str!("../../kb/assets/categories.yaml");
    let kb_codes = extract_subcategory_codes(kb_yaml);

    assert_eq!(kb_codes.len(), 85, "kb categories.yaml must list 85 subcategories");
    assert_eq!(
        our_codes, kb_codes,
        "diagnosis catalog codes must match kb categories.yaml exactly"
    );
}

/// Extract every `LD-NN-NN` code from a categories.yaml document (lightweight scan; the kb crate
/// owns the strict parse — this test only needs the code set).
fn extract_subcategory_codes(yaml: &str) -> BTreeSet<String> {
    let mut codes = BTreeSet::new();
    for line in yaml.lines() {
        if let Some(idx) = line.find("code: LD-") {
            let rest = &line[idx + "code: ".len()..];
            let code: String = rest
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '-')
                .collect();
            // Only LD-NN-NN (8 chars), skip the LD-NN category codes (5 chars).
            if code.len() == 8 {
                codes.insert(code);
            }
        }
    }
    codes
}

/// abstention: an incomplete path never guesses (prd §4.1.5).
#[test]
fn incomplete_path_abstains() {
    let input = rule_engine::DiagnosisInput {
        identity_type: data_model::IdentityType::StandardFullTime,
        dispute_subtype: data_model::DisputeSubtype::SocialInsArrears,
        answer_path: vec!["work_injury".into()],
        skipped_questions: 0,
    };
    assert!(matches!(
        engine().diagnose(&input),
        DiagnosisOutput::OutOfScope { .. }
    ));
}
