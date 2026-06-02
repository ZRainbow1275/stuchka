//! 出境守卫入口 (`compliance/02` §3.1) — the data-export routing table, the唯一判定.
//!
//! `route(&request) -> RoutingDecision` implements the §3.1 priority matrix verbatim. Priority is
//! top-down with short-circuit: R0 (forced-local scene) → R1 (L4) → R2 (L3 + hsd) → R3 (L3) →
//! R4 (L2 domestic) → R5 (L2 overseas) → R6 (L1). The HSD hit (`crates/hsd`) and the §4.5 forced
//! scenes have the HIGHEST priority and override any user choice (INV-05). L4 is always local and
//! is NEVER sent to any cloud even masked (§7 禁止条款). This guard is the only place that may emit
//! `OverseasCloud`, and only with `requires_second_confirm = true` (§4.4).

use data_model::{grade_of, DataGrade};

use crate::routing::decision::{Route, RoutingDecision};

/// The maximum sensitivity grade across a payload's named fields (`compliance/02` §2.1). An
/// unregistered field is treated conservatively as L2 (一般个人信息) — never below — so an
/// unknown field can never silently slip a high-sensitivity payload to a cloud as L1.
pub fn max_grade_of_fields<'a>(fields: impl IntoIterator<Item = &'a str>) -> DataGrade {
    let mut max = DataGrade::L1;
    let mut any = false;
    for f in fields {
        any = true;
        let g = grade_of(f).unwrap_or(DataGrade::L2);
        if g > max {
            max = g;
        }
    }
    // An empty field set (free-text only) is conservatively L2: it may carry incidental PII the
    // hsd layer (passed separately as `hsd_hit`) is responsible for catching.
    if !any {
        return DataGrade::L2;
    }
    max
}

/// The §4.5 forced-local case scenes (medical leave / three periods / minor / sexual harassment /
/// work-injury appraisal material / criminal report / audio transcript). The case subtype label or
/// document kind is mapped to this flag by the caller; the guard treats `true` as the highest
/// priority (R0), overriding everything (INV-05).
#[derive(Debug, Clone, Copy, Default)]
pub struct ForcedLocalScene(pub bool);

/// The guard request (`compliance/02` §3.1 inputs).
#[derive(Debug, Clone)]
pub struct GuardRequest {
    /// Highest sensitivity grade across the payload (use [`max_grade_of_fields`]).
    pub input_max_grade: DataGrade,
    /// Whether the high-sensitivity detector hit (from `crates/hsd`).
    pub hsd_hit: bool,
    /// Whether a §4.5 forced-local scene applies.
    pub scene_forced_local: bool,
    /// Whether the user has enabled overseas cloud at all (settings, default false).
    pub overseas_enabled: bool,
    /// Whether the user chose overseas cloud for THIS request.
    pub overseas_chosen: bool,
    /// Whether a domestic-cloud desensitisation pass already ran clean (no residual PII). The L3
    /// domestic path (R3) requires this to be `true`; otherwise the guard forces local (§3.4).
    pub desensitization_clean: bool,
}

impl GuardRequest {
    /// The user's chosen channel, derived from the overseas flags (the audit `user_choice`).
    fn user_choice(&self) -> Route {
        if self.overseas_enabled && self.overseas_chosen {
            Route::OverseasCloud
        } else {
            Route::DomesticCloud
        }
    }
}

/// Apply the §3.1 routing matrix to `req` and return the [`RoutingDecision`] (`actual_route`).
///
/// Short-circuit priority (highest first), exactly the §3.1 table:
/// - R0: forced-local scene → local (override everything, `scene_forced_local`).
/// - R1: L4 → local (never any cloud, even masked).
/// - R2: L3 + hsd_hit → local (overrides user choice).
/// - R3: L3 (no hsd) → domestic cloud, desensitised; if desensitisation left residual PII, local.
/// - R4: L2, not overseas → domestic cloud (raw, L2 allowed domestic).
/// - R5: L2, overseas enabled + chosen → overseas cloud (needs second confirm + separate consent).
/// - R6: L1 → the user's current channel (overseas still needs the R5 confirm).
pub fn route(req: &GuardRequest) -> RoutingDecision {
    let user_choice = req.user_choice();
    let base = |actual_route: Route,
                override_reason: Option<&'static str>,
                desensitization_applied: bool,
                requires_second_confirm: bool,
                matched_rule: &'static str| RoutingDecision {
        actual_route,
        input_max_grade: req.input_max_grade,
        hsd_hit: req.hsd_hit,
        scene_forced_local: req.scene_forced_local,
        user_choice,
        override_reason,
        desensitization_applied,
        requires_second_confirm,
        matched_rule,
    };

    // R0 — forced-local scene (§4.5): override everything.
    if req.scene_forced_local {
        let reason = (user_choice != Route::LocalSmallSteelCannon)
            .then_some("scene_force_local");
        return base(Route::LocalSmallSteelCannon, reason, false, false, "R0");
    }

    // R1 — L4 核心隐私: always local; never any cloud (§7).
    if req.input_max_grade == DataGrade::L4 {
        let reason = (user_choice != Route::LocalSmallSteelCannon).then_some("l4_force_local");
        return base(Route::LocalSmallSteelCannon, reason, false, false, "R1");
    }

    // R2 — hsd hit: local, overriding the user's choice. The §3.1 table writes this row as "L3 +
    // hsd", but the §3.1 prose and §4.5 make the hsd hit (`crates/hsd` 高敏命中) the HIGHEST-priority
    // force-local signal that 覆盖一切 — so ANY hsd hit (whatever the structured-field grade) pins
    // local: a free-text payload graded L1/L2 by its fields can still carry incidental PII the hsd
    // layer caught, and that must never leave the device (INV-05). This is exactly the invariant
    // `RoutingDecision::assert_invariants` enforces (hsd_hit ⇒ local), so the two can never diverge.
    if req.hsd_hit {
        let reason = (user_choice != Route::LocalSmallSteelCannon)
            .then_some("high_sensitive_force_local");
        let rule = if req.input_max_grade == DataGrade::L3 {
            "R2"
        } else {
            "R2-hsd"
        };
        return base(Route::LocalSmallSteelCannon, reason, false, false, rule);
    }

    // R3 — L3 (no hsd): domestic cloud AFTER desensitisation; residual PII → force local (§3.4).
    if req.input_max_grade == DataGrade::L3 {
        if req.desensitization_clean {
            // The user may have wanted overseas; L3 may never go overseas → override to domestic.
            let reason =
                (user_choice == Route::OverseasCloud).then_some("l3_no_overseas_force_domestic");
            return base(Route::DomesticCloud, reason, true, false, "R3");
        } else {
            return base(
                Route::LocalSmallSteelCannon,
                Some("l3_residual_pii_force_local"),
                true,
                false,
                "R3-residual",
            );
        }
    }

    // R5 — L2 overseas (enabled + chosen): overseas cloud, second confirm required.
    if req.input_max_grade == DataGrade::L2 && req.overseas_enabled && req.overseas_chosen {
        return base(Route::OverseasCloud, None, false, true, "R5");
    }

    // R4 — L2 domestic (default).
    if req.input_max_grade == DataGrade::L2 {
        return base(Route::DomesticCloud, None, false, false, "R4");
    }

    // R6 — L1 公共 / 非个人: the user's current channel (overseas still confirmed per R5).
    match user_choice {
        Route::OverseasCloud => base(Route::OverseasCloud, None, false, true, "R6-overseas"),
        other => base(other, None, false, false, "R6"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req() -> GuardRequest {
        GuardRequest {
            input_max_grade: DataGrade::L2,
            hsd_hit: false,
            scene_forced_local: false,
            overseas_enabled: false,
            overseas_chosen: false,
            desensitization_clean: true,
        }
    }

    #[test]
    fn r0_forced_scene_overrides_overseas() {
        let mut r = req();
        r.scene_forced_local = true;
        r.overseas_enabled = true;
        r.overseas_chosen = true;
        let d = route(&r);
        assert_eq!(d.actual_route, Route::LocalSmallSteelCannon);
        assert!(d.overridden());
        assert_eq!(d.override_reason, Some("scene_force_local"));
        d.assert_invariants().unwrap();
    }

    #[test]
    fn r1_l4_always_local_never_cloud() {
        let mut r = req();
        r.input_max_grade = DataGrade::L4;
        r.overseas_enabled = true;
        r.overseas_chosen = true;
        let d = route(&r);
        assert_eq!(d.actual_route, Route::LocalSmallSteelCannon);
        assert_eq!(d.override_reason, Some("l4_force_local"));
        d.assert_invariants().unwrap();
    }

    #[test]
    fn r2_l3_with_hsd_forces_local_over_user_choice() {
        let mut r = req();
        r.input_max_grade = DataGrade::L3;
        r.hsd_hit = true;
        r.overseas_enabled = true;
        r.overseas_chosen = true;
        let d = route(&r);
        assert_eq!(d.actual_route, Route::LocalSmallSteelCannon);
        assert_eq!(d.override_reason, Some("high_sensitive_force_local"));
        d.assert_invariants().unwrap();
    }

    #[test]
    fn r3_l3_clean_desensitized_goes_domestic() {
        let mut r = req();
        r.input_max_grade = DataGrade::L3;
        r.desensitization_clean = true;
        let d = route(&r);
        assert_eq!(d.actual_route, Route::DomesticCloud);
        assert!(d.desensitization_applied);
        d.assert_invariants().unwrap();
    }

    #[test]
    fn r3_l3_residual_pii_forces_local() {
        let mut r = req();
        r.input_max_grade = DataGrade::L3;
        r.desensitization_clean = false;
        let d = route(&r);
        assert_eq!(d.actual_route, Route::LocalSmallSteelCannon);
        assert_eq!(d.override_reason, Some("l3_residual_pii_force_local"));
        d.assert_invariants().unwrap();
    }

    #[test]
    fn r3_l3_never_goes_overseas_even_if_chosen() {
        let mut r = req();
        r.input_max_grade = DataGrade::L3;
        r.overseas_enabled = true;
        r.overseas_chosen = true;
        r.desensitization_clean = true;
        let d = route(&r);
        assert_eq!(d.actual_route, Route::DomesticCloud);
        assert_eq!(d.override_reason, Some("l3_no_overseas_force_domestic"));
    }

    #[test]
    fn hsd_hit_forces_local_at_any_grade() {
        // §3.1 prose + §4.5: an hsd hit is the highest-priority force-local signal and overrides a
        // low structured-field grade (incidental PII in free text). Matches assert_invariants.
        for grade in [DataGrade::L1, DataGrade::L2] {
            let mut r = req();
            r.input_max_grade = grade;
            r.hsd_hit = true;
            r.overseas_enabled = true;
            r.overseas_chosen = true;
            let d = route(&r);
            assert_eq!(
                d.actual_route,
                Route::LocalSmallSteelCannon,
                "hsd hit must force local at {grade:?}"
            );
            assert_eq!(d.override_reason, Some("high_sensitive_force_local"));
            assert_eq!(d.matched_rule, "R2-hsd");
            d.assert_invariants().unwrap();
        }
    }

    #[test]
    fn r4_l2_domestic_default() {
        let d = route(&req());
        assert_eq!(d.actual_route, Route::DomesticCloud);
        assert!(!d.requires_second_confirm);
    }

    #[test]
    fn r5_l2_overseas_requires_second_confirm() {
        let mut r = req();
        r.overseas_enabled = true;
        r.overseas_chosen = true;
        let d = route(&r);
        assert_eq!(d.actual_route, Route::OverseasCloud);
        assert!(d.requires_second_confirm);
        d.assert_invariants().unwrap();
    }

    #[test]
    fn r6_l1_follows_user_channel() {
        let mut r = req();
        r.input_max_grade = DataGrade::L1;
        let d = route(&r);
        assert_eq!(d.actual_route, Route::DomesticCloud);
    }

    #[test]
    fn unregistered_field_is_at_least_l2() {
        assert_eq!(max_grade_of_fields(["totally_unknown_field"]), DataGrade::L2);
        assert_eq!(max_grade_of_fields(["id_card_number"]), DataGrade::L4);
        assert_eq!(
            max_grade_of_fields(["law_ref_id", "phone_number"]),
            DataGrade::L3
        );
        // empty (free text only) is conservatively L2.
        let empty: [&str; 0] = [];
        assert_eq!(max_grade_of_fields(empty), DataGrade::L2);
    }
}
