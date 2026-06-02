import 'dart:convert';
import 'dart:typed_data';

import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:stuchka/features/case/case_providers.dart';
import 'package:stuchka/features/kb/kb_management_page.dart';
import 'package:stuchka/ipc/dto/dtos.dart';
import 'package:stuchka/ipc/rust_core_client.dart';
import 'package:stuchka/ipc/rust_core_handshake.dart';
import 'package:stuchka/ipc/rust_core_providers.dart';
import 'package:stuchka/theme/stuchka_theme.dart';

/// Returns a canned `/kb/refresh` envelope so the refresh button exercises the real client + DTO
/// parse without a socket. The freshness/age echo what a re-validated manifest would report.
class _KbRefreshAdapter implements HttpClientAdapter {
  @override
  void close({bool force = false}) {}

  @override
  Future<ResponseBody> fetch(
    RequestOptions options,
    Stream<Uint8List>? requestStream,
    Future<void>? cancelFuture,
  ) async {
    final body = jsonEncode({
      'data': {
        'versionHash': 'abc123',
        'versionLabel': '2026-05 法规包',
        'ageDays': 3,
        'freshness': 'fresh',
        'changed': false,
      },
      'error': null,
      'traceId': 't',
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

RustCoreClient _client() {
  final hs = RustCoreHandshake.parse('READY{"port":54321,"token":"${'ab' * 64}"}')!;
  final dio = Dio()..httpClientAdapter = _KbRefreshAdapter();
  return RustCoreClient(hs, dio: dio);
}

KbVersionDto _version({required int ageDays}) => KbVersionDto(
      versionHash: 'abc123',
      versionLabel: '2026-05 法规包',
      updatedAt: DateTime.now().toUtc().subtract(Duration(days: ageDays)),
    );

Widget _host(RustCoreClient client, KbVersionDto version) => ProviderScope(
      overrides: [
        rustCoreClientProvider.overrideWithValue(client),
        kbVersionProvider.overrideWith((ref) async => version),
      ],
      child: MaterialApp(theme: StuchkaTheme.light(), home: const KbManagementPage()),
    );

void main() {
  group('KbManagementPage (M6 知识库管理)', () {
    testWidgets('shows the live version + label + hash', (tester) async {
      await tester.pumpWidget(_host(_client(), _version(ageDays: 3)));
      await tester.pumpAndSettle();
      expect(find.text('当前知识库'), findsOneWidget);
      expect(find.textContaining('2026-05 法规包'), findsOneWidget);
      expect(find.textContaining('abc123'), findsWidgets);
      expect(find.text('检查并校验知识库'), findsOneWidget);
    });

    testWidgets('surfaces the INV-04 Level-4 expired warning at >= 30 days', (tester) async {
      await tester.pumpWidget(_host(_client(), _version(ageDays: 31)));
      await tester.pumpAndSettle();
      expect(find.textContaining('赔偿计算已停用'), findsOneWidget);
      expect(find.textContaining('Level4'), findsWidgets);
    });

    testWidgets('refresh calls the real client and renders the freshness result', (tester) async {
      await tester.pumpWidget(_host(_client(), _version(ageDays: 3)));
      await tester.pumpAndSettle();
      await tester.tap(find.text('检查并校验知识库'));
      await tester.pumpAndSettle();
      expect(find.text('校验结果'), findsOneWidget);
      // The honest "did not pull from the network" flag is shown truthfully.
      expect(find.textContaining('未联网拉取'), findsOneWidget);
    });
  });
}
