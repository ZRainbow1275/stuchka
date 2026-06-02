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
