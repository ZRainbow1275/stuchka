import 'dart:convert';

import 'package:dio/dio.dart';

import '../features/accessibility/high_risk/countdown_button.dart';
import 'dto/diagnose_dto.dart';
import 'dto/dtos.dart';
import 'dto/editor_dto.dart';
import 'rust_core_handshake.dart';

/// Maps a backend `ApiError.code` / transport failure onto a typed Dart exception (backend/01
/// §1.12). UI code matches on [code] for E_INVALID_TOKEN / E_PII_BLOCKED / E_KB_OUTDATED /
/// E_LLM_ABSTENTION etc.
class RustCoreApiException implements Exception {
  RustCoreApiException({
    required this.code,
    required this.message,
    this.httpStatus,
    this.hint,
    this.traceId,
  });

  final String code;
  final String message;
  final int? httpStatus;
  final String? hint;
  final String? traceId;

  bool get isInvalidToken => code == 'E_INVALID_TOKEN';
  bool get isPiiBlocked => code == 'E_PII_BLOCKED';
  bool get isKbOutdated => code == 'E_KB_OUTDATED';
  bool get isLlmAbstention => code == 'E_LLM_ABSTENTION';
  bool get isRuleNoCoverage => code == 'E_RULE_NO_COVERAGE';

  @override
  String toString() => 'RustCoreApiException($code: $message)';
}

/// The channel-B HTTP client. Wraps a dio instance whose interceptor injects
/// `Authorization: Bearer <token>` on EVERY request (D1). The loopback Host is implicit in the
/// base URL `http://127.0.0.1:<port>` so the backend's anti-rebinding Host gate is satisfied.
class RustCoreClient {
  RustCoreClient(this.handshake, {Dio? dio}) : _dio = dio ?? Dio() {
    _dio.options
      ..baseUrl = handshake.baseUrl
      ..connectTimeout = const Duration(seconds: 5)
      ..receiveTimeout = const Duration(seconds: 30)
      ..headers['Authorization'] = 'Bearer ${handshake.token}'
      ..validateStatus = (_) => true; // we map errors ourselves from the envelope
  }

  final RustCoreHandshake handshake;
  final Dio _dio;

  Dio get raw => _dio;

  // --- self-check ---

  /// `GET /health` self-check. Returns the [HealthDto]; throws on non-200 / non-ready.
  Future<HealthDto> health() async {
    final res = await _dio.get<Map<String, dynamic>>('/health');
    return _unwrap(res, (d) => HealthDto.fromJson(d as Map<String, dynamic>));
  }

  /// Convenience: true iff the backend reports ready.
  Future<bool> isReady() async {
    try {
      return (await health()).isReady;
    } catch (_) {
      return false;
    }
  }

  // --- case ---

  Future<CaseDto> createCase(CreateCaseReq req) async {
    final res = await _dio.post<Map<String, dynamic>>('/case', data: req.toJson());
    return _unwrap(res, (d) => CaseDto.fromJson(d as Map<String, dynamic>));
  }

  Future<List<CaseDto>> listCases() async {
    final res = await _dio.get<Map<String, dynamic>>('/case');
    return _unwrap<List<CaseDto>>(res, (data) {
      // backend returns `{ count, cases: [...] }`.
      final map = data as Map<String, dynamic>;
      final cases = (map['cases'] as List?) ?? const [];
      return cases.map((e) => CaseDto.fromJson(e as Map<String, dynamic>)).toList();
    });
  }

  Future<CaseAggregateDto> getCase(String id) async {
    final res = await _dio.get<Map<String, dynamic>>('/case/$id');
    return _unwrap(res, (d) => CaseAggregateDto.fromJson(d as Map<String, dynamic>));
  }

  Future<CaseDto> patchCase(String id, {required String action, required String reason}) async {
    final res = await _dio.patch<Map<String, dynamic>>(
      '/case/$id',
      data: {'action': action, 'reason': reason},
    );
    return _unwrap(res, (d) => CaseDto.fromJson(d as Map<String, dynamic>));
  }

  // --- fact ---

  Future<FactDto> createFact(String caseId, CreateFactReq req) async {
    final res = await _dio.post<Map<String, dynamic>>('/case/$caseId/fact', data: req.toJson());
    return _unwrap(res, (d) => FactDto.fromJson(d as Map<String, dynamic>));
  }

  Future<FactDto> patchFact(String factId, {required String action, required String reason}) async {
    final res = await _dio.patch<Map<String, dynamic>>(
      '/fact/$factId',
      data: {'action': action, 'reason': reason},
    );
    return _unwrap(res, (d) => FactDto.fromJson(d as Map<String, dynamic>));
  }

  // --- evidence ---

  /// `POST /case/:id/evidence` multipart upload (file + metadata JSON string).
  Future<EvidenceDto> uploadEvidence(
    String caseId, {
    required List<int> fileBytes,
    required String fileName,
    required Map<String, dynamic> metadata,
  }) async {
    final form = FormData.fromMap({
      'file': MultipartFile.fromBytes(fileBytes, filename: fileName),
      'metadata': jsonEncode(metadata),
    });
    final res = await _dio.post<Map<String, dynamic>>('/case/$caseId/evidence', data: form);
    return _unwrap(res, (d) => EvidenceDto.fromJson(d as Map<String, dynamic>));
  }

  Future<EvidenceDto> rescoreEvidence(String evidenceId) async {
    final res = await _dio.post<Map<String, dynamic>>('/evidence/$evidenceId/rescore');
    return _unwrap(res, (d) => EvidenceDto.fromJson(d as Map<String, dynamic>));
  }

  // --- diagnose (M1) ---

  /// `POST /diagnose` — the deterministic M1 DiagnosisEngine (backend item A). Drives the structured
  /// §4.1.2 DiagnosisOutput (subcategory + coverage_tier + recommended_procedures + next_actions)
  /// from the ≤7-question 问诊树 answer set. This is the PRIMARY diagnosis path; the LLM free-text
  /// query below is a secondary assist only.
  Future<DiagnosisOutput> diagnose(DiagnoseReq req) async {
    final res = await _dio.post<Map<String, dynamic>>('/diagnose', data: req.toJson());
    return _unwrap(res, (d) => DiagnosisOutput.fromJson(d as Map<String, dynamic>));
  }

  // --- llm ---

  Future<LlmQueryResp> llmQuery(LlmQueryReq req) async {
    final res = await _dio.post<Map<String, dynamic>>('/llm/query', data: req.toJson());
    return _unwrap(res, (d) => LlmQueryResp.fromJson(d as Map<String, dynamic>));
  }

  // --- compute / deadline (typed wrappers return the raw maps; rule outcomes are tagged unions) ---

  Future<Map<String, dynamic>> computeRun(Map<String, dynamic> req) async {
    final res = await _dio.post<Map<String, dynamic>>('/compute/run', data: req);
    return _unwrap(res, (d) => (d as Map).cast<String, dynamic>());
  }

  Future<Map<String, dynamic>> deadlineRun(Map<String, dynamic> req) async {
    final res = await _dio.post<Map<String, dynamic>>('/deadline/run', data: req);
    return _unwrap(res, (d) => (d as Map).cast<String, dynamic>());
  }

  // --- document ---

  /// `POST /case/:id/document` — create a Yjs document for the case and return its id. Used by the
  /// editor host to obtain a `docId` before mounting the WebView (FE02 §2.4).
  Future<String> createDocument(String caseId, {String templateId = 'arb_application'}) async {
    final res = await _dio.post<Map<String, dynamic>>(
      '/case/$caseId/document',
      data: {'templateId': templateId, 'claimIds': const <String>[]},
    );
    return _unwrap(res, (d) => (d as Map)['docId'] as String);
  }

  // --- editor steps (A->B relay, FE02 §2.11) ---

  /// `POST /document/:id/steps` — persist a batch of ProseMirror editor steps relayed from the
  /// WebView editor (Channel A) into the main-store `doc_step` table + the independent audit.sqlite
  /// (INV-06 double-write). Each step carries its `why` (edit | ai_accept | merge_resolve). Returns
  /// the [StepAckDto] (per-step step_no + audit seq).
  Future<StepAckDto> pushDocumentSteps(String docId, EditorStepBatch batch) async {
    final res = await _dio.post<Map<String, dynamic>>(
      '/document/$docId/steps',
      data: batch.toJson(),
    );
    return _unwrap(res, (d) => StepAckDto.fromJson(d as Map<String, dynamic>));
  }

  /// `POST /document/:id/export` — produce the real GB 45438 三件套 dossier zip (all four layers,
  /// INV-02) + the D3 Export audit. Used by the S-05 刑事报案材料导出 high-risk site so the export is
  /// a genuine artefact, never a fabricated success message. Returns the [ExportDocRespDto] (zip path
  /// + entries + completeness); throws on any backend error so the caller surfaces the failure.
  Future<ExportDocRespDto> exportDocument(
    String docId, {
    List<String> formats = const ['pdf', 'md', 'json'],
    bool embedWaterMark = true,
    int gb45438Level = 4,
  }) async {
    final res = await _dio.post<Map<String, dynamic>>(
      '/document/$docId/export',
      data: {
        'formats': formats,
        'embedWaterMark': embedWaterMark,
        'gb45438Level': gb45438Level,
      },
    );
    return _unwrap(res, (d) => ExportDocRespDto.fromJson(d as Map<String, dynamic>));
  }

  // --- kb ---

  Future<KbVersionDto> kbVersion() async {
    final res = await _dio.get<Map<String, dynamic>>('/kb/version');
    return _unwrap(res, (d) => KbVersionDto.fromJson(d as Map<String, dynamic>));
  }

  // --- audit ---

  /// `POST /audit/ack` — persist an INV-10 high-risk disclaimer acknowledgement into the
  /// independent audit.sqlite hash chain (compliance/05 §2.3 audit four-tuple, INV-06). Called by
  /// every `HighRiskGate` call site on the SECOND (final) confirm so the acknowledgement
  /// (scene + measured [ConfirmTimings]) is AUDITED rather than dropped at the UI. Returns the
  /// appended record `seq`.
  Future<int> ackHighRisk({
    required String caseId,
    required String sceneId,
    required ConfirmTimings timings,
  }) async {
    final res = await _dio.post<Map<String, dynamic>>(
      '/audit/ack',
      data: {
        'caseId': caseId,
        'sceneId': sceneId,
        'timings': {
          'requiresSecondPress': timings.requiresSecondPress,
          'firstCountdownMs': timings.firstCountdownMs,
          'secondPressGapMs': timings.secondPressGapMs,
          'totalFlowMs': timings.totalFlowMs,
        },
      },
    );
    return _unwrap(res, (d) => ((d as Map)['seq'] as num).toInt());
  }

  Future<List<AuditEntryDto>> queryAudit({String? caseId, int? cursor, int? limit}) async {
    final query = <String, dynamic>{};
    if (caseId != null) query['case_id'] = caseId;
    if (cursor != null) query['cursor'] = cursor;
    if (limit != null) query['limit'] = limit;
    final res = await _dio.get<dynamic>('/audit', queryParameters: query);
    return _unwrapList(res, AuditEntryDto.fromJson);
  }

  // --- envelope unwrap ---

  /// Map a bare HTTP status (the bearer/anti-rebinding middleware returns an empty-body 401/403
  /// before any handler runs) onto the §1.12 error code so the UI sees E_INVALID_TOKEN etc. rather
  /// than a generic E_INTERNAL.
  RustCoreApiException _statusError(int? status) {
    final code = switch (status) {
      401 => 'E_INVALID_TOKEN',
      403 => 'E_INVALID_TOKEN', // loopback Host / Origin gate
      404 => 'E_NOT_FOUND',
      _ => 'E_INTERNAL',
    };
    final message = switch (code) {
      'E_INVALID_TOKEN' => '鉴权失败：本地通信令牌无效或缺失',
      'E_NOT_FOUND' => '未找到对应记录',
      _ => '响应体不是合法的 JSON 信封',
    };
    return RustCoreApiException(code: code, message: message, httpStatus: status);
  }

  T _unwrap<T>(Response res, T Function(Object? data) parse) {
    final body = res.data;
    if (body is! Map<String, dynamic>) {
      throw _statusError(res.statusCode);
    }
    final env = ApiEnvelope.fromJson<T>(body, parse);
    if (!env.isOk || env.data == null) {
      final e = env.error;
      throw RustCoreApiException(
        code: e?.code ?? 'E_INTERNAL',
        message: e?.message ?? '未知错误',
        httpStatus: res.statusCode,
        hint: e?.hint,
        traceId: env.traceId,
      );
    }
    return env.data as T;
  }

  List<R> _unwrapList<R>(Response res, R Function(Map<String, dynamic>) parseItem) {
    final body = res.data;
    if (body is! Map<String, dynamic>) {
      throw _statusError(res.statusCode);
    }
    if (body['error'] != null) {
      final e = ApiError.fromJson(body['error'] as Map<String, dynamic>);
      throw RustCoreApiException(
        code: e.code,
        message: e.message,
        httpStatus: res.statusCode,
        hint: e.hint,
        traceId: body['traceId'] as String?,
      );
    }
    final data = body['data'];
    if (data is! List) return const [];
    return data.map((e) => parseItem((e as Map).cast<String, dynamic>())).toList();
  }
}
