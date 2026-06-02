//! First-run wizard schema placeholder (D1 · `crates/core`).
//!
//! The first-run wizard collects the master password (→ argon2 key derivation in
//! `crates/crypto`), the data directory, and FS-encryption acknowledgement before the main
//! store opens. The substantive steps land in the crypto / db subtasks; this module fixes the
//! step schema so the API layer and the Dart wizard share a stable contract.

use serde::{Deserialize, Serialize};

/// Ordered first-run wizard steps (schema only; logic in later subtasks).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FirstRunStep {
    /// Choose / confirm the data directory.
    DataDirectory,
    /// Set the master password (argon2-derived key, `crates/crypto`, INV-05).
    MasterPassword,
    /// Acknowledge filesystem-encryption status (INV-05 §5.2.1; E_FS_UNENCRYPTED gate).
    FsEncryptionAck,
    /// Pull the initial knowledge base and freeze its version hash (INV-04).
    KnowledgeBase,
    /// Wizard complete; main store may open.
    Done,
}

impl FirstRunStep {
    /// Steps in presentation order.
    pub const ORDER: [FirstRunStep; 5] = [
        FirstRunStep::DataDirectory,
        FirstRunStep::MasterPassword,
        FirstRunStep::FsEncryptionAck,
        FirstRunStep::KnowledgeBase,
        FirstRunStep::Done,
    ];

    /// The step that follows this one, or `None` at [`FirstRunStep::Done`].
    pub fn next(self) -> Option<FirstRunStep> {
        let idx = Self::ORDER.iter().position(|s| *s == self)?;
        Self::ORDER.get(idx + 1).copied()
    }
}

/// First-run wizard state (schema placeholder).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct FirstRunWizard {
    /// Step the user is currently on.
    pub current: FirstRunStep,
    /// Whether the wizard has been completed.
    pub completed: bool,
}

impl FirstRunWizard {
    /// A pending wizard positioned at the first step.
    pub fn pending() -> Self {
        Self {
            current: FirstRunStep::DataDirectory,
            completed: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_starts_at_data_directory() {
        let w = FirstRunWizard::pending();
        assert_eq!(w.current, FirstRunStep::DataDirectory);
        assert!(!w.completed);
    }

    #[test]
    fn steps_chain_to_done() {
        let mut step = FirstRunStep::DataDirectory;
        let mut count = 1;
        while let Some(next) = step.next() {
            step = next;
            count += 1;
        }
        assert_eq!(step, FirstRunStep::Done);
        assert_eq!(count, FirstRunStep::ORDER.len());
    }

    #[test]
    fn step_serializes_snake_case() {
        assert_eq!(
            serde_json::to_string(&FirstRunStep::FsEncryptionAck).unwrap(),
            "\"fs_encryption_ack\""
        );
    }
}
