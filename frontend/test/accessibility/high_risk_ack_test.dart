import 'dart:convert';
import 'dart:typed_data';

import 'package:dio/dio.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:stuchka/features/accessibility/high_risk/countdown_button.dart';
import 'package:stuchka/features/accessibility/high_risk/disclaimer_payload.dart';
import 'package:stuchka/features/accessibility/high_risk/high_risk_ack.dart';
import 'package:stuchka/ipc/rust_core_client.dart';
import 'package:stuchka/ipc/rust_core_handshake.dart';
import 'package:stuchka/ipc/rust_core_providers.dart';

/// A recording [HttpClientAdapter] that captures the outgoing request and returns a canned
/// `ApiEnvelope` success body. Lets us assert the frontend POSTs the INV-10 acknowledgement
/// (compliance/05 §2.3) instead of dropping the timings — without opening a socket.
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
      'data': {'seq': 42},
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

RustCoreClient _clientWith(_RecordingAdapter adapter) {
  final hs = RustCoreHandshake.parse('READY{"port":54321,"token":"${'ab' * 64}"}')!;
  final dio = Dio()..httpClientAdapter = adapter;
  return RustCoreClient(hs, dio: dio);
}

const _timings = ConfirmTimings(
  requiresSecondPress: true,
  firstCountdownMs: 8000,
  secondPressGapMs: 3000,
  totalFlowMs: 11000,
);

void main() {
  group('RustCoreClient.ackHighRisk (compliance/05 §2.3 audit four-tuple, INV-06)', () {
    test('POSTs /audit/ack with the scene id + ConfirmTimings and returns the seq', () async {
      final adapter = _RecordingAdapter();
      final client = _clientWith(adapter);

      final seq = await client.ackHighRisk(
        caseId: 'case-123',
        sceneId: 'S-02',
        timings: _timings,
      );

      expect(seq, 42);
      expect(adapter.lastRequest!.method, 'POST');
      expect(adapter.lastRequest!.path, '/audit/ack');
      final body = jsonDecode(adapter.lastBody!) as Map<String, dynamic>;
      expect(body['caseId'], 'case-123');
      expect(body['sceneId'], 'S-02');
      final t = body['timings'] as Map<String, dynamic>;
      expect(t['requiresSecondPress'], true);
      expect(t['firstCountdownMs'], 8000);
      expect(t['secondPressGapMs'], 3000);
      expect(t['totalFlowMs'], 11000);
    });
  });

  group('postHighRiskAck (the call-site helper used by every HighRiskGate site)', () {
    // Drive the helper with a REAL WidgetRef obtained from a mounted Consumer (WidgetRef is sealed
    // in Riverpod 3 and cannot be hand-rolled). The ProviderScope override injects the recording
    // client so we can assert the POST that every gate call site makes on confirm.
    Future<Map<String, dynamic>> runHelper(
      WidgetTester tester,
      _RecordingAdapter adapter,
      HighRiskScenario scenario, {
      String caseId = 'case-xyz',
    }) async {
      late WidgetRef capturedRef;
      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            rustCoreClientProvider.overrideWithValue(_clientWith(adapter)),
          ],
          child: Consumer(
            builder: (context, ref, _) {
              capturedRef = ref;
              return const SizedBox.shrink();
            },
          ),
        ),
      );
      // Call the real helper under a real (mounted) WidgetRef and await the POST directly. This is
      // deterministic — the in-memory adapter resolves under runAsync without a tap/pumpAndSettle race.
      await tester.runAsync(() async {
        await postHighRiskAck(
          capturedRef,
          caseId: caseId,
          scenario: scenario,
          timings: _timings,
        );
      });
      expect(adapter.lastRequest, isNotNull, reason: 'the ack MUST be posted, never dropped');
      expect(adapter.lastRequest!.path, '/audit/ack');
      return jsonDecode(adapter.lastBody!) as Map<String, dynamic>;
    }

    testWidgets('posts the acknowledgement on confirm, mapping the scenario to its scene id',
        (tester) async {
      final adapter = _RecordingAdapter();
      final body = await runHelper(tester, adapter, HighRiskScenario.resignAdvice);
      expect(body['caseId'], 'case-xyz');
      // resignAdvice -> S-01 (compliance/05 §1 stable scene id).
      expect(body['sceneId'], 'S-01');
    });

    testWidgets('maps every wired scenario to its compliance/05 §1 scene id', (tester) async {
      const expected = {
        HighRiskScenario.resignAdvice: 'S-01',
        HighRiskScenario.settlementBelow80: 'S-02',
        HighRiskScenario.abandonClaim: 'S-03',
        HighRiskScenario.criminalReportExport: 'S-05',
      };
      for (final entry in expected.entries) {
        final adapter = _RecordingAdapter();
        final body = await runHelper(tester, adapter, entry.key);
        expect(body['sceneId'], entry.value, reason: entry.key.name);
      }
    });
  });
}
