//! law-200 STRUCTURAL gate (ai/05 §5.2): every cited statute is a well-formed real D8 LawRef URN.
//!
//! IMPORTANT (honest scope): the spec's law-200 上线门槛 (法条准确率 >= 95%) is a HUMAN process gate
//! graded by 2 lawyers + 1 law lecturer blind (§5.2 / §5.9, ~2 person-months). It CANNOT be decided
//! automatically. What this gate verifies is the AUTOMATED structural proxy: every URN cited in the
//! seed corpus parses through the production D8 parser (`data_model::parse_law_ref`) — i.e. there are
//! no hallucinated / malformed law ids (must be 100%). The seed URNs are GENUINE statutes drawn from
//! the seeded KB corpus + the §0.5 parser exemplars + region tables; none are fabricated.

use serde::Deserialize;

use crate::common::read_json;
use crate::report::{CaseResult, GateReport};

#[derive(Debug, Deserialize)]
struct LawCase {
    id: String,
    urn: String,
    #[serde(default)]
    category: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LawDataset {
    cases: Vec<LawCase>,
}

pub fn run(dataset: &std::path::Path) -> Result<GateReport, String> {
    let data: LawDataset = read_json(dataset)?;
    let mut cases = Vec::with_capacity(data.cases.len());
    for case in &data.cases {
        let cat = case.category.as_deref().unwrap_or("law_ref");
        match data_model::parse_law_ref(&case.urn) {
            Ok(_) => cases.push(CaseResult::new(
                &case.id,
                true,
                format!("parses ({cat})"),
                "parsed",
            )),
            Err(e) => cases.push(CaseResult::new(
                &case.id,
                false,
                format!("parses ({cat})"),
                format!("parse error: {e}"),
            )),
        }
    }
    Ok(GateReport::from_cases("law", cases))
}
