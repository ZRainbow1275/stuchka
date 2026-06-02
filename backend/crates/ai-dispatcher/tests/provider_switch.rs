//! Provider primary/secondary switching + onboarding validation (ai/01 §1.9, brief acceptance #3).

use ai_dispatcher::{
    decide_switch, OnboardingChoice, OnboardingError, OnboardingSpec, OnboardingValidator,
    ProviderId, ProviderRegistry, SwitchDecision,
};

#[test]
fn first_failure_retries_then_fails_over() {
    let reg = ProviderRegistry::default_siliconflow();
    assert_eq!(
        decide_switch(&reg, true, 1, false, true),
        SwitchDecision::RetryPrimaryOnce
    );
    assert_eq!(
        decide_switch(&reg, true, 2, false, true),
        SwitchDecision::Failover {
            to: ProviderId::QwenCloud
        }
    );
}

#[test]
fn both_down_with_local_cools_down_to_local() {
    let reg = ProviderRegistry::default_siliconflow();
    assert_eq!(
        decide_switch(&reg, true, 2, true, true),
        SwitchDecision::CooldownThenLocal
    );
}

#[test]
fn both_down_without_local_is_rule_only() {
    let reg = ProviderRegistry::default_siliconflow();
    assert_eq!(
        decide_switch(&reg, true, 2, true, false),
        SwitchDecision::Level3RuleOnly
    );
}

#[test]
fn onboarding_rejects_identical_primary_secondary() {
    let spec = OnboardingSpec::default();
    let choice = OnboardingChoice {
        primary: ProviderId::DeepSeek,
        secondary: ProviderId::DeepSeek,
        overseas_enabled: false,
        overseas_signature: None,
        local_downloaded: false,
    };
    assert_eq!(
        OnboardingValidator::validate(&spec, &choice),
        Err(OnboardingError::SecondaryMustDiffer)
    );
}

#[test]
fn onboarding_accepts_distinct_cn_providers() {
    let spec = OnboardingSpec::default();
    let choice = OnboardingChoice {
        primary: ProviderId::DeepSeek,
        secondary: ProviderId::QwenCloud,
        overseas_enabled: false,
        overseas_signature: None,
        local_downloaded: false,
    };
    assert!(OnboardingValidator::validate(&spec, &choice).is_ok());
}

#[test]
fn onboarding_requires_overseas_signature() {
    let spec = OnboardingSpec::default();
    let choice = OnboardingChoice {
        primary: ProviderId::DeepSeek,
        secondary: ProviderId::QwenCloud,
        overseas_enabled: true,
        overseas_signature: None,
        local_downloaded: false,
    };
    assert_eq!(
        OnboardingValidator::validate(&spec, &choice),
        Err(OnboardingError::OverseasNeedsSignature)
    );
}
