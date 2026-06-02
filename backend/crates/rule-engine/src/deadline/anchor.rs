//! `case_occurred_at` 起算锚 + `DeadlineKind` (ai/06 §6.3).

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use super::interrupt::{InterruptEvent, SuspendInterval};
use crate::coverage::RuleLawRef;
use crate::error::RuleError;

/// All facts an M5 deadline computation may consume. `case_occurred_at` is the day-precise anchor
/// (知道或应当知道权利被侵害之日). Missing it → `OutOfScope { Unknown }` (no guessing, C-C-6).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeadlineFacts {
    pub case_occurred_at: NaiveDate,
    pub labor_relationship_active: bool,
    pub labor_relationship_ended_at: Option<NaiveDate>,
    #[serde(default)]
    pub interrupt_events: Vec<InterruptEvent>,
    #[serde(default)]
    pub suspend_intervals: Vec<SuspendInterval>,
    pub as_of: NaiveDate,
    /// 工伤认定结论作出日 — recognition-conclusion date. Required before the assessment phase may
    /// start (phase gate, ai/06 §6.5). Absent → assessment returns `OutOfScope { Boundary }`.
    #[serde(default)]
    pub recognition_conclusion_at: Option<NaiveDate>,
    /// 劳动能力鉴定结论作出日 — labor-capacity assessment-conclusion date. Phase 3 (工伤待遇核付,
    /// benefit payout) depends on this conclusion (ai/06 §6.5 阶段 3「依赖鉴定结论」). Absent →
    /// benefit payout returns `OutOfScope { Boundary }` (phase gate, no guessing, C-C-6).
    #[serde(default)]
    pub assessment_conclusion_at: Option<NaiveDate>,
}

impl DeadlineFacts {
    /// Re-anchor the clock at `anchor` (used by wage-arrears: count from the termination date).
    pub fn with_anchor(&self, anchor: NaiveDate) -> DeadlineFacts {
        DeadlineFacts {
            case_occurred_at: anchor,
            ..self.clone()
        }
    }
}

/// 时效类型 — deadline kind (ai/06 §6.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeadlineKind {
    /// 仲裁一般时效 1 年。
    ArbitrationGeneral,
    /// 拖欠工资特殊时效。
    ArbitrationWage,
    /// 劳动监察投诉 2 年。
    Inspection,
    /// 一审上诉 15 日。
    AppealFirstInstance,
    /// 二审上诉 15 日。
    AppealSecondInstance,
    /// 执行申请 2 年。
    Enforcement,
    /// 工伤认定 1 年。
    InjuryRecognition,
    /// 劳动能力鉴定（地方规定）。
    InjuryAssessment,
    /// 工伤待遇核付。
    InjuryBenefitPayout,
    /// 职业病诊断。
    OccupationalDisease,
}

impl DeadlineKind {
    /// The §0.5 URN law references for this deadline kind (D8).
    pub fn law_refs(self) -> Result<Vec<RuleLawRef>, RuleError> {
        let urns: &[&str] = match self {
            DeadlineKind::ArbitrationGeneral | DeadlineKind::ArbitrationWage => {
                &["law:中华人民共和国劳动争议调解仲裁法/v2007-12-29/§27"]
            }
            DeadlineKind::Inspection => &["law:劳动保障监察条例/v2004-12-01/§20"],
            DeadlineKind::AppealFirstInstance | DeadlineKind::AppealSecondInstance => {
                &["law:中华人民共和国民事诉讼法/v2023-09-01/§171"]
            }
            DeadlineKind::Enforcement => &["law:中华人民共和国民事诉讼法/v2023-09-01/§250"],
            DeadlineKind::InjuryRecognition => &["law:工伤保险条例/v2010-12-20/§17"],
            DeadlineKind::InjuryAssessment | DeadlineKind::InjuryBenefitPayout => {
                &["law:工伤保险条例/v2010-12-20/§25"]
            }
            DeadlineKind::OccupationalDisease => {
                &["law:中华人民共和国职业病防治法/v2018-12-29/§55"]
            }
        };
        RuleLawRef::parse_all(urns)
    }
}
