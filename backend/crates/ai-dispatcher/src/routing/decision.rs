//! `RoutingDecision` — the data-export guard's verdict (`compliance/02` §3.1).
//!
//! The guard maps `(input_max_grade, hsd_hit, scene_forced_local, overseas_enabled,
//! overseas_chosen)` onto a [`Route`] per the §3.1 routing matrix and records the decision for the
//! audit four-tuple (§6). When the guard overrides the user's channel choice (INV-05) the
//! `override_reason` MUST be present (§7 禁止省略).

use data_model::DataGrade;
use serde::{Deserialize, Serialize};

/// The three R1 inference channels (`compliance/02` §1). The guard never invents a fourth.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Route {
    /// 本地小钢炮 — data never leaves the device (the forced-local sink, INV-05).
    LocalSmallSteelCannon,
    /// 境内云 — domestic cloud (data resident in China; L3 needs desensitisation first).
    DomesticCloud,
    /// 境外云 — overseas cloud (default disabled; needs separate consent + a fresh confirm).
    OverseasCloud,
}

impl Route {
    /// Stable `&'static str` (the audit JSON `actual_route`, `compliance/02` §6).
    pub fn as_str(self) -> &'static str {
        match self {
            Route::LocalSmallSteelCannon => "local_small_steel_cannon",
            Route::DomesticCloud => "domestic_cloud",
            Route::OverseasCloud => "overseas_cloud",
        }
    }

    /// Whether this route leaves the device (any cloud).
    pub fn is_cloud(self) -> bool {
        matches!(self, Route::DomesticCloud | Route::OverseasCloud)
    }
}

/// The guard's verdict (`compliance/02` §3.1 return struct). Carries everything the audit writer
/// needs for the four-tuple (§6) and everything the dispatcher needs to actually route.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutingDecision {
    /// The channel the guard decided on.
    pub actual_route: Route,
    /// Highest sensitivity grade across the payload's fields (L1..L4).
    pub input_max_grade: DataGrade,
    /// Whether the high-sensitivity detector (`crates/hsd`) hit on the payload.
    pub hsd_hit: bool,
    /// Whether a §4.5 forced-local scene applied (medical leave / three-periods / minor / ...).
    pub scene_forced_local: bool,
    /// The user's chosen channel (for the audit `user_choice` — may differ from `actual_route`).
    pub user_choice: Route,
    /// Override reason — REQUIRED when `actual_route != user_choice` (INV-05 / §7 禁止省略).
    pub override_reason: Option<&'static str>,
    /// Whether desensitisation was applied (domestic-cloud L3 path, §3.4).
    pub desensitization_applied: bool,
    /// Whether a second explicit confirm is required (overseas, §4.4).
    pub requires_second_confirm: bool,
    /// Which row of the §3.1 matrix fired (diagnostics / test anchor).
    pub matched_rule: &'static str,
}

impl RoutingDecision {
    /// True when the guard forced a route different from the user's choice (the case where
    /// `override_reason` MUST be set — checked by [`RoutingDecision::assert_invariants`]).
    pub fn overridden(&self) -> bool {
        self.actual_route != self.user_choice
    }

    /// Internal consistency invariant (INV-05 / §7): an overridden decision must carry a reason;
    /// an L4 / forced-local / hsd-hit decision must end at local. Returns the offending message if
    /// violated (the guard asserts this in tests so a future edit cannot silently drop it).
    pub fn assert_invariants(&self) -> Result<(), &'static str> {
        if self.overridden() && self.override_reason.is_none() {
            return Err("overridden routing decision must carry an override_reason (§7)");
        }
        if (self.input_max_grade == DataGrade::L4 || self.hsd_hit || self.scene_forced_local)
            && self.actual_route != Route::LocalSmallSteelCannon
        {
            return Err("L4 / hsd-hit / forced-local scene must route local (INV-05)");
        }
        if self.actual_route == Route::OverseasCloud && !self.requires_second_confirm {
            return Err("overseas route must require a second confirm (§4.4)");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_str_matches_audit_json() {
        assert_eq!(
            Route::LocalSmallSteelCannon.as_str(),
            "local_small_steel_cannon"
        );
        assert_eq!(Route::DomesticCloud.as_str(), "domestic_cloud");
        assert_eq!(Route::OverseasCloud.as_str(), "overseas_cloud");
    }

    #[test]
    fn invariants_catch_missing_override_reason() {
        let d = RoutingDecision {
            actual_route: Route::LocalSmallSteelCannon,
            input_max_grade: DataGrade::L4,
            hsd_hit: false,
            scene_forced_local: false,
            user_choice: Route::OverseasCloud,
            override_reason: None,
            desensitization_applied: false,
            requires_second_confirm: false,
            matched_rule: "R1",
        };
        assert!(d.assert_invariants().is_err());
    }
}
