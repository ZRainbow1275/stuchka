//! 中断 / 中止状态机 — interrupt (restart) + suspend (freeze) state machine (ai/06 §6.4).
//!
//! 法源：`law:中华人民共和国劳动争议调解仲裁法/v2007-12-29/§27`.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use super::anchor::DeadlineFacts;

/// 中断事由 — interruption cause; each interrupt restarts the clock (§27 ¶2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InterruptCause {
    /// 向对方当事人主张权利。
    AssertRightToParty,
    /// 对方当事人同意履行义务。
    PartyAgreesToPerform,
    /// 申请调解。
    ApplyMediation,
    /// 申请仲裁。
    ApplyArbitration,
}

/// 中止事由 — suspension cause; the clock freezes for the interval (§27 ¶3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SuspendCause {
    /// 不可抗力。
    ForceMajeure,
    /// 其他正当障碍。
    OtherJustifiedObstacle,
}

/// A dated interruption event. The latest one becomes the new clock start.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterruptEvent {
    pub at: NaiveDate,
    pub cause: InterruptCause,
}

/// A suspension interval. `to == None` means the obstacle persists → state freezes (`Suspended`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuspendInterval {
    pub from: NaiveDate,
    pub to: Option<NaiveDate>,
    pub cause: SuspendCause,
}

/// 时效状态 — limitation state machine terminal/intermediate value (ai/06 §6.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum DeadlineState {
    /// 计时中。
    Running { remaining_days: i64 },
    /// 中止：剩余天数冻结。
    Suspended { frozen_remaining_days: i64 },
    /// 已过期。
    Expired { overdue_days: i64 },
}

/// Compute the effective remaining days: restart from the latest interruption, deduct suspended
/// intervals; an open (unclosed) suspension interval forces `Suspended` (ai/06 §6.4).
pub fn effective_remaining(base_window_days: i64, facts: &DeadlineFacts) -> DeadlineState {
    // 1. 起算基准 = max(case_occurred_at, 最后一次中断事件日)。
    let start = facts
        .interrupt_events
        .iter()
        .map(|e| e.at)
        .max()
        .unwrap_or(facts.case_occurred_at);
    // 2. 累计中止天数（区间未闭合则到 as_of）。
    let suspended: i64 = facts
        .suspend_intervals
        .iter()
        .map(|s| (s.to.unwrap_or(facts.as_of) - s.from).num_days().max(0))
        .sum();
    // 3. 有效经过天数 = (as_of − start) − 中止天数。
    let elapsed = (facts.as_of - start).num_days() - suspended;
    let remaining = base_window_days - elapsed;
    if facts.suspend_intervals.iter().any(|s| s.to.is_none()) {
        DeadlineState::Suspended {
            frozen_remaining_days: remaining,
        }
    } else if remaining <= 0 {
        DeadlineState::Expired {
            overdue_days: -remaining,
        }
    } else {
        DeadlineState::Running {
            remaining_days: remaining,
        }
    }
}
