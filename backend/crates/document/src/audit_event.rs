//! Export audit event (D3) — writes one `Export`-category record to the independent `audit.sqlite`
//! when a dossier export succeeds (compliance/01 §6.4 `audit::write_export_event`).
//!
//! Gated behind the `audit` feature. The audit append API is async (the audit crate runs on
//! tokio + sqlx); this module bridges the synchronous four-layer result into that write.

use audit::{AuditLog, AuditReason, Subject};
use data_model::AuditCategory;
use uuid::Uuid;

use crate::gb45438::metadata::DocMeta;
use crate::gb45438::LayerCompleteness;

/// Write the export audit event to the independent `audit.sqlite` (D3). The `why` is
/// `DocumentExported` whose category projects to [`AuditCategory::Export`]. `case_id` is required
/// by the audit schema for this reason, so the caller supplies the (plaintext) case id that scopes
/// the export. Returns the new record `seq`.
#[allow(clippy::too_many_arguments)]
pub async fn write_export_event(
    log: &AuditLog,
    who: Subject,
    case_id: Uuid,
    meta: &DocMeta,
    completeness: &LayerCompleteness,
    bundle_filename: &str,
) -> Result<i64, audit::AuditError> {
    debug_assert_eq!(
        AuditReason::DocumentExported.category(),
        AuditCategory::Export,
        "DocumentExported must project to the Export category"
    );
    let what = serde_json::json!({
        "case_id": case_id.to_string(),
        "doc_id": meta.doc_id.to_string(),
        "template_id": meta.template_id,
        "filename": bundle_filename,
        "kb_version": meta.kb_hash_short(),
        "segment_count": meta.segment_count,
        "gb45438_layer_completeness": {
            "layer1_explicit": completeness.layer1_explicit,
            "layer2_metadata": completeness.layer2_metadata,
            "layer3_zerowidth": completeness.layer3_zerowidth,
            "layer3_lsb": completeness.layer3_lsb,
            "layer4_manifest": completeness.layer4_manifest,
        },
    });
    log.append(who, AuditReason::DocumentExported, what).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::template::TemplateId;

    #[tokio::test]
    async fn export_event_lands_one_export_record_and_chain_verifies() {
        let key = [9u8; 32];
        let log = audit::open_in_memory(&key).await.unwrap();
        let case_id = Uuid::now_v7();
        let meta = DocMeta::sample(TemplateId::ArbApplication);
        let completeness = LayerCompleteness {
            layer1_explicit: true,
            layer2_metadata: true,
            layer3_zerowidth: true,
            layer3_lsb: true,
            layer4_manifest: true,
        };
        let who = Subject::System {
            component: "document".into(),
        };
        let seq = write_export_event(
            &log,
            who,
            case_id,
            &meta,
            &completeness,
            "labor_arbitration_application.pdf",
        )
        .await
        .unwrap();
        assert!(seq >= 1);

        let records = log.query_by_case(case_id).await.unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].why, AuditReason::DocumentExported);
        assert_eq!(records[0].why.category(), AuditCategory::Export);

        let v = log.verify_chain().await.unwrap();
        assert!(v.ok);
        assert_eq!(v.checked, 1);
    }
}
