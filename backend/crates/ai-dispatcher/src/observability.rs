//! Degrade visibility / explainability / recoverability (ai/01 §1.8, PRD §5.5.3).
//!
//! Every degrade transition produces a [`DegradeEvent`] the status bar renders as
//! `Level X · 原因 · 影响 N 模块 · 如何恢复`. Any degrade MUST be visible, explainable and
//! recoverable.

use serde::{Deserialize, Serialize};

use crate::levels::{module_matrix_for, DegradeLevel, ModuleId, ModuleState};

/// Why a degrade happened (ai/01 §1.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasonCode {
    /// Primary provider timed out / errored.
    PrimaryTimeout,
    /// Secondary provider rate-limited (429).
    Secondary429,
    /// KB older than 30 days.
    KbStale,
    /// Local model missing / unhealthy.
    LocalMissing,
    /// User forced a degrade (e.g. force-local).
    UserForce,
    /// Recovery (upward transition).
    Recovered,
}

impl ReasonCode {
    /// User-facing Chinese reason text.
    pub fn text_zh(self) -> &'static str {
        match self {
            ReasonCode::PrimaryTimeout => "主用 AI 服务超时或不可用",
            ReasonCode::Secondary429 => "备用 AI 服务被限流",
            ReasonCode::KbStale => "知识库已超过 30 天未更新",
            ReasonCode::LocalMissing => "本地小模型不可用",
            ReasonCode::UserForce => "用户主动选择本地处理",
            ReasonCode::Recovered => "服务已恢复",
        }
    }
}

/// How to recover from a degrade (ai/01 §1.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryHint {
    /// Restart / retry the provider.
    RetryProvider,
    /// Top up / renew the provider quota.
    RenewQuota,
    /// Restore network connectivity.
    Reconnect,
    /// Update the knowledge base.
    UpdateKb,
    /// Download the local model.
    DownloadLocal,
}

impl RecoveryHint {
    /// User-facing Chinese recovery text.
    pub fn text_zh(self) -> &'static str {
        match self {
            RecoveryHint::RetryProvider => "稍后重试主用 AI 服务",
            RecoveryHint::RenewQuota => "为 AI 服务续费或更换可用 provider",
            RecoveryHint::Reconnect => "恢复网络连接后重试",
            RecoveryHint::UpdateKb => "联网更新知识库后重试",
            RecoveryHint::DownloadLocal => "下载本地小模型以启用本地兜底",
        }
    }
}

/// A degrade-transition event (ai/01 §1.8). Surfaced to the status bar + audit trail.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DegradeEvent {
    /// When the transition occurred.
    pub ts: chrono::DateTime<chrono::Utc>,
    /// Source level.
    pub from: DegradeLevel,
    /// Destination level.
    pub to: DegradeLevel,
    /// Why.
    pub reason_code: ReasonCode,
    /// Chinese reason text.
    pub reason_text_zh: String,
    /// Modules affected (those not fully `AiCloud` / `Available` at the new level).
    pub affected_modules: Vec<ModuleId>,
    /// How to recover.
    pub recovery_hint: RecoveryHint,
}

impl DegradeEvent {
    /// Build a degrade event, deriving the affected-module list from the destination level matrix
    /// and a Chinese reason text from `reason_code`.
    pub fn new(
        from: DegradeLevel,
        to: DegradeLevel,
        reason_code: ReasonCode,
        recovery_hint: RecoveryHint,
    ) -> Self {
        let affected_modules = affected_modules(to);
        Self {
            ts: chrono::Utc::now(),
            from,
            to,
            reason_code,
            reason_text_zh: reason_code.text_zh().to_string(),
            affected_modules,
            recovery_hint,
        }
    }

    /// The permanent status-bar line: `Level X · 原因 · 影响 N 模块 · 如何恢复`.
    pub fn status_bar_line(&self) -> String {
        format!(
            "Level {} · {} · 影响 {} 个模块 · {}",
            self.to.as_u8(),
            self.reason_text_zh,
            self.affected_modules.len(),
            self.recovery_hint.text_zh()
        )
    }
}

/// The modules whose availability is reduced at `level` (not `AiCloud` / `Available` / `Rule`).
pub fn affected_modules(level: DegradeLevel) -> Vec<ModuleId> {
    module_matrix_for(level)
        .rows
        .into_iter()
        .filter(|(_, state)| {
            !matches!(
                state,
                ModuleState::AiCloud | ModuleState::Available | ModuleState::Rule
            )
        })
        .map(|(m, _)| m)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level0_has_no_affected_modules() {
        assert!(affected_modules(DegradeLevel::Level0).is_empty());
    }

    #[test]
    fn level4_affects_several_modules() {
        let affected = affected_modules(DegradeLevel::Level4);
        assert!(affected.contains(&ModuleId::M9Compute));
        assert!(affected.contains(&ModuleId::M3Document));
        assert!(!affected.is_empty());
    }

    #[test]
    fn status_bar_line_is_explainable_and_recoverable() {
        let ev = DegradeEvent::new(
            DegradeLevel::Level0,
            DegradeLevel::Level1,
            ReasonCode::PrimaryTimeout,
            RecoveryHint::RetryProvider,
        );
        let line = ev.status_bar_line();
        assert!(line.contains("Level 1"));
        assert!(line.contains("主用 AI 服务超时"));
        // The recovery hint text is present (explainable + recoverable).
        assert!(line.contains(RecoveryHint::RetryProvider.text_zh()));
        assert!(line.contains("影响"));
    }
}
