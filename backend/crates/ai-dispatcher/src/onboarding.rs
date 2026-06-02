//! First-run onboarding wizard schema + validator (ai/01 §1.6).
//!
//! The spec's enum-variant literals (`OverseasOptIn { default_off: true }`, `DownloadLocalQwen {
//! size_gb: 5.0 }`) are pseudo-code — Rust enum variants cannot carry default values — so they are
//! implemented as plain fields with defaults provided by [`OnboardingSpec::default`] (brief §6 I-4).
//!
//! Validation: primary/secondary are required and must differ (`SecondaryMustDiffer`); overseas
//! opt-in requires a signature (`OverseasNeedsSignature`).

use serde::{Deserialize, Serialize};

use crate::provider::{ProviderId, CN_PROVIDERS};

/// One onboarding step (ai/01 §1.6). Each maps to a Flutter card; the next card unlocks only when
/// the current required step passes [`OnboardingValidator::validate`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "step", rename_all = "snake_case")]
pub enum OnboardingStep {
    /// Pick the primary provider (one of the five CN ids).
    PickPrimary {
        /// Selectable provider ids.
        options: Vec<ProviderId>,
        /// Whether this step is mandatory.
        required: bool,
    },
    /// Pick the secondary provider (must differ from primary).
    PickSecondary {
        /// Selectable ids (primary excluded by the UI).
        options_excluding_primary: Vec<ProviderId>,
        /// Whether this step is mandatory.
        required: bool,
    },
    /// Opt in to overseas providers (default off; needs a signature).
    OverseasOptIn {
        /// Whether the toggle defaults to off (always true).
        default_off: bool,
        /// Markdown warning shown before enabling.
        warning_md: String,
        /// Whether enabling requires a typed signature.
        requires_signature: bool,
    },
    /// Optionally download the 5GB local small model.
    DownloadLocalQwen {
        /// Download size in GB.
        size_gb: f32,
        /// Whether the download is optional (always true).
        optional: bool,
        /// Mirror urls (GitHub Release + jsDelivr CDN).
        mirror_urls: Vec<String>,
    },
    /// Blocking initial KB sync (with expected SHA).
    KbInitialSync {
        /// Source url.
        source_url: String,
        /// Expected SHA-256.
        expected_sha: String,
        /// Whether the wizard blocks until sync completes (always true).
        blocking: bool,
    },
}

/// The full onboarding schema, pulled by the frontend over the D1 Bearer-HTTP channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardingSpec {
    /// Ordered steps.
    pub steps: Vec<OnboardingStep>,
}

impl Default for OnboardingSpec {
    fn default() -> Self {
        Self {
            steps: vec![
                OnboardingStep::PickPrimary {
                    options: CN_PROVIDERS.to_vec(),
                    required: true,
                },
                OnboardingStep::PickSecondary {
                    options_excluding_primary: CN_PROVIDERS.to_vec(),
                    required: true,
                },
                OnboardingStep::OverseasOptIn {
                    default_off: true,
                    warning_md:
                        "境外服务会将数据传输至中国境外，需逐次签名同意，且不可记忆。默认关闭。"
                            .to_string(),
                    requires_signature: true,
                },
                OnboardingStep::DownloadLocalQwen {
                    size_gb: 5.0,
                    optional: true,
                    mirror_urls: Vec::new(),
                },
                OnboardingStep::KbInitialSync {
                    source_url: String::new(),
                    expected_sha: String::new(),
                    blocking: true,
                },
            ],
        }
    }
}

/// The user's onboarding choices, validated by [`OnboardingValidator`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardingChoice {
    /// Chosen primary.
    pub primary: ProviderId,
    /// Chosen secondary.
    pub secondary: ProviderId,
    /// Whether overseas was enabled.
    pub overseas_enabled: bool,
    /// Typed signature when overseas is enabled.
    pub overseas_signature: Option<String>,
    /// Whether the local model was downloaded.
    pub local_downloaded: bool,
}

/// Onboarding validation errors (ai/01 §1.6).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OnboardingError {
    /// Secondary must differ from primary.
    #[error("备用 provider 必须与主 provider 不同")]
    SecondaryMustDiffer,
    /// Overseas enabled without a signature.
    #[error("开启境外服务需要文字签名同意")]
    OverseasNeedsSignature,
    /// Primary must be a CN-jurisdiction provider.
    #[error("主 provider 必须是境内服务")]
    PrimaryMustBeCn,
    /// Secondary must be a CN-jurisdiction provider.
    #[error("备用 provider 必须是境内服务")]
    SecondaryMustBeCn,
}

/// Stateless onboarding validator (ai/01 §1.6).
pub struct OnboardingValidator;

impl OnboardingValidator {
    /// Validate the user's choices against the schema (ai/01 §1.6).
    pub fn validate(
        _spec: &OnboardingSpec,
        choice: &OnboardingChoice,
    ) -> Result<(), OnboardingError> {
        if !choice.primary.is_cn_jurisdiction() {
            return Err(OnboardingError::PrimaryMustBeCn);
        }
        if !choice.secondary.is_cn_jurisdiction() {
            return Err(OnboardingError::SecondaryMustBeCn);
        }
        if choice.primary == choice.secondary {
            return Err(OnboardingError::SecondaryMustDiffer);
        }
        if choice.overseas_enabled
            && choice
                .overseas_signature
                .as_ref()
                .map(|s| s.trim().is_empty())
                .unwrap_or(true)
        {
            return Err(OnboardingError::OverseasNeedsSignature);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok_choice() -> OnboardingChoice {
        OnboardingChoice {
            primary: ProviderId::DeepSeek,
            secondary: ProviderId::QwenCloud,
            overseas_enabled: false,
            overseas_signature: None,
            local_downloaded: false,
        }
    }

    #[test]
    fn valid_choice_passes() {
        assert!(OnboardingValidator::validate(&OnboardingSpec::default(), &ok_choice()).is_ok());
    }

    #[test]
    fn same_provider_rejected() {
        let mut c = ok_choice();
        c.secondary = ProviderId::DeepSeek;
        assert_eq!(
            OnboardingValidator::validate(&OnboardingSpec::default(), &c),
            Err(OnboardingError::SecondaryMustDiffer)
        );
    }

    #[test]
    fn overseas_without_signature_rejected() {
        let mut c = ok_choice();
        c.overseas_enabled = true;
        c.overseas_signature = None;
        assert_eq!(
            OnboardingValidator::validate(&OnboardingSpec::default(), &c),
            Err(OnboardingError::OverseasNeedsSignature)
        );
    }

    #[test]
    fn overseas_with_signature_ok() {
        let mut c = ok_choice();
        c.overseas_enabled = true;
        c.overseas_signature = Some("我同意数据出境".to_string());
        assert!(OnboardingValidator::validate(&OnboardingSpec::default(), &c).is_ok());
    }

    #[test]
    fn default_spec_offers_five_cn_primary() {
        let spec = OnboardingSpec::default();
        if let OnboardingStep::PickPrimary { options, required } = &spec.steps[0] {
            assert_eq!(options.len(), 5);
            assert!(*required);
        } else {
            panic!("first step must be PickPrimary");
        }
    }
}
