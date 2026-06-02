//! Cloud second-confirm protocol — 6 scenarios e2e (ai/01 §1.9, brief acceptance #5):
//! 境内单次 / 境内记忆 / 境外签名 / 境外拒签 / 高敏阻断 / 取消.

use ai_dispatcher::{
    evaluate_confirm, AiPurpose, CloudConfirmRequest, CloudConfirmResponse, ConfirmOutcome,
    HsdHitSummary, Jurisdiction, ProviderId,
};
use uuid::Uuid;

fn cn_req() -> CloudConfirmRequest {
    CloudConfirmRequest::build(
        ProviderId::DeepSeek,
        "脱敏预览：关于经济补偿的咨询".into(),
        vec![],
        1200,
        AiPurpose::Diagnosis,
        Uuid::now_v7(),
    )
}

fn overseas_req() -> CloudConfirmRequest {
    CloudConfirmRequest::build(
        ProviderId::OpenAI,
        "脱敏预览".into(),
        vec![],
        1200,
        AiPurpose::Document,
        Uuid::now_v7(),
    )
}

fn hsd_req() -> CloudConfirmRequest {
    CloudConfirmRequest::build(
        ProviderId::DeepSeek,
        "脱敏预览".into(),
        vec![HsdHitSummary {
            kind: "id_card".into(),
            layer: "regex".into(),
        }],
        1200,
        AiPurpose::Diagnosis,
        Uuid::now_v7(),
    )
}

// 1. 境内单次确认 → proceed.
#[test]
fn scenario_cn_single_confirm() {
    let req = cn_req();
    assert_eq!(req.jurisdiction, Jurisdiction::Cn);
    assert!(!req.require_signature);
    let resp = CloudConfirmResponse::Accepted {
        ts: chrono::Utc::now(),
        signature: None,
    };
    assert_eq!(evaluate_confirm(&req, &resp), ConfirmOutcome::ProceedCloud);
}

// 2. 境内记忆 7 天可关闭.
#[test]
fn scenario_cn_memory_7d() {
    assert!(cn_req().memory_allowed(), "CN confirm may be remembered");
}

// 3. 境外签名 → proceed.
#[test]
fn scenario_overseas_signed() {
    let req = overseas_req();
    assert_eq!(req.jurisdiction, Jurisdiction::Overseas);
    assert!(req.require_signature);
    assert!(!req.memory_allowed(), "overseas never memorable (C-B-5)");
    let resp = CloudConfirmResponse::Accepted {
        ts: chrono::Utc::now(),
        signature: Some("我同意数据出境".into()),
    };
    assert_eq!(evaluate_confirm(&req, &resp), ConfirmOutcome::ProceedCloud);
}

// 4. 境外拒签（接受但未签名）→ rejected.
#[test]
fn scenario_overseas_unsigned() {
    let req = overseas_req();
    let resp = CloudConfirmResponse::Accepted {
        ts: chrono::Utc::now(),
        signature: None,
    };
    assert_eq!(
        evaluate_confirm(&req, &resp),
        ConfirmOutcome::SignatureMissing
    );
}

// 5. 高敏阻断 → 按钮禁用，强制本地.
#[test]
fn scenario_hsd_blocked() {
    let req = hsd_req();
    assert!(req.confirm_button_disabled());
    assert!(!req.memory_allowed());
    let resp = CloudConfirmResponse::DeclinedAndForceLocal;
    assert_eq!(evaluate_confirm(&req, &resp), ConfirmOutcome::ForceLocal);
}

// 6. 取消 → cancelled.
#[test]
fn scenario_cancel() {
    let req = cn_req();
    let resp = CloudConfirmResponse::Declined {
        reason: "用户取消".into(),
    };
    assert_eq!(evaluate_confirm(&req, &resp), ConfirmOutcome::Cancelled);
}
