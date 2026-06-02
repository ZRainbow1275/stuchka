//! Level0-4 FSM coverage — 5×5 = 25 transition paths (ai/01 §1.9, brief acceptance #1).
//!
//! The FSM is health-driven, not from-state-driven: the destination level is a pure function of the
//! `HealthSnapshot`. We therefore enumerate, for every (from-level × representative snapshot) pair,
//! that `tick` lands on the spec-locked destination, exercising all 25 from→to combinations,
//! including the recover-upward path and the Level4 KB-stale override.

use ai_dispatcher::{DegradeLevel, HealthSnapshot, LevelMachine};

/// A snapshot that resolves to each target level.
fn snapshot_for(level: DegradeLevel) -> HealthSnapshot {
    match level {
        // primary ok, kb fresh → Level0
        DegradeLevel::Level0 => HealthSnapshot {
            primary: true,
            secondary: true,
            local: true,
            kb_age_days: 1,
        },
        // primary down, secondary up → Level1
        DegradeLevel::Level1 => HealthSnapshot {
            primary: false,
            secondary: true,
            local: true,
            kb_age_days: 1,
        },
        // primary+secondary down, local up → Level2
        DegradeLevel::Level2 => HealthSnapshot {
            primary: false,
            secondary: false,
            local: true,
            kb_age_days: 1,
        },
        // all down → Level3
        DegradeLevel::Level3 => HealthSnapshot {
            primary: false,
            secondary: false,
            local: false,
            kb_age_days: 1,
        },
        // kb stale (>30) → Level4 regardless of providers
        DegradeLevel::Level4 => HealthSnapshot {
            primary: true,
            secondary: true,
            local: true,
            kb_age_days: 31,
        },
    }
}

const ALL: [DegradeLevel; 5] = [
    DegradeLevel::Level0,
    DegradeLevel::Level1,
    DegradeLevel::Level2,
    DegradeLevel::Level3,
    DegradeLevel::Level4,
];

#[test]
fn all_25_transitions_land_on_spec_destination() {
    let mut count = 0;
    for from in ALL {
        for to in ALL {
            // Drive the machine into `from` first.
            let mut m = LevelMachine::new();
            m.tick(snapshot_for(from));
            assert_eq!(m.current(), from, "failed to enter {from:?}");

            // Now tick toward `to`.
            let landed = m.tick(snapshot_for(to));
            assert_eq!(
                landed, to,
                "transition {from:?} -> {to:?} landed on {landed:?}"
            );
            count += 1;
        }
    }
    assert_eq!(count, 25, "must cover 25 transition paths");
}

#[test]
fn transition_records_are_audited() {
    let mut m = LevelMachine::new(); // starts Level0
    m.tick(snapshot_for(DegradeLevel::Level1));
    m.tick(snapshot_for(DegradeLevel::Level3));
    m.tick(snapshot_for(DegradeLevel::Level0)); // recover upward
    let recs = m.transitions();
    assert_eq!(recs.len(), 3);
    assert_eq!(recs[0].from, DegradeLevel::Level0);
    assert_eq!(recs[0].to, DegradeLevel::Level1);
    assert_eq!(recs[2].to, DegradeLevel::Level0, "recover upward recorded");
}

#[test]
fn no_record_when_level_unchanged() {
    let mut m = LevelMachine::new();
    m.tick(snapshot_for(DegradeLevel::Level0)); // already Level0 → no transition
    assert!(m.transitions().is_empty());
}

#[test]
fn kb_stale_overrides_healthy_providers() {
    // Even with all providers healthy, kb_age > 30 forces Level4 (C-C-5 + INV-04).
    let mut m = LevelMachine::new();
    let snap = HealthSnapshot {
        primary: true,
        secondary: true,
        local: true,
        kb_age_days: 45,
    };
    assert_eq!(m.tick(snap), DegradeLevel::Level4);
}

#[test]
fn recover_upward_from_level3_to_level0() {
    let mut m = LevelMachine::new();
    m.tick(snapshot_for(DegradeLevel::Level3));
    assert_eq!(m.current(), DegradeLevel::Level3);
    // Providers come back online.
    let landed = m.tick(snapshot_for(DegradeLevel::Level0));
    assert_eq!(landed, DegradeLevel::Level0, "FSM recovers upward");
}

#[test]
fn boundary_30_days_is_not_yet_level4() {
    // The spec rule is `days > 30`; exactly 30 stays on the provider-driven level.
    let mut m = LevelMachine::new();
    let snap = HealthSnapshot {
        primary: true,
        secondary: true,
        local: true,
        kb_age_days: 30,
    };
    assert_eq!(m.tick(snap), DegradeLevel::Level0);
    let snap31 = HealthSnapshot {
        kb_age_days: 31,
        ..snap
    };
    assert_eq!(m.tick(snap31), DegradeLevel::Level4);
}
