//! 到期三级预警 — three-tier expiry warning (ai/06 §6.8).
//!
//! 纯规则部分（`tier_for`）R1a 可测；推送通道属 M12（R1b），本 crate 仅产出 `WarningTier`。

use serde::{Deserialize, Serialize};

/// 预警档位 — based on `buffered_remaining_days`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WarningTier {
    /// ≤ 10 天。
    T10Days,
    /// ≤ 3 天。
    T3Days,
    /// ≤ 1 天。
    T1Day,
}

/// Map the buffered remaining days onto a warning tier (`None` when > 10 days out).
pub fn tier_for(buffered_remaining_days: i64) -> Option<WarningTier> {
    match buffered_remaining_days {
        d if d <= 1 => Some(WarningTier::T1Day),
        d if d <= 3 => Some(WarningTier::T3Days),
        d if d <= 10 => Some(WarningTier::T10Days),
        _ => None,
    }
}

impl WarningTier {
    /// The on-wire snake_case token (matches the `serde` rename) for the warning feed (M12).
    pub fn as_str(self) -> &'static str {
        match self {
            WarningTier::T10Days => "t10_days",
            WarningTier::T3Days => "t3_days",
            WarningTier::T1Day => "t1_day",
        }
    }

    /// Short Chinese label for the desktop warning surface (M12).
    pub fn label_zh(self) -> &'static str {
        match self {
            WarningTier::T10Days => "10 天内到期",
            WarningTier::T3Days => "3 天内到期",
            WarningTier::T1Day => "1 天内到期",
        }
    }
}

/// Derive the warning tier for a computed [`DeadlineValue`] from its REAL buffered remaining days.
/// Returns `Some((tier, buffered_remaining_days))` when within the 10-day window, else `None`.
/// A wage-arrears no-limit window (`i64::MAX` while the labour relationship is active) never warns.
/// This is the M12 (R1b) bridge over the already-real M5 engine output — it fabricates nothing.
pub fn warning_for(value: &super::buffer::DeadlineValue) -> Option<(WarningTier, i64)> {
    let buffered = value.buffered_remaining_days;
    if buffered == i64::MAX {
        return None; // no-limit (wage active) — no countdown, no warning.
    }
    tier_for(buffered).map(|t| (t, buffered))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deadline::anchor::DeadlineKind;
    use crate::deadline::buffer::DeadlineValue;
    use crate::deadline::interrupt::DeadlineState;

    fn value(buffered: i64) -> DeadlineValue {
        DeadlineValue {
            kind: DeadlineKind::ArbitrationGeneral,
            raw_remaining_days: buffered,
            buffered_remaining_days: buffered,
            state: DeadlineState::Running {
                remaining_days: buffered,
            },
            manual_confirm_required: true,
            law_refs: vec![],
        }
    }

    #[test]
    fn warning_for_real_buffered_days() {
        assert_eq!(warning_for(&value(1)), Some((WarningTier::T1Day, 1)));
        assert_eq!(warning_for(&value(0)), Some((WarningTier::T1Day, 0)));
        assert_eq!(warning_for(&value(3)), Some((WarningTier::T3Days, 3)));
        assert_eq!(warning_for(&value(5)), Some((WarningTier::T10Days, 5)));
        assert_eq!(warning_for(&value(10)), Some((WarningTier::T10Days, 10)));
        assert_eq!(warning_for(&value(11)), None);
    }

    #[test]
    fn warning_for_no_limit_never_warns() {
        // Wage-arrears while the labour relationship is active → i64::MAX → no warning.
        assert_eq!(warning_for(&DeadlineValue::no_limit_while_active(
            DeadlineKind::ArbitrationWage,
            vec![]
        )), None);
    }

    #[test]
    fn tier_labels_match_wire() {
        assert_eq!(WarningTier::T1Day.as_str(), "t1_day");
        assert_eq!(WarningTier::T3Days.as_str(), "t3_days");
        assert_eq!(WarningTier::T10Days.as_str(), "t10_days");
    }
}
