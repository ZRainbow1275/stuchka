//! Shared test fixtures for db integration tests.
//!
//! Each integration-test binary compiles this module independently, so fixtures used only by
//! other test files look unused here; suppress the per-binary dead-code noise.
#![allow(dead_code)]

use chrono::{NaiveDate, Utc};
use data_model::enums::*;
use data_model::{Case, CaseFact, Claim, Evidence, Group, GroupContributor, LawRef};
use rust_decimal::Decimal;
use serde_json::json;
use std::str::FromStr;
use uuid::Uuid;

pub fn sample_case() -> Case {
    Case {
        id: Uuid::now_v7(),
        identity_type: IdentityType::StandardFullTime,
        dispute_subtype: DisputeSubtype::SocialInsWaiverInvalid,
        dispute_category: "LD-01-02".to_string(),
        coverage_tier: CoverageTier::MakeUsable,
        case_occurred_at: NaiveDate::from_ymd_opt(2026, 9, 1).unwrap(),
        province: "44".to_string(),
        city: "4401".to_string(),
        region_code: Some("440106".to_string()),
        kb_version_hash: "a".repeat(64),
        kb_version_label: "2026-05-12-r1".to_string(),
        status: CaseStatus::Draft,
        group_id: None,
        dialogue_template_id: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        frozen_at: None,
    }
}

pub fn sample_fact(case_id: Uuid) -> CaseFact {
    CaseFact {
        id: Uuid::now_v7(),
        case_id,
        content: "未签订书面劳动合同".to_string(),
        category: FactCategory::RelationQualification,
        status: FactStatus::Pending,
        source: FactSource::AiInferred,
        confidence: Some(0.82),
        coverage_tag: CoverageTag::Approximate,
        evidence_refs: vec![Uuid::now_v7(), Uuid::now_v7()],
        group_id: None,
        contributor_id: None,
        authorization_chain: Some(json!([{"from": "a", "to": "b"}])),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

pub fn sample_evidence(case_id: Uuid) -> Evidence {
    Evidence {
        id: Uuid::now_v7(),
        case_id,
        evidence_type: EvidenceCategory::AudioVideo,
        file_path: "evidence/abc.age".to_string(),
        file_hash: "b".repeat(64),
        mime_type: "audio/mpeg".to_string(),
        byte_size: 1024,
        effective_score: 0.75,
        score_breakdown: json!({
            "source": 0.8, "timing": 0.7, "completeness": 0.6,
            "relevance": 0.9, "authenticity": 0.8
        }),
        status: EvidenceStatus::Uploaded,
        high_sensitivity: true,
        collected_at: Some(Utc::now()),
        device_id: Some("dev-1".to_string()),
        gps_coords: Some(json!({"lat": 39.9, "lon": 116.4})),
        chain_membership: vec!["wage".to_string(), "overtime".to_string()],
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

pub fn sample_claim(case_id: Uuid, amount: &str) -> Claim {
    Claim {
        id: Uuid::now_v7(),
        case_id,
        claim_type: ClaimType::EconomicCompensation,
        amount_pre_tax: Some(Decimal::from_str(amount).unwrap()),
        amount_post_tax: None,
        calculation_breakdown: json!({"formula": "N+1", "base": "8000.00"}),
        status: ClaimStatus::Draft,
        law_refs: vec![Uuid::now_v7()],
        fact_refs: vec![Uuid::now_v7()],
        withdraw_inv10_confirmed: false,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

pub fn sample_law_ref() -> LawRef {
    LawRef {
        id: Uuid::now_v7(),
        stable_id: "law:中华人民共和国劳动合同法/v2012-12-28/§39/¶1".to_string(),
        content_hash: "c".repeat(64),
        kb_version_label: "2026-05-12-r1".to_string(),
        title: "中华人民共和国劳动合同法".to_string(),
        version_date: NaiveDate::from_ymd_opt(2012, 12, 28).unwrap(),
        article: "§39/¶1".to_string(),
        body_snapshot: "劳动者有下列情形之一的……".to_string(),
        created_at: Utc::now(),
    }
}

pub fn sample_group(case_id: Uuid) -> Group {
    Group {
        id: Uuid::now_v7(),
        case_id,
        initiator_id: Uuid::now_v7(),
        inv10_confirmed_at: Utc::now(),
        created_at: Utc::now(),
    }
}

pub fn sample_contributor(group_id: Uuid) -> GroupContributor {
    GroupContributor {
        group_id,
        contributor_id: Uuid::now_v7(),
        authorization_chain: json!([{"signer": "x"}]),
        joined_at: Utc::now(),
    }
}
