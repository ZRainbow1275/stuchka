import 'dart:convert';
import 'dart:typed_data';

import 'package:dio/dio.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:stuchka/bridge/bridge_dispatcher.dart';
import 'package:stuchka/bridge/bridge_protocol.dart';
import 'package:stuchka/features/editor/editor_controller.dart';
import 'package:stuchka/ipc/rust_core_client.dart';
import 'package:stuchka/ipc/rust_core_handshake.dart';

import '../../shell/fake_editor_bridge_host.dart';

/// Records the outgoing request and returns a canned `StepAck` ApiEnvelope, so we can assert the
/// controller actually POSTs `/document/:id/steps` with the correct batch (no socket opened).
class _RecordingAdapter implements HttpClientAdapter {
  RequestOptions? lastRequest;
  String? lastBody;

  @override
  void close({bool force = false}) {}

  @override
  Future<ResponseBody> fetch(
    RequestOptions options,
    Stream<Uint8List>? requestStream,
    Future<void>? cancelFuture,
  ) async {
    lastRequest = options;
    if (requestStream != null) {
      final chunks = await requestStream.toList();
      lastBody = utf8.decode(chunks.expand((c) => c).toList());
    }
    final body = jsonEncode({
      'data': {
        'docId': 'doc-1',
        'stepNo': 7,
        'auditSeq': 42,
        'results': [
          {'stepNo': 7, 'auditSeq': 42},
        ],
      },
      'error': null,
      'traceId': 'test-trace',
    });
    return ResponseBody.fromString(
      body,
      200,
      headers: {
        Headers.contentTypeHeader: [Headers.jsonContentType],
      },
    );
  }
}

RustCoreClient _client(_RecordingAdapter adapter) {
  final hs = RustCoreHandshake.parse('READY{"port":54321,"token":"${'ab' * 64}"}')!;
  final dio = Dio()..httpClientAdapter = adapter;
  return RustCoreClient(hs, dio: dio);
}

/// The REAL editor.step wire shape (editor/src/main.ts): `stepJson` is the ARRAY of ProseMirror step
/// JSON for the transaction and `clientId` is the NUMERIC Yjs clientID. The relay must accept this
/// exact shape (spec §2.11) — anything Map/String here would mask a dead A->B persistence path.
BridgeEnvelope stepEnvelope({List<Map<String, dynamic>>? steps, Object clientId = 9}) =>
    BridgeEnvelope(
      id: 'test-editor.step',
      type: BridgeMessageType.editorStep,
      payload: {
        'stepJson': steps ??
            [
              {'stepType': 'replace', 'from': 1, 'to': 1},
            ],
        'clientId': clientId,
        'version': 3,
        'why': 'ai_accept',
        'caseId': 'case-123',
        'docId': 'doc-1',
      },
    );

void main() {
  group('EditorStepRelayController (FE02 §2.11 A->B step relay over /document/:id/steps)', () {
    test('relays editor.step to POST /document/:id/steps with the batch + emits the ack', () async {
      final adapter = _RecordingAdapter();
      final host = FakeEditorBridgeHost();
      final dispatcher = BridgeDispatcher(host);
      final controller = EditorStepRelayController(
        dispatcher: dispatcher,
        client: _client(adapter),
        actor: 'actor-uuid-7',
      );

      final ackFuture = controller.acks.first;
      host.emit(stepEnvelope());
      final ack = await ackFuture;

      // The POST actually happened, to the doc-scoped route, with the audited batch.
      expect(adapter.lastRequest, isNotNull, reason: 'the step MUST be relayed, never dropped');
      expect(adapter.lastRequest!.method, 'POST');
      expect(adapter.lastRequest!.path, '/document/doc-1/steps');
      final body = jsonDecode(adapter.lastBody!) as Map<String, dynamic>;
      expect(body['caseId'], 'case-123');
      expect(body['actor'], 'actor-uuid-7');
      // The numeric Yjs clientID is stringified for the backend's String doc_step.client_id column.
      expect(body['clientId'], '9');
      final steps = body['steps'] as List;
      expect(steps.length, 1);
      expect((steps.first as Map)['why'], 'ai_accept');
      expect(((steps.first as Map)['stepJson'] as Map)['stepType'], 'replace');

      // The ack is surfaced (INV-06 audit seq), and the version is tracked.
      expect(ack.docId, 'doc-1');
      expect(ack.auditSeq, 42);
      expect(controller.lastVersion, 3);
      await controller.dispose();
      await dispatcher.dispose();
    });

    test('relays EVERY step in a multi-step transaction array as one batch', () async {
      final adapter = _RecordingAdapter();
      final host = FakeEditorBridgeHost();
      final dispatcher = BridgeDispatcher(host);
      final controller = EditorStepRelayController(
        dispatcher: dispatcher,
        client: _client(adapter),
        actor: 'actor-uuid-7',
      );

      final ackFuture = controller.acks.first;
      host.emit(stepEnvelope(steps: [
        {'stepType': 'replace', 'from': 1, 'to': 1},
        {'stepType': 'addMark', 'from': 2, 'to': 5},
        {'stepType': 'replace', 'from': 6, 'to': 6},
      ]));
      await ackFuture;

      final body = jsonDecode(adapter.lastBody!) as Map<String, dynamic>;
      final steps = body['steps'] as List;
      // The whole transaction array is relayed (no step dropped), each carrying the txn `why`.
      expect(steps.length, 3);
      expect(((steps[1] as Map)['stepJson'] as Map)['stepType'], 'addMark');
      expect(steps.every((s) => (s as Map)['why'] == 'ai_accept'), isTrue);
      await controller.dispose();
      await dispatcher.dispose();
    });

    test('disposing while a step POST is in flight never throws an uncaught StateError', () async {
      final adapter = _RecordingAdapter();
      final host = FakeEditorBridgeHost();
      final dispatcher = BridgeDispatcher(host);
      final controller = EditorStepRelayController(
        dispatcher: dispatcher,
        client: _client(adapter),
        actor: 'actor-uuid-7',
      );

      // Fire a step (its POST future is in flight) then immediately dispose; the resumed _onStep
      // must NOT add to the now-closed _acks/_errors (use-after-dispose guard).
      host.emit(stepEnvelope());
      await controller.dispose();
      await dispatcher.dispose();
      // Let any in-flight microtasks/futures resolve; the test fails if an async error escapes.
      await Future<void>.delayed(const Duration(milliseconds: 20));
    });

    test('surfaces a malformed editor.step as an error instead of POSTing', () async {
      final adapter = _RecordingAdapter();
      final host = FakeEditorBridgeHost();
      final dispatcher = BridgeDispatcher(host);
      final controller = EditorStepRelayController(
        dispatcher: dispatcher,
        client: _client(adapter),
        actor: 'actor-uuid-7',
      );

      final errFuture = controller.errors.first;
      host.emit(BridgeEnvelope(
        id: 'bad',
        type: BridgeMessageType.editorStep,
        payload: const {'stepJson': 'not-an-object'}, // missing docId/caseId/clientId
      ));
      final err = await errFuture;
      expect(err, isA<FormatException>());
      expect(adapter.lastRequest, isNull, reason: 'a malformed step must NOT be POSTed');
      await controller.dispose();
      await dispatcher.dispose();
    });
  });
}
