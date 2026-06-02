//! §4.8 — three-layer court-admissibility aggregation (C-C-15, AU-11).
//!
//! ```text
//! Layer 1 · local hash chain intact (verify_chain() passed)
//! Layer 2 · OpenTimestamps receipt verified (third-party timestamp, R1a)
//! Layer 3 · GitHub commit anchor verifiable from a public repo (when the user enabled it)
//! ```
//!
//! Missing any layer -> "user-is-their-own-auditor" admissibility is zero. The UI surfaces all
//! three states (Lucide ShieldCheck / Clock / AlertTriangle, never Emoji — I9). This module is the
//! pure aggregation function the UI and the `/audit` export consume.

use serde::{Deserialize, Serialize};

/// The three admissibility layers for a given day / export (§4.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct TrustState {
    /// Layer 1 — `verify_chain()` passed for the covered records.
    pub chain_ok: bool,
    /// Layer 2 — an OTS receipt is present and binds to the daily hash (R1a必死).
    pub ots_verified: bool,
    /// Layer 3 — a GitHub commit anchor is present and verifiable (opt-in; R1b for real verify).
    pub github_anchored: bool,
}

/// The aggregate admissibility verdict (§4.8): a UI-facing tri-state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Admissibility {
    /// All required layers satisfied (chain + OTS; GitHub is optional opt-in).
    Admissible,
    /// Chain is intact but external anchoring (Layer 2) is still pending.
    Pending,
    /// The chain itself is broken — admissibility is zero, guide the user to legal aid (C-C-15).
    Broken,
}

impl TrustState {
    /// Construct from the three layer booleans.
    #[must_use]
    pub fn new(chain_ok: bool, ots_verified: bool, github_anchored: bool) -> Self {
        Self {
            chain_ok,
            ots_verified,
            github_anchored,
        }
    }

    /// Aggregate verdict (§4.8). GitHub (Layer 3) is opt-in, so it raises confidence but its
    /// absence does not by itself block admissibility once OTS (Layer 2) is satisfied.
    #[must_use]
    pub fn verdict(&self) -> Admissibility {
        if !self.chain_ok {
            Admissibility::Broken
        } else if self.ots_verified {
            Admissibility::Admissible
        } else {
            Admissibility::Pending
        }
    }

    /// Court admissibility requires Layer 1 AND Layer 2 (§4.8). True only when both hold.
    #[must_use]
    pub fn is_court_admissible(&self) -> bool {
        self.chain_ok && self.ots_verified
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// AU-11: with no OTS anchor, Layer 2 is false and the export is not court-admissible.
    #[test]
    fn au_11_missing_ots_makes_layer2_false() {
        let t = TrustState::new(true, false, false);
        assert!(
            !t.ots_verified,
            "Layer 2 must be false without an OTS receipt"
        );
        assert!(!t.is_court_admissible());
        assert_eq!(t.verdict(), Admissibility::Pending);
    }

    #[test]
    fn full_chain_plus_ots_is_admissible() {
        let t = TrustState::new(true, true, false);
        assert!(t.is_court_admissible());
        assert_eq!(t.verdict(), Admissibility::Admissible);
    }

    #[test]
    fn broken_chain_is_zero_admissibility() {
        let t = TrustState::new(false, true, true);
        assert!(!t.is_court_admissible());
        assert_eq!(t.verdict(), Admissibility::Broken);
    }
}
