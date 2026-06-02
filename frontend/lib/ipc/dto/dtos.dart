// Dart DTO mirrors for the backend api route contracts.
//
// Wire field names are camelCase (backend `#[serde(rename_all = "camelCase")]`); enum *values*
// are snake_case (handled by the enums.dart mirror). Manual fromJson/toJson keeps the build free
// of codegen fragility while staying byte-accurate against
// Stučka/backend/crates/api/src/routes/*.rs.

import 'enums.dart';

/// Uniform IPC response envelope (data-model error.rs::ApiEnvelope).
/// Success → `{ data, error: null, traceId }`; failure → `{ data: null, error, traceId }`.
class ApiEnvelope<T> {
  ApiEnvelope({this.data, this.error, required this.traceId});

  final T? data;
  final ApiError? error;
  final String traceId;

  bool get isOk => error == null;

  static ApiEnvelope<T> fromJson<T>(
    Map<String, dynamic> json,
    T Function(Object? data) parseData,
  ) {
    final err = json['error'];
    return ApiEnvelope<T>(
      data: json['data'] == null ? null : parseData(json['data']),
      error: err == null ? null : ApiError.fromJson(err as Map<String, dynamic>),
      traceId: (json['traceId'] ?? '') as String,
    );
  }
}

/// Structured API error (data-model error.rs::ApiError) with the §1.12 code string.
class ApiError {
  ApiError({required this.code, required this.message, this.detail, this.hint});

  final String code;
  final String message;
  final Object? detail;
  final String? hint;

  factory ApiError.fromJson(Map<String, dynamic> json) => ApiError(
        code: (json['code'] ?? 'E_INTERNAL') as String,
        message: (json['message'] ?? '') as String,
        detail: json['detail'],
        hint: json['hint'] as String?,
      );
}

/// CaseDto (routes/case.rs::CaseDto).
class CaseDto {
  CaseDto({
    required this.id,
    required this.status,
    required this.identityType,
    required this.province,
    required this.city,
    this.regionCode,
    required this.caseOccurredAt,
    required this.disputeSubtype,
    this.disputeCategoryId,
    this.coverageTier,
    this.coverageTag,
    required this.kbVersionHash,
    this.createdAt,
    this.updatedAt,
  });

  final String id;
  final CaseStatus status;
  final IdentityType identityType;
  final String province;
  final String city;
  final String? regionCode;
  final String caseOccurredAt; // ISO yyyy-MM-dd
  final DisputeSubtype disputeSubtype;
  final String? disputeCategoryId;
  final CoverageTier? coverageTier;
  final CoverageTag? coverageTag;
  final String kbVersionHash;
  final DateTime? createdAt;
  final DateTime? updatedAt;

  String get shortLabel => '$province$city · ${disputeSubtype.labelZh}';

  factory CaseDto.fromJson(Map<String, dynamic> j) => CaseDto(
        id: j['id'] as String,
        status: CaseStatus.fromWire(j['status'] as String? ?? 'draft'),
        identityType: IdentityType.fromWire(j['identityType'] as String? ?? 'standard_full_time'),
        province: (j['province'] ?? '') as String,
        city: (j['city'] ?? '') as String,
        regionCode: j['regionCode'] as String?,
        caseOccurredAt: (j['caseOccurredAt'] ?? '') as String,
        disputeSubtype:
            DisputeSubtype.fromWire(j['disputeSubtype'] as String? ?? 'social_ins_arrears'),
        disputeCategoryId: j['disputeCategoryId'] as String?,
        coverageTier: j['coverageTier'] == null
            ? null
            : CoverageTier.fromWire(j['coverageTier'] as String),
        coverageTag:
            j['coverageTag'] == null ? null : CoverageTag.fromWire(j['coverageTag'] as String),
        kbVersionHash: (j['kbVersionHash'] ?? '') as String,
        createdAt: _dt(j['createdAt']),
        updatedAt: _dt(j['updatedAt']),
      );
}

/// CreateCaseReq (routes/case.rs::CreateCaseReq).
class CreateCaseReq {
  CreateCaseReq({
    required this.identityType,
    required this.province,
    required this.city,
    this.regionCode,
    required this.caseOccurredAt,
    required this.disputeSubtype,
    required this.firstDescription,
    required this.kbVersionHash,
  });

  final IdentityType identityType;
  final String province;
  final String city;
  final String? regionCode;
  final String caseOccurredAt;
  final DisputeSubtype disputeSubtype;
  final String firstDescription;
  final String kbVersionHash;

  Map<String, dynamic> toJson() => {
        'identityType': identityType.wire,
        'province': province,
        'city': city,
        if (regionCode != null) 'regionCode': regionCode,
        'caseOccurredAt': caseOccurredAt,
        'disputeSubtype': disputeSubtype.wire,
        'firstDescription': firstDescription,
        'kbVersionHash': kbVersionHash,
      };
}

/// FactDto (routes/fact.rs::FactDto).
class FactDto {
  FactDto({
    required this.id,
    required this.caseId,
    required this.content,
    required this.category,
    required this.status,
    required this.source,
    this.confidence,
    required this.coverageTag,
    this.createdAt,
    this.updatedAt,
  });

  final String id;
  final String caseId;
  final String content;
  final FactCategory category;
  final FactStatus status;
  final FactSource source;
  final double? confidence;
  final CoverageTag coverageTag;
  final DateTime? createdAt;
  final DateTime? updatedAt;

  factory FactDto.fromJson(Map<String, dynamic> j) => FactDto(
        id: j['id'] as String,
        caseId: (j['caseId'] ?? '') as String,
        content: (j['content'] ?? '') as String,
        category: FactCategory.fromWire(j['category'] as String? ?? 'other'),
        status: FactStatus.fromWire(j['status'] as String? ?? 'pending'),
        source: FactSource.fromWire(j['source'] as String? ?? 'user_input'),
        confidence: _d(j['confidence']),
        coverageTag: CoverageTag.fromWire(j['coverageTag'] as String? ?? 'unknown'),
        createdAt: _dt(j['createdAt']),
        updatedAt: _dt(j['updatedAt']),
      );
}

/// CreateFactReq (routes/fact.rs::CreateFactReq).
class CreateFactReq {
  CreateFactReq({
    required this.category,
    required this.statement,
    this.occurredAt,
    required this.source,
  });

  final FactCategory category;
  final String statement;
  final String? occurredAt;
  final FactSource source;

  Map<String, dynamic> toJson() => {
        'category': category.wire,
        'statement': statement,
        if (occurredAt != null) 'occurredAt': occurredAt,
        'source': source.wire,
      };
}

/// ScoreBreakdown five dimensions (routes/evidence.rs::ScoreBreakdown).
class ScoreBreakdown {
  const ScoreBreakdown({
    required this.source,
    required this.temporal,
    required this.integrity,
    required this.relevance,
    required this.authenticity,
  });

  final double source;
  final double temporal;
  final double integrity;
  final double relevance;
  final double authenticity;

  factory ScoreBreakdown.fromJson(Map<String, dynamic> j) => ScoreBreakdown(
        source: _d(j['source']) ?? 0,
        temporal: _d(j['temporal']) ?? 0,
        integrity: _d(j['integrity']) ?? 0,
        relevance: _d(j['relevance']) ?? 0,
        authenticity: _d(j['authenticity']) ?? 0,
      );

  static const ScoreBreakdown zero = ScoreBreakdown(
    source: 0,
    temporal: 0,
    integrity: 0,
    relevance: 0,
    authenticity: 0,
  );
}

/// EvidenceDto (routes/evidence.rs::EvidenceDto).
class EvidenceDto {
  EvidenceDto({
    required this.id,
    required this.caseId,
    required this.category,
    required this.status,
    required this.filePath,
    required this.fileSha256,
    required this.fileSizeBytes,
    required this.isHighSensitive,
    required this.effectiveScore,
    required this.scoreBreakdown,
    this.linkedFactIds = const [],
    this.collectedAt,
    this.createdAt,
  });

  final String id;
  final String caseId;
  final EvidenceCategory category;
  final EvidenceStatus status;
  final String filePath;
  final String fileSha256;
  final int fileSizeBytes;
  final bool isHighSensitive;
  final double effectiveScore;
  final ScoreBreakdown scoreBreakdown;
  final List<String> linkedFactIds;
  final DateTime? collectedAt;
  final DateTime? createdAt;

  factory EvidenceDto.fromJson(Map<String, dynamic> j) => EvidenceDto(
        id: j['id'] as String,
        caseId: (j['caseId'] ?? '') as String,
        category: EvidenceCategory.fromWire(j['category'] as String? ?? 'documentary_contract'),
        status: EvidenceStatus.fromWire(j['status'] as String? ?? 'uploaded'),
        filePath: (j['filePath'] ?? '') as String,
        fileSha256: (j['fileSha256'] ?? '') as String,
        fileSizeBytes: (j['fileSizeBytes'] as num?)?.toInt() ?? 0,
        isHighSensitive: (j['isHighSensitive'] ?? false) as bool,
        effectiveScore: _d(j['effectiveScore']) ?? 0,
        scoreBreakdown: j['scoreBreakdown'] == null
            ? ScoreBreakdown.zero
            : ScoreBreakdown.fromJson(j['scoreBreakdown'] as Map<String, dynamic>),
        linkedFactIds:
            (j['linkedFactIds'] as List?)?.map((e) => e as String).toList() ?? const [],
        collectedAt: _dt(j['collectedAt']),
        createdAt: _dt(j['createdAt']),
      );
}

/// CaseAggregateDto (routes/case.rs::CaseAggregateDto — flattened CaseDto + arrays).
class CaseAggregateDto {
  CaseAggregateDto({
    required this.caseDto,
    this.facts = const [],
    this.evidences = const [],
    this.documents = const [],
    this.claims = const [],
    this.deadlines = const [],
  });

  final CaseDto caseDto;
  final List<FactDto> facts;
  final List<EvidenceDto> evidences;
  final List<Map<String, dynamic>> documents;
  final List<Map<String, dynamic>> claims;
  final List<Map<String, dynamic>> deadlines;

  factory CaseAggregateDto.fromJson(Map<String, dynamic> j) => CaseAggregateDto(
        caseDto: CaseDto.fromJson(j),
        facts: (j['facts'] as List?)
                ?.map((e) => FactDto.fromJson(e as Map<String, dynamic>))
                .toList() ??
            const [],
        evidences: (j['evidences'] as List?)
                ?.map((e) => EvidenceDto.fromJson(e as Map<String, dynamic>))
                .toList() ??
            const [],
        documents: (j['documents'] as List?)
                ?.map((e) => (e as Map).cast<String, dynamic>())
                .toList() ??
            const [],
        claims: (j['claims'] as List?)
                ?.map((e) => (e as Map).cast<String, dynamic>())
                .toList() ??
            const [],
        deadlines: (j['deadlines'] as List?)
                ?.map((e) => (e as Map).cast<String, dynamic>())
                .toList() ??
            const [],
      );
}

/// LlmQueryReq (routes/llm.rs::LlmQueryReq).
class LlmQueryReq {
  LlmQueryReq({
    this.caseId,
    required this.prompt,
    this.forceLocal = false,
    this.allowCrossBorder = false,
    this.context = const {},
  });

  final String? caseId;
  final String prompt;
  final bool forceLocal;
  final bool allowCrossBorder;
  final Map<String, dynamic> context;

  Map<String, dynamic> toJson() => {
        if (caseId != null) 'caseId': caseId,
        'prompt': prompt,
        'forceLocal': forceLocal,
        'allowCrossBorder': allowCrossBorder,
        'context': context,
      };
}

/// EvidenceLink (routes/llm.rs::EvidenceLink).
class EvidenceLink {
  EvidenceLink({required this.lawRefId, required this.kbFragmentHash});
  final String lawRefId;
  final String kbFragmentHash;

  factory EvidenceLink.fromJson(Map<String, dynamic> j) => EvidenceLink(
        lawRefId: (j['lawRefId'] ?? '') as String,
        kbFragmentHash: (j['kbFragmentHash'] ?? '') as String,
      );
}

/// LlmQueryResp (routes/llm.rs::LlmQueryResp).
class LlmQueryResp {
  LlmQueryResp({
    required this.content,
    required this.sourceTag,
    required this.confidence,
    required this.coverageTag,
    this.evidenceChain = const [],
    required this.fallbackLevel,
    this.heuristicFollowups = const [],
    required this.piiBlocked,
  });

  final String content;
  final SourceTag sourceTag;
  final double confidence;
  final CoverageTag coverageTag;
  final List<EvidenceLink> evidenceChain;
  final int fallbackLevel;
  final List<String> heuristicFollowups;
  final bool piiBlocked;

  factory LlmQueryResp.fromJson(Map<String, dynamic> j) => LlmQueryResp(
        content: (j['content'] ?? '') as String,
        sourceTag: SourceTag.fromWire(j['sourceTag'] as String? ?? 'inferred'),
        confidence: _d(j['confidence']) ?? 0,
        coverageTag: CoverageTag.fromWire(j['coverageTag'] as String? ?? 'unknown'),
        evidenceChain: (j['evidenceChain'] as List?)
                ?.map((e) => EvidenceLink.fromJson(e as Map<String, dynamic>))
                .toList() ??
            const [],
        fallbackLevel: (j['fallbackLevel'] as num?)?.toInt() ?? 0,
        heuristicFollowups:
            (j['heuristicFollowups'] as List?)?.map((e) => e as String).toList() ?? const [],
        piiBlocked: (j['piiBlocked'] ?? false) as bool,
      );
}

/// KbVersionDto (routes/kb.rs::KbVersionDto).
class KbVersionDto {
  KbVersionDto({required this.versionHash, required this.versionLabel, required this.updatedAt});
  final String versionHash;
  final String versionLabel;
  final DateTime? updatedAt;

  /// Whole-day age of the active KB (Level-4 gate at >= 30 days).
  int get ageDays =>
      updatedAt == null ? 0 : DateTime.now().toUtc().difference(updatedAt!).inDays;

  factory KbVersionDto.fromJson(Map<String, dynamic> j) => KbVersionDto(
        versionHash: (j['versionHash'] ?? '') as String,
        versionLabel: (j['versionLabel'] ?? '') as String,
        updatedAt: _dt(j['updatedAt']),
      );
}

/// HealthDto (routes/health.rs::HealthDto — `{status, version}`).
class HealthDto {
  HealthDto({required this.status, required this.version});
  final String status;
  final String version;

  bool get isReady => status == 'ok';

  factory HealthDto.fromJson(Map<String, dynamic> j) => HealthDto(
        status: (j['status'] ?? '') as String,
        version: (j['version'] ?? '') as String,
      );
}

/// AuditEntryDto (routes/audit.rs::AuditEntryDto).
class AuditEntryDto {
  AuditEntryDto({
    required this.seq,
    this.caseId,
    required this.who,
    this.when,
    required this.why,
    required this.what,
    required this.category,
    required this.prevHash,
    required this.recordHash,
    this.otsProof,
  });

  final int seq;
  final String? caseId;
  final String who;
  final DateTime? when;
  final String why;
  final Object? what;
  final AuditCategory category;
  final String prevHash;
  final String recordHash;
  final String? otsProof;

  factory AuditEntryDto.fromJson(Map<String, dynamic> j) => AuditEntryDto(
        seq: (j['seq'] as num?)?.toInt() ?? 0,
        caseId: j['caseId'] as String?,
        who: (j['who'] ?? '') as String,
        when: _dt(j['when']),
        why: (j['why'] ?? '') as String,
        what: j['what'],
        category: AuditCategory.fromWire(j['category'] as String? ?? 'state_change'),
        prevHash: (j['prevHash'] ?? '') as String,
        recordHash: (j['recordHash'] ?? '') as String,
        otsProof: j['otsProof'] as String?,
      );
}

// --- shared parse helpers ---

double? _d(Object? v) => v == null ? null : (v as num).toDouble();

DateTime? _dt(Object? v) {
  if (v == null) return null;
  if (v is String) return DateTime.tryParse(v)?.toUtc();
  return null;
}
