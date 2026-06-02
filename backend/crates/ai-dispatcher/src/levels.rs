//! Level 0-4 degrade FSM + module availability matrix (ai/01 §1.2).
//!
//! The machine ticks every 60s on a [`HealthSnapshot`] and moves to the highest Level the current
//! health permits (single-direction-down is NOT irreversible — recovery walks back up). The
//! transition rule is locked by spec and matched by priority:
//!
//! ```text
//! kb_age_days > 30                       -> Level4   (highest priority: KB stale)
//! primary                                -> Level0
//! !primary &  secondary                  -> Level1
//! !primary & !secondary &  local         -> Level2
//! !primary & !secondary & !local         -> Level3
//! ```
//!
//! **Level4 lock (C-C-5 + INV-04)**: once Level4 is reached the compensation engine abstains;
//! Stage E never falls back to Level0-3 AI inference. M5 (deadline) / M9 (compute) / M16 are pure
//! rules and stay available at every Level (they never degrade with the AI tiers).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// The five degrade levels (ai/01 §1.2). `Copy` so the FSM and confidence penalty can pass it by
/// value; serialises snake_case for the D1 Bearer-HTTP contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DegradeLevel {
    /// All cloud OK (primary + secondary + local all reachable).
    Level0,
    /// Primary down → secondary cloud.
    Level1,
    /// All cloud down → local Qwen2.5-7B-Q4, only 8/10 modules.
    Level2,
    /// No AI resource → only the M5 / M9 / M16 pure-rule modules.
    Level3,
    /// KB older than 30 days → strong warning + compensation calculation refused.
    Level4,
}

impl DegradeLevel {
    /// The 0-4 numeric used by the api `fallback_level: u8` DTO (backend/01 §1.10).
    pub fn as_u8(self) -> u8 {
        match self {
            DegradeLevel::Level0 => 0,
            DegradeLevel::Level1 => 1,
            DegradeLevel::Level2 => 2,
            DegradeLevel::Level3 => 3,
            DegradeLevel::Level4 => 4,
        }
    }
}

/// A point-in-time health reading driving [`LevelMachine::tick`] (ai/01 §1.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HealthSnapshot {
    /// Primary provider reachable.
    pub primary: bool,
    /// Secondary provider reachable.
    pub secondary: bool,
    /// Local model loaded + healthy.
    pub local: bool,
    /// Days since the active KB manifest was generated.
    pub kb_age_days: u32,
}

/// Per-module availability classification at a given Level (ai/01 §1.2 matrix).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleState {
    /// Full cloud AI.
    AiCloud,
    /// Cloud AI on the secondary provider.
    AiSecondary,
    /// Local small-model only.
    LocalOnly,
    /// Pure rule engine (never degrades with AI).
    Rule,
    /// Available (non-AI feature, e.g. M7 self).
    Available,
    /// Collection-only (no inference).
    CollectOnly,
    /// Template fill-only.
    TemplateOnly,
    /// Abstains (refuses to infer).
    Abstention,
    /// Refuses to generate (compensation under Level4).
    Refuse,
    /// Warning state.
    Warn,
    /// Disabled.
    Disabled,
}

/// The ten modules tracked in the availability matrix (ai/01 §1.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleId {
    /// M1 诊断.
    M1Diagnosis,
    /// M2 证据.
    M2Evidence,
    /// M3 文书.
    M3Document,
    /// M5 时效 (pure rule).
    M5Deadline,
    /// M7 自身 (the dispatcher).
    M7Dispatcher,
    /// M9 计算 (pure rule).
    M9Compute,
    /// M11 情绪.
    M11Emotion,
    /// M12 预警.
    M12Alert,
    /// M15 群体.
    M15Group,
    /// M16 执行 (pure rule).
    M16Execution,
}

/// The full Level×module availability matrix (ai/01 §1.2). Indexed by [`ModuleId`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleMatrix {
    /// (module, state) pairs for the current level, in the spec table order.
    pub rows: Vec<(ModuleId, ModuleState)>,
}

impl ModuleMatrix {
    /// Look up the availability state of a single module.
    pub fn state_of(&self, module: ModuleId) -> ModuleState {
        self.rows
            .iter()
            .find(|(m, _)| *m == module)
            .map(|(_, s)| *s)
            .expect("matrix is exhaustive over all ten modules")
    }
}

/// An audited FSM transition (INV-06). Persisted to the independent `audit.sqlite` by the caller.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransitionRecord {
    /// When the transition occurred (UTC, chrono per W2).
    pub at: DateTime<Utc>,
    /// Source level.
    pub from: DegradeLevel,
    /// Destination level.
    pub to: DegradeLevel,
    /// Health reading that triggered it.
    pub primary: bool,
    /// Secondary reachable at the time.
    pub secondary: bool,
    /// Local model healthy at the time.
    pub local: bool,
    /// KB age at the time.
    pub kb_age_days: u32,
}

/// The degrade-level state machine (ai/01 §1.2).
#[derive(Debug, Clone)]
pub struct LevelMachine {
    current: DegradeLevel,
    kb_age_days: u32,
    primary_ok: bool,
    secondary_ok: bool,
    local_ok: bool,
    last_transition: DateTime<Utc>,
    transitions: Vec<TransitionRecord>,
}

impl LevelMachine {
    /// Construct a machine pinned at `Level0` (all healthy) with no prior transitions.
    pub fn new() -> Self {
        Self {
            current: DegradeLevel::Level0,
            kb_age_days: 0,
            primary_ok: true,
            secondary_ok: true,
            local_ok: true,
            last_transition: Utc::now(),
            transitions: Vec::new(),
        }
    }

    /// Pure transition function — the level a health snapshot maps to (no side effects). Exposed for
    /// the 25-transition unit test so it can assert each (from × snapshot) pairing.
    pub fn resolve(health: HealthSnapshot) -> DegradeLevel {
        match (
            health.primary,
            health.secondary,
            health.local,
            health.kb_age_days,
        ) {
            // KB stale dominates everything (C-C-5 + INV-04).
            (_, _, _, days) if days > 30 => DegradeLevel::Level4,
            (true, _, _, _) => DegradeLevel::Level0,
            (false, true, _, _) => DegradeLevel::Level1,
            (false, false, true, _) => DegradeLevel::Level2,
            (false, false, false, _) => DegradeLevel::Level3,
        }
    }

    /// Drive the FSM once (every 60s). Computes the new level from `health`, records the transition
    /// when it differs from the current level (INV-06), and returns the resulting level.
    pub fn tick(&mut self, health: HealthSnapshot) -> DegradeLevel {
        let new = Self::resolve(health);
        self.primary_ok = health.primary;
        self.secondary_ok = health.secondary;
        self.local_ok = health.local;
        self.kb_age_days = health.kb_age_days;

        if new != self.current {
            let at = Utc::now();
            self.transitions.push(TransitionRecord {
                at,
                from: self.current,
                to: new,
                primary: health.primary,
                secondary: health.secondary,
                local: health.local,
                kb_age_days: health.kb_age_days,
            });
            self.current = new;
            self.last_transition = at;
        }
        self.current
    }

    /// The current level.
    pub fn current(&self) -> DegradeLevel {
        self.current
    }

    /// When the last transition happened.
    pub fn last_transition(&self) -> DateTime<Utc> {
        self.last_transition
    }

    /// The audited transition history (INV-06).
    pub fn transitions(&self) -> &[TransitionRecord] {
        &self.transitions
    }

    /// Build the per-module availability matrix for the current level (ai/01 §1.2 table).
    pub fn module_matrix(&self) -> ModuleMatrix {
        module_matrix_for(self.current)
    }
}

impl Default for LevelMachine {
    fn default() -> Self {
        Self::new()
    }
}

/// The availability matrix for a given level (ai/01 §1.2 table, transcribed exactly).
pub fn module_matrix_for(level: DegradeLevel) -> ModuleMatrix {
    use ModuleId::*;
    use ModuleState::*;
    let rows = match level {
        DegradeLevel::Level0 => vec![
            (M1Diagnosis, AiCloud),
            (M2Evidence, AiCloud),
            (M3Document, AiCloud),
            (M5Deadline, Rule),
            (M7Dispatcher, Available),
            (M9Compute, Rule),
            (M11Emotion, AiCloud),
            (M12Alert, Available),
            (M15Group, Available),
            (M16Execution, Available),
        ],
        DegradeLevel::Level1 => vec![
            (M1Diagnosis, AiSecondary),
            (M2Evidence, AiSecondary),
            (M3Document, AiSecondary),
            (M5Deadline, Rule),
            (M7Dispatcher, Available),
            (M9Compute, Rule),
            (M11Emotion, AiSecondary),
            (M12Alert, Available),
            (M15Group, Available),
            (M16Execution, Available),
        ],
        DegradeLevel::Level2 => vec![
            (M1Diagnosis, LocalOnly),
            (M2Evidence, LocalOnly),
            (M3Document, LocalOnly),
            (M5Deadline, Rule),
            (M7Dispatcher, Available),
            (M9Compute, Rule),
            (M11Emotion, Disabled),
            (M12Alert, Available),
            (M15Group, LocalOnly),
            (M16Execution, Available),
        ],
        DegradeLevel::Level3 => vec![
            (M1Diagnosis, Abstention),
            (M2Evidence, CollectOnly),
            (M3Document, TemplateOnly),
            (M5Deadline, Rule),
            (M7Dispatcher, Available),
            (M9Compute, Rule),
            (M11Emotion, Disabled),
            (M12Alert, Available),
            (M15Group, LocalOnly),
            (M16Execution, Available),
        ],
        DegradeLevel::Level4 => vec![
            (M1Diagnosis, Abstention),
            (M2Evidence, CollectOnly),
            (M3Document, Refuse),
            (M5Deadline, Rule),
            (M7Dispatcher, Warn),
            // Compensation is REFUSED at Level4 and never falls back (C-C-5 + INV-04).
            (M9Compute, Refuse),
            (M11Emotion, Disabled),
            (M12Alert, Warn),
            (M15Group, LocalOnly),
            (M16Execution, Available),
        ],
    };
    ModuleMatrix { rows }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pure_rule_modules_never_degrade() {
        for lvl in [
            DegradeLevel::Level0,
            DegradeLevel::Level1,
            DegradeLevel::Level2,
            DegradeLevel::Level3,
            DegradeLevel::Level4,
        ] {
            let m = module_matrix_for(lvl);
            assert_eq!(m.state_of(ModuleId::M5Deadline), ModuleState::Rule);
            assert_eq!(m.state_of(ModuleId::M16Execution), ModuleState::Available);
            // M9 is Rule everywhere EXCEPT Level4 where compensation is refused.
            let m9 = m.state_of(ModuleId::M9Compute);
            if lvl == DegradeLevel::Level4 {
                assert_eq!(m9, ModuleState::Refuse);
            } else {
                assert_eq!(m9, ModuleState::Rule);
            }
        }
    }

    #[test]
    fn level4_refuses_compensation() {
        let m = module_matrix_for(DegradeLevel::Level4);
        assert_eq!(m.state_of(ModuleId::M9Compute), ModuleState::Refuse);
    }

    #[test]
    fn as_u8_is_zero_to_four() {
        assert_eq!(DegradeLevel::Level0.as_u8(), 0);
        assert_eq!(DegradeLevel::Level4.as_u8(), 4);
    }
}
