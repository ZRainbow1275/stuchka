//! Routing decision: HSD hit / network state / model availability (ai/01 §1.5 Stage A + E).
//!
//! Pure functions deciding where a query may go given the HSD route hint, the current degrade
//! level, and local-model availability. The actual orchestration lives in [`crate::stage`].

use data_model::RouteHint;

use crate::levels::DegradeLevel;
use crate::provider::ProviderId;

/// Where Stage E is allowed to route a query.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteTarget {
    /// A cloud provider for the given level (primary at Level0, secondary at Level1).
    Cloud(ProviderId),
    /// The local small model (Level2, or HSD-forced-local).
    Local,
    /// No AI — Level3 rule-only abstention.
    RuleOnly,
    /// Level4 — KB stale, compensation refused.
    Level4Refuse,
    /// HSD forced local but no local model available — stop (INV-05).
    BlockedNoLocal,
}

/// Decide the Stage E route.
///
/// - HSD `ForceLocal` (or the user's `force_local`) pins to local; if no local model exists the
///   request is blocked (INV-05 — never send detection text to the cloud).
/// - Otherwise the route follows the degrade level: Level0 → primary, Level1 → secondary,
///   Level2 → local, Level3 → rule-only, Level4 → refuse.
pub fn decide_route(
    route_hint: RouteHint,
    user_force_local: bool,
    level: DegradeLevel,
    primary: ProviderId,
    secondary: ProviderId,
    local_available: bool,
) -> RouteTarget {
    let force_local = user_force_local || matches!(route_hint, RouteHint::ForceLocal);

    if force_local {
        return if local_available {
            RouteTarget::Local
        } else {
            RouteTarget::BlockedNoLocal
        };
    }

    match level {
        DegradeLevel::Level0 => RouteTarget::Cloud(primary),
        DegradeLevel::Level1 => RouteTarget::Cloud(secondary),
        DegradeLevel::Level2 => {
            if local_available {
                RouteTarget::Local
            } else {
                RouteTarget::RuleOnly
            }
        }
        DegradeLevel::Level3 => RouteTarget::RuleOnly,
        DegradeLevel::Level4 => RouteTarget::Level4Refuse,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn force_local_with_model_routes_local() {
        let t = decide_route(
            RouteHint::ForceLocal,
            false,
            DegradeLevel::Level0,
            ProviderId::DeepSeek,
            ProviderId::QwenCloud,
            true,
        );
        assert_eq!(t, RouteTarget::Local);
    }

    #[test]
    fn force_local_without_model_blocks() {
        let t = decide_route(
            RouteHint::ForceLocal,
            false,
            DegradeLevel::Level0,
            ProviderId::DeepSeek,
            ProviderId::QwenCloud,
            false,
        );
        assert_eq!(t, RouteTarget::BlockedNoLocal);
    }

    #[test]
    fn levels_route_to_expected_targets() {
        let p = ProviderId::DeepSeek;
        let s = ProviderId::QwenCloud;
        assert_eq!(
            decide_route(RouteHint::Auto, false, DegradeLevel::Level0, p, s, true),
            RouteTarget::Cloud(p)
        );
        assert_eq!(
            decide_route(RouteHint::Auto, false, DegradeLevel::Level1, p, s, true),
            RouteTarget::Cloud(s)
        );
        assert_eq!(
            decide_route(RouteHint::Auto, false, DegradeLevel::Level2, p, s, true),
            RouteTarget::Local
        );
        assert_eq!(
            decide_route(RouteHint::Auto, false, DegradeLevel::Level2, p, s, false),
            RouteTarget::RuleOnly
        );
        assert_eq!(
            decide_route(RouteHint::Auto, false, DegradeLevel::Level3, p, s, true),
            RouteTarget::RuleOnly
        );
        assert_eq!(
            decide_route(RouteHint::Auto, false, DegradeLevel::Level4, p, s, true),
            RouteTarget::Level4Refuse
        );
    }

    #[test]
    fn user_force_local_overrides_auto() {
        let t = decide_route(
            RouteHint::Auto,
            true,
            DegradeLevel::Level0,
            ProviderId::DeepSeek,
            ProviderId::QwenCloud,
            true,
        );
        assert_eq!(t, RouteTarget::Local);
    }
}
