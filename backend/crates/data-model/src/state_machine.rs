//! State-machine transition validation (data/01 §1.1.3 / §1.2.3 / §1.5.3).
//!
//! Whitelist transitions only; illegal moves return `Err(StateTransitionError)`.
//! These mirror the SQL trigger whitelists in data/01 so the application layer and DB
//! agree (the DDL triggers are authoritative, these guards are the in-process mirror).

use crate::enums::{CaseStatus, ClaimStatus, EvidenceStatus, FactStatus};

/// Illegal state transition (data/01 §1.1.3 / §1.2.3 / §1.3.4 / §1.5.3).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StateTransitionError {
    #[error("illegal case status transition: {from:?} -> {to:?}")]
    Case { from: CaseStatus, to: CaseStatus },
    #[error("case is frozen (terminal); modification requires re-creation")]
    CaseFrozenTerminal,
    #[error("illegal fact status transition: {from:?} -> {to:?}")]
    Fact { from: FactStatus, to: FactStatus },
    #[error("illegal evidence status transition: {from:?} -> {to:?}")]
    Evidence {
        from: EvidenceStatus,
        to: EvidenceStatus,
    },
    #[error("illegal claim status transition: {from:?} -> {to:?}")]
    Claim { from: ClaimStatus, to: ClaimStatus },
    #[error("claim withdrawal requires withdraw_inv10_confirmed = true (INV-10)")]
    ClaimWithdrawNotConfirmed,
}

/// Validate a `case` status transition (data/01 §1.1.3 whitelist).
///
/// Whitelist: `draft→diagnosed`, `diagnosed→confirmed`, `diagnosed→disputed`,
/// `disputed→draft`, `confirmed→frozen`. `frozen` is terminal. Same-state is a no-op (Ok).
pub fn validate_case_transition(
    from: CaseStatus,
    to: CaseStatus,
) -> Result<(), StateTransitionError> {
    use CaseStatus::*;
    if from == to {
        return Ok(());
    }
    if from == Frozen {
        return Err(StateTransitionError::CaseFrozenTerminal);
    }
    let allowed = matches!(
        (from, to),
        (Draft, Diagnosed)
            | (Diagnosed, Confirmed)
            | (Diagnosed, Disputed)
            | (Disputed, Draft)
            | (Confirmed, Frozen)
    );
    if allowed {
        Ok(())
    } else {
        Err(StateTransitionError::Case { from, to })
    }
}

/// Validate a `fact` status transition (data/01 §1.2.3 whitelist; backend/02 §2.3).
///
/// Whitelist: `pending→confirmed`, `pending→disputed`, `confirmed→disputed`,
/// `confirmed→deprecated`, `disputed→deprecated`. The last edge adopts backend/02 §2.3
/// (`confirmed|disputed→deprecated`) so new evidence can retire a disputed fact. Same-state is a
/// no-op (Ok).
pub fn validate_fact_transition(
    from: FactStatus,
    to: FactStatus,
) -> Result<(), StateTransitionError> {
    use FactStatus::*;
    if from == to {
        return Ok(());
    }
    let allowed = matches!(
        (from, to),
        (Pending, Confirmed)
            | (Pending, Disputed)
            | (Confirmed, Disputed)
            | (Confirmed, Deprecated)
            | (Disputed, Deprecated)
    );
    if allowed {
        Ok(())
    } else {
        Err(StateTransitionError::Fact { from, to })
    }
}

/// Validate an `evidence` status transition (data/01 §1.3.4 whitelist).
///
/// Whitelist: `uploaded→parsed`, `parsed→scored`, `scored→verified`,
/// `scored→disputed`, `disputed→quarantined`. Same-state is a no-op (Ok).
pub fn validate_evidence_transition(
    from: EvidenceStatus,
    to: EvidenceStatus,
) -> Result<(), StateTransitionError> {
    use EvidenceStatus::*;
    if from == to {
        return Ok(());
    }
    let allowed = matches!(
        (from, to),
        (Uploaded, Parsed)
            | (Parsed, Scored)
            | (Scored, Verified)
            | (Scored, Disputed)
            | (Disputed, Quarantined)
    );
    if allowed {
        Ok(())
    } else {
        Err(StateTransitionError::Evidence { from, to })
    }
}

/// Validate a `claim` status transition (data/01 §1.5.3 whitelist).
///
/// Whitelist: `draft→finalized`, `finalized→granted`, `finalized→denied`,
/// `finalized→withdrawn`. Withdrawal additionally requires `withdraw_inv10_confirmed = true`
/// (INV-10, data/01 §1.5.3). Same-state is a no-op (Ok).
pub fn validate_claim_transition(
    from: ClaimStatus,
    to: ClaimStatus,
    withdraw_inv10_confirmed: bool,
) -> Result<(), StateTransitionError> {
    use ClaimStatus::*;
    if from == to {
        return Ok(());
    }
    let allowed = matches!(
        (from, to),
        (Draft, Finalized) | (Finalized, Granted) | (Finalized, Denied) | (Finalized, Withdrawn)
    );
    if !allowed {
        return Err(StateTransitionError::Claim { from, to });
    }
    if to == Withdrawn && !withdraw_inv10_confirmed {
        return Err(StateTransitionError::ClaimWithdrawNotConfirmed);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn case_accepts_whitelisted_transitions() {
        use CaseStatus::*;
        for (f, t) in [
            (Draft, Diagnosed),
            (Diagnosed, Confirmed),
            (Diagnosed, Disputed),
            (Disputed, Draft),
            (Confirmed, Frozen),
        ] {
            assert!(
                validate_case_transition(f, t).is_ok(),
                "{f:?}->{t:?} should be allowed"
            );
        }
    }

    #[test]
    fn case_rejects_illegal_transitions() {
        use CaseStatus::*;
        assert_eq!(
            validate_case_transition(Draft, Confirmed),
            Err(StateTransitionError::Case {
                from: Draft,
                to: Confirmed
            })
        );
        assert!(validate_case_transition(Draft, Frozen).is_err());
        assert!(validate_case_transition(Diagnosed, Frozen).is_err());
    }

    /// OM-04: frozen is terminal; any move out is rejected, and frozen_at must be set
    /// (the latter half is enforced by the DDL `frozen_consistent` CHECK).
    #[test]
    fn case_frozen_is_terminal() {
        use CaseStatus::*;
        for t in [Draft, Diagnosed, Confirmed, Disputed] {
            assert_eq!(
                validate_case_transition(Frozen, t),
                Err(StateTransitionError::CaseFrozenTerminal),
                "frozen -> {t:?} must be rejected as terminal"
            );
        }
        // same-state no-op is allowed
        assert!(validate_case_transition(Frozen, Frozen).is_ok());
    }

    #[test]
    fn fact_whitelist_and_rejections() {
        use FactStatus::*;
        for (f, t) in [
            (Pending, Confirmed),
            (Pending, Disputed),
            (Confirmed, Disputed),
            (Confirmed, Deprecated),
            // backend/02 §2.3: disputed → deprecated (new evidence retires a disputed fact).
            (Disputed, Deprecated),
        ] {
            assert!(validate_fact_transition(f, t).is_ok(), "{f:?}->{t:?}");
        }
        assert!(validate_fact_transition(Pending, Deprecated).is_err());
        assert!(validate_fact_transition(Deprecated, Confirmed).is_err());
        // disputed must not be able to go back to confirmed, only forward to deprecated.
        assert!(validate_fact_transition(Disputed, Confirmed).is_err());
    }

    /// Regression: disputed → deprecated must be accepted (backend/02 §2.3 new-evidence workflow).
    #[test]
    fn fact_disputed_to_deprecated_allowed() {
        use FactStatus::*;
        assert!(validate_fact_transition(Disputed, Deprecated).is_ok());
    }

    #[test]
    fn evidence_whitelist_and_rejections() {
        use EvidenceStatus::*;
        for (f, t) in [
            (Uploaded, Parsed),
            (Parsed, Scored),
            (Scored, Verified),
            (Scored, Disputed),
            (Disputed, Quarantined),
        ] {
            assert!(validate_evidence_transition(f, t).is_ok(), "{f:?}->{t:?}");
        }
        assert!(validate_evidence_transition(Uploaded, Scored).is_err());
        assert!(validate_evidence_transition(Verified, Quarantined).is_err());
    }

    #[test]
    fn claim_whitelist_and_rejections() {
        use ClaimStatus::*;
        assert!(validate_claim_transition(Draft, Finalized, false).is_ok());
        assert!(validate_claim_transition(Finalized, Granted, false).is_ok());
        assert!(validate_claim_transition(Finalized, Denied, false).is_ok());
        // illegal jumps
        assert!(validate_claim_transition(Draft, Granted, false).is_err());
        assert!(validate_claim_transition(Granted, Withdrawn, true).is_err());
    }

    /// OM-06: withdrawn requires withdraw_inv10_confirmed = true.
    #[test]
    fn claim_withdraw_requires_inv10() {
        use ClaimStatus::*;
        assert_eq!(
            validate_claim_transition(Finalized, Withdrawn, false),
            Err(StateTransitionError::ClaimWithdrawNotConfirmed)
        );
        assert!(validate_claim_transition(Finalized, Withdrawn, true).is_ok());
    }
}
