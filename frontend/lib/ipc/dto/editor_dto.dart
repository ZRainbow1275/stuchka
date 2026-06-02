/// DTOs for the editor Step A->B relay (`POST /document/:id/steps`, 03-sync-yjs §3.5 / FE02 §2.11).
///
/// The wire contract mirrors the backend `EditorStepReq` / `StepAck`
/// (backend/crates/api/src/routes/document.rs, camelCase): a batch of ProseMirror steps, each
/// carrying its INV-06 `why` (edit | ai_accept | merge_resolve), double-written to the main store
/// `doc_step` table and the independent `audit.sqlite`.
library;

/// One ProseMirror step + its INV-06 reason.
class EditorStepDto {
  const EditorStepDto({required this.stepJson, required this.why, this.reason});

  /// Full ProseMirror Step JSON (opaque to Dart; forwarded verbatim).
  final Map<String, dynamic> stepJson;

  /// edit | ai_accept | merge_resolve (the audit four-tuple `why`, INV-06).
  final String why;

  /// Optional free-text decision reason.
  final String? reason;

  Map<String, dynamic> toJson() => {
        'stepJson': stepJson,
        'why': why,
        if (reason != null) 'reason': reason,
      };
}

/// `POST /document/:id/steps` request body (`EditorStepReq`).
class EditorStepBatch {
  const EditorStepBatch({
    required this.caseId,
    required this.actor,
    required this.clientId,
    required this.steps,
  });

  /// Owning case id (UUID v7) — required by the audit `what` schema.
  final String caseId;

  /// Acting user id (audit `who`).
  final String actor;

  /// Yjs client id.
  final String clientId;

  /// The steps to process in order.
  final List<EditorStepDto> steps;

  Map<String, dynamic> toJson() => {
        'caseId': caseId,
        'actor': actor,
        'clientId': clientId,
        'steps': steps.map((s) => s.toJson()).toList(),
      };
}

/// One processed step's outcome (step number + audit seq, INV-06 double-write).
class StepResultDto {
  const StepResultDto({required this.stepNo, required this.auditSeq});

  final int stepNo;
  final int auditSeq;

  factory StepResultDto.fromJson(Map<String, dynamic> json) => StepResultDto(
        stepNo: (json['stepNo'] as num).toInt(),
        auditSeq: (json['auditSeq'] as num).toInt(),
      );
}

/// `POST /document/:id/steps` ack (`StepAck`).
class StepAckDto {
  const StepAckDto({
    required this.docId,
    required this.stepNo,
    required this.auditSeq,
    required this.results,
  });

  final String docId;

  /// The last step number written (back-compat scalar).
  final int stepNo;

  /// The audit `seq` of the last appended `audit.sqlite` record (INV-06).
  final int auditSeq;

  /// One result per processed step (in order).
  final List<StepResultDto> results;

  factory StepAckDto.fromJson(Map<String, dynamic> json) => StepAckDto(
        docId: json['docId'] as String,
        stepNo: (json['stepNo'] as num).toInt(),
        auditSeq: (json['auditSeq'] as num).toInt(),
        results: ((json['results'] as List?) ?? const [])
            .map((e) => StepResultDto.fromJson((e as Map).cast<String, dynamic>()))
            .toList(),
      );
}
