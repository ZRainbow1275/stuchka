import 'dart:convert';
import 'dart:typed_data';

import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:stuchka/features/case/case_providers.dart';
import 'package:stuchka/features/case/performance/performance_timeline_page.dart';
import 'package:stuchka/ipc/dto/dtos.dart';
import 'package:stuchka/ipc/rust_core_client.dart';
import 'package:stuchka/ipc/rust_core_handshake.dart';
import 'package:stuchka/ipc/rust_core_providers.dart';
import 'package:stuchka/theme/stuchka_theme.dart';

/// A recording adapter that captures the outgoing `/performance/evaluate` request and returns a
/// canned `PerformanceStatus` envelope shaped exactly like `rule_engine::PerformanceStatus` (one
/// on-time + one overdue installment, with the §250 enforcement countdown reanchored at the first
/// breach). This lets the test assert the real wire shape + parsing without opening a socket.
class _PerfAdapter implements HttpClientAdapter {
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
        'installments': [
          {
            'dueOn': '2024-02-01',
            'amount': '5000',
            'paidOn': '2024-02-01',
            'paidAmount': '5000',
            'state': {'state': 'paid_on_time'},
          },
          {
            'dueOn': '2024-03-01',
            'amount': '5000',
            'paidOn': null,
            'paidAmount': null,
            'state': {'state': 'overdue', 'overdue_days': 31, 'shortfall': '5000'},
          },
        ],
        'totalAmount': '10000',
        'totalPaid': '5000',
        'totalShortfall': '5000',
        'firstBreachAt': '2024-03-01',
        'enforcement': {
          'status': 'ok',
          'value': {
            'Deadline': {
              'kind': 'enforcement',
              'raw_remaining_days': 699,
              'buffered_remaining_days': 629,
              'state': {'state': 'running', 'remaining_days': 699},
              'manual_confirm_required': true,
              'law_refs': [
                {'urn': 'law:中华人民共和国民事诉讼法/v2023-09-01/§250'}
              ],
            }
          },
          'coverage_tag': 'exact',
          'law_refs': [
            {'urn': 'law:中华人民共和国民事诉讼法/v2023-09-01/§250'}
          ],
          'derivation': const [],
        },
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

RustCoreClient _clientWith(_PerfAdapter adapter) {
  final hs = RustCoreHandshake.parse('READY{"port":54321,"token":"${'ab' * 64}"}')!;
  final dio = Dio()..httpClientAdapter = adapter;
  return RustCoreClient(hs, dio: dio);
}

CaseAggregateDto _aggregate() => CaseAggregateDto.fromJson(const {
      'id': 'case-1',
      'status': 'draft',
      'identityType': 'standard_full_time',
      'province': '广东省',
      'city': '深圳市',
      'caseOccurredAt': '2024-01-01',
      'disputeSubtype': 'social_ins_arrears',
      'kbVersionHash': 'hash',
    });

void main() {
  group('RustCoreClient.evaluatePerformance (M16 wire contract)', () {
    test('POSTs /performance/evaluate with snake_case installments + parses the breach outcome',
        () async {
      final adapter = _PerfAdapter();
      final client = _clientWith(adapter);

      final status = await client.evaluatePerformance(PerformanceEvalReq(
        caseId: 'case-1',
        province: '广东省',
        city: '深圳市',
        instrumentKind: 'award',
        effectiveDate: '2024-01-01',
        installments: [
          PerformanceInstallmentReq(
            dueOn: '2024-02-01',
            amount: '5000',
            paidOn: '2024-02-01',
            paidAmount: '5000',
          ),
          PerformanceInstallmentReq(dueOn: '2024-03-01', amount: '5000'),
        ],
      ));

      // Request shape: outer camelCase, installment items snake_case; unpaid row omits paid_on.
      expect(adapter.lastRequest!.method, 'POST');
      expect(adapter.lastRequest!.path, '/performance/evaluate');
      final sent = jsonDecode(adapter.lastBody!) as Map<String, dynamic>;
      expect(sent['instrumentKind'], 'award');
      expect(sent['effectiveDate'], '2024-01-01');
      final insts = sent['installments'] as List;
      expect((insts[0] as Map)['due_on'], '2024-02-01');
      expect((insts[0] as Map)['paid_on'], '2024-02-01');
      expect((insts[1] as Map).containsKey('paid_on'), isFalse);

      // Parsed engine output: real per-installment states + §250 enforcement countdown.
      expect(status.hasBreach, isTrue);
      expect(status.firstBreachAt, '2024-03-01');
      expect(status.totalShortfall, '5000');
      expect(status.installments[0].state, 'paid_on_time');
      expect(status.installments[1].isBreach, isTrue);
      expect(status.installments[1].overdueDays, 31);
      expect(status.enforcement!.isOk, isTrue);
      expect(status.enforcement!.bufferedRemainingDays, 629);
      expect(status.enforcement!.lawRefs.first, contains('§250'));
    });
  });

  group('PerformanceTimelinePage (M16 履行监控)', () {
    Widget host(RustCoreClient client) => ProviderScope(
          overrides: [
            rustCoreClientProvider.overrideWithValue(client),
            caseAggregateProvider('case-1').overrideWith((ref) async => _aggregate()),
          ],
          child: MaterialApp(
            theme: StuchkaTheme.light(),
            home: const PerformanceTimelinePage(caseId: 'case-1'),
          ),
        );

    testWidgets('renders the schedule form (instrument selector + add-installment)', (tester) async {
      await tester.pumpWidget(host(_clientWith(_PerfAdapter())));
      await tester.pumpAndSettle();
      expect(find.text('按期付款时间表'), findsOneWidget);
      expect(find.text('增加一期'), findsOneWidget);
      expect(find.text('评估履行情况'), findsOneWidget);
    });

    testWidgets('evaluates a schedule and renders the real breach + §250 countdown', (tester) async {
      await tester.pumpWidget(host(_clientWith(_PerfAdapter())));
      await tester.pumpAndSettle();

      // Fill the first installment row (TextField order: 0=effectiveDate, 1=dueOn, 2=amount).
      final fields = find.byType(TextField);
      await tester.enterText(fields.at(1), '2024-03-01');
      await tester.enterText(fields.at(2), '5000');

      await tester.tap(find.text('评估履行情况'));
      await tester.pumpAndSettle();

      // The engine's own breach state + enforcement countdown surface (nothing fabricated).
      expect(find.textContaining('存在违约期'), findsOneWidget);
      expect(find.textContaining('违约'), findsWidgets);
      expect(find.text('执行申请时效倒计时'), findsOneWidget);
      expect(find.textContaining('人工二次确认'), findsOneWidget);
      // §250 surfaces in both the countdown body and the law-ref line.
      expect(find.textContaining('§250'), findsWidgets);
    });
  });
}
