// Dart DTO mirror for the M1 diagnose contract (spec frontend/04 §4.8 + prd §4.1.2 output object).
//
// These align field-for-field with the backend `POST /diagnose` route (backend item A): the wire
// names are camelCase (backend `#[serde(rename_all = "camelCase")]`) and enum *values* are
// snake_case (reuse the enums.dart mirror). The deterministic DiagnosisEngine on the backend
// consumes a ≤7-question 问诊树 answer set and returns the structured DiagnosisOutput below — the
// LLM free-text path stays a secondary assist (see DiagnosePage), never the primary diagnosis.

import 'enums.dart';

/// One answer to a 问诊树 question (spec prd §4.1.4): the question id + the chosen option value.
/// Sent verbatim so the backend's deterministic engine can replay the same decision path.
class DiagnoseAnswer {
  const DiagnoseAnswer({required this.questionId, required this.optionId, this.skipped = false});

  final String questionId;
  final String optionId;
  final bool skipped;

  Map<String, dynamic> toJson() => {
        'questionId': questionId,
        'optionId': optionId,
        'skipped': skipped,
      };

  factory DiagnoseAnswer.fromJson(Map<String, dynamic> j) => DiagnoseAnswer(
        questionId: (j['questionId'] ?? '') as String,
        optionId: (j['optionId'] ?? '') as String,
        skipped: (j['skipped'] ?? false) as bool,
      );
}

/// DiagnoseReq (routes/diagnose.rs::DiagnoseReq). Inputs per prd §4.1.1:
/// identity分流 + 地区(province+city) + case_occurred_at + 问诊树答案 + KB hash.
class DiagnoseReq {
  DiagnoseReq({
    this.caseId,
    required this.identityType,
    required this.province,
    required this.city,
    required this.caseOccurredAt,
    required this.answers,
    required this.kbVersionHash,
    this.freeText,
  });

  final String? caseId;
  final IdentityType identityType;
  final String province;
  final String city;
  final String caseOccurredAt; // ISO yyyy-MM-dd
  final List<DiagnoseAnswer> answers;
  final String kbVersionHash;

  /// Optional secondary free-text the user typed (assist only — never the primary signal).
  final String? freeText;

  Map<String, dynamic> toJson() => {
        if (caseId != null) 'caseId': caseId,
        'identityType': identityType.wire,
        'province': province,
        'city': city,
        'caseOccurredAt': caseOccurredAt,
        'answers': answers.map((a) => a.toJson()).toList(),
        'kbVersionHash': kbVersionHash,
        if (freeText != null && freeText!.isNotEmpty) 'freeText': freeText,
      };
}

/// One recommended procedure (spec frontend/04 §4.8). Carries the procedure name, expected
/// duration, the仲裁/监察 time-limit note, the win-probability band, and the law-ref backing it.
class RecommendedProcedure {
  const RecommendedProcedure({
    required this.kind,
    required this.name,
    this.expectedDuration,
    this.deadlineNote,
    this.winProbabilityBand,
    this.lawRefId,
    this.documentTemplateId,
    this.note,
  });

  /// One of `arbitration` / `mediation` / `inspection` / `litigation` / `negotiation` / `enforcement`
  /// (free string — the backend names the procedure; the UI renders the label as-is).
  final String kind;
  final String name;
  final String? expectedDuration;
  final String? deadlineNote;
  final String? winProbabilityBand;
  final String? lawRefId;

  /// Template id the "一键生成对应文书" button loads (spec frontend/04 §4.8).
  final String? documentTemplateId;
  final String? note;

  factory RecommendedProcedure.fromJson(Map<String, dynamic> j) => RecommendedProcedure(
        kind: (j['kind'] ?? '') as String,
        name: (j['name'] ?? '') as String,
        expectedDuration: j['expectedDuration'] as String?,
        deadlineNote: j['deadlineNote'] as String?,
        winProbabilityBand: j['winProbabilityBand'] as String?,
        lawRefId: j['lawRefId'] as String?,
        documentTemplateId: j['documentTemplateId'] as String?,
        note: j['note'] as String?,
      );
}

/// recommended_procedures · 主路径 / 并行 / 备用 三档 (prd §4.1.2 / spec frontend/04 §4.8).
class RecommendedProcedures {
  const RecommendedProcedures({this.main, this.parallel, this.fallback});

  final RecommendedProcedure? main;
  final RecommendedProcedure? parallel;
  final RecommendedProcedure? fallback;

  bool get isEmpty => main == null && parallel == null && fallback == null;

  factory RecommendedProcedures.fromJson(Map<String, dynamic> j) => RecommendedProcedures(
        main: j['main'] == null
            ? null
            : RecommendedProcedure.fromJson((j['main'] as Map).cast<String, dynamic>()),
        parallel: j['parallel'] == null
            ? null
            : RecommendedProcedure.fromJson((j['parallel'] as Map).cast<String, dynamic>()),
        fallback: j['fallback'] == null
            ? null
            : RecommendedProcedure.fromJson((j['fallback'] as Map).cast<String, dynamic>()),
      );

  static const RecommendedProcedures empty = RecommendedProcedures();
}

/// One concrete next action (prd §4.1.2 next_actions — "不留'建议你思考'"). Carries an action label
/// and an optional route the UI can navigate to ("一键跳" affordance).
class DiagnoseNextAction {
  const DiagnoseNextAction({required this.label, this.route, this.detail});

  final String label;
  final String? route;
  final String? detail;

  factory DiagnoseNextAction.fromJson(Object? raw) {
    if (raw is String) return DiagnoseNextAction(label: raw);
    final j = (raw as Map).cast<String, dynamic>();
    return DiagnoseNextAction(
      label: (j['label'] ?? '') as String,
      route: j['route'] as String?,
      detail: j['detail'] as String?,
    );
  }
}

/// DiagnosisOutput (routes/diagnose.rs::DiagnoseResp) — the structured §4.1.2 output object.
class DiagnosisOutput {
  DiagnosisOutput({
    required this.identityType,
    this.disputeSubtype,
    this.disputeCategoryId,
    this.subcategoryLabel,
    required this.coverageTier,
    required this.confidence,
    required this.coverageTag,
    required this.recommendedProcedures,
    this.nextActions = const [],
    this.outOfScope = false,
    this.reasons = const [],
  });

  final IdentityType identityType;
  final DisputeSubtype? disputeSubtype;

  /// dispute_category · 20 大类 + 85 子类的子类 ID (prd §4.1.2).
  final String? disputeCategoryId;
  final String? subcategoryLabel;
  final CoverageTier coverageTier;
  final double confidence;
  final CoverageTag coverageTag;
  final RecommendedProcedures recommendedProcedures;
  final List<DiagnoseNextAction> nextActions;

  /// abstention (prd §4.1.5): coverage_tag = unknown → {status:"out_of_scope", reasons, next_actions}.
  final bool outOfScope;
  final List<String> reasons;

  factory DiagnosisOutput.fromJson(Map<String, dynamic> j) {
    final status = j['status'] as String?;
    final tag = CoverageTag.fromWire(j['coverageTag'] as String? ?? 'unknown');
    return DiagnosisOutput(
      identityType:
          IdentityType.fromWire(j['identityType'] as String? ?? 'standard_full_time'),
      disputeSubtype:
          j['disputeSubtype'] == null ? null : DisputeSubtype.fromWire(j['disputeSubtype'] as String),
      disputeCategoryId: j['disputeCategoryId'] as String?,
      subcategoryLabel: j['subcategoryLabel'] as String?,
      coverageTier: CoverageTier.fromWire(j['coverageTier'] as String? ?? 'make_usable'),
      confidence: (j['confidence'] as num?)?.toDouble() ?? 0,
      coverageTag: tag,
      recommendedProcedures: j['recommendedProcedures'] == null
          ? RecommendedProcedures.empty
          : RecommendedProcedures.fromJson(
              (j['recommendedProcedures'] as Map).cast<String, dynamic>()),
      nextActions: (j['nextActions'] as List?)
              ?.map(DiagnoseNextAction.fromJson)
              .toList() ??
          const [],
      outOfScope: status == 'out_of_scope' || tag == CoverageTag.unknown,
      reasons: (j['reasons'] as List?)?.map((e) => e as String).toList() ?? const [],
    );
  }
}
