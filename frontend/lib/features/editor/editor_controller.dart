import 'dart:async';

import '../../bridge/bridge_dispatcher.dart';
import '../../bridge/bridge_protocol.dart';
import '../../ipc/dto/editor_dto.dart';
import '../../ipc/rust_core_client.dart';

/// The Channel-A -> Channel-B Step relay (FE02 §2.11). Subscribes to `editor.step` envelopes from
/// the WebView editor and persists each over Channel B via `POST /document/:id/steps` (main-store
/// `doc_step` + independent audit.sqlite, INV-06 double-write). CSP `connect-src 'none'` means the
/// editor CANNOT reach the core itself — this controller is the only persistence path for steps.
///
/// The `why` (edit | ai_accept | merge_resolve) rides each step (the audit four-tuple). Yjs document
/// catch-up (`editor.applyRemote`) is delivered by the Channel-B WebSocket sync provider, not by the
/// step ack, so this controller's sole job is the durable, audited relay of locally-produced steps.
class EditorStepRelayController {
  EditorStepRelayController({
    required BridgeDispatcher dispatcher,
    required RustCoreClient client,
    required String actor,
  })  : _client = client,
        _actor = actor {
    _sub = dispatcher.on(BridgeMessageType.editorStep).listen(_onStep);
  }

  final RustCoreClient _client;
  final String _actor;
  late final StreamSubscription<BridgeEnvelope> _sub;

  final StreamController<StepAckDto> _acks = StreamController<StepAckDto>.broadcast();
  final StreamController<Object> _errors = StreamController<Object>.broadcast();

  /// Acks for relayed step batches (each carrying the last step_no + audit seq).
  Stream<StepAckDto> get acks => _acks.stream;

  /// Relay failures (transport / E_* envelope errors). The step is NOT silently dropped — it is
  /// surfaced so the UI can warn + retry.
  Stream<Object> get errors => _errors.stream;

  int _lastVersion = -1;
  int get lastVersion => _lastVersion;

  Future<void> _onStep(BridgeEnvelope env) async {
    final p = env.payload;
    // The AUTHORITATIVE editor (editor/src/main.ts) emits `stepJson` as the ARRAY of ProseMirror
    // Step JSON applied by the transaction and `clientId` as the NUMERIC Yjs clientID (spec §2.11
    // reference relay: `stepJson as List`, `clientId as num`). Mirror that wire shape exactly — a
    // `Map`/`String` guard here would reject every real step and silently kill the only A->B
    // persistence path (INV-06 doc_step + audit.sqlite double-write).
    final stepJson = p['stepJson'];
    final docId = p['docId'];
    final caseId = p['caseId'];
    final clientIdRaw = p['clientId'];
    final why = (p['why'] as String?) ?? 'edit';
    if (stepJson is! List ||
        stepJson.isEmpty ||
        stepJson.any((s) => s is! Map) ||
        docId is! String ||
        caseId is! String ||
        clientIdRaw == null) {
      _emitError(
        FormatException('editor.step missing/invalid required fields: ${env.payload.keys}'),
      );
      return;
    }
    // Yjs clientID is numeric on the wire; the backend `doc_step.client_id` / audit `who` is a
    // String column (the merge route persists the literal "merge"), so STRINGIFY rather than reject.
    final clientId = clientIdRaw is num ? clientIdRaw.toInt().toString() : clientIdRaw.toString();
    final version = (p['version'] as num?)?.toInt();
    if (version != null) _lastVersion = version;

    // One transaction -> one batch; every step in it shares the transaction's INV-06 `why` (E1).
    final batch = EditorStepBatch(
      caseId: caseId,
      actor: _actor,
      clientId: clientId,
      steps: [
        for (final s in stepJson)
          EditorStepDto(stepJson: (s as Map).cast<String, dynamic>(), why: why),
      ],
    );
    try {
      final ack = await _client.pushDocumentSteps(docId, batch);
      if (!_acks.isClosed) _acks.add(ack);
    } catch (e) {
      _emitError(e);
    }
  }

  /// Surface a relay error WITHOUT crashing if the controller was disposed mid-flight: `_onStep` is
  /// async and can resume (after an awaited POST) after [dispose] has closed the controllers. Adding
  /// to a closed StreamController throws `StateError`, so guard every add (mirrors the host /
  /// dispatcher isClosed/hasListener guards).
  void _emitError(Object error) {
    if (!_errors.isClosed) _errors.add(error);
  }

  Future<void> dispose() async {
    await _sub.cancel();
    await _acks.close();
    await _errors.close();
  }
}
