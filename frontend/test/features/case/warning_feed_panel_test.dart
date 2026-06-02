import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:stuchka/features/case/case_providers.dart';
import 'package:stuchka/features/case/warning/warning_feed_panel.dart';
import 'package:stuchka/ipc/dto/dtos.dart';
import 'package:stuchka/theme/stuchka_theme.dart';

WarningFeedDto _feed({required List<WarningItemDto> items}) => WarningFeedDto(
      caseId: 'case-1',
      items: items,
      evidenceLossSeam: EvidenceLossSeamDto(
        available: false,
        reason: '证据灭失预警需第三方数据保存期字段，R1 数据模型未建模',
        requiredFields: const ['third_party_retention_until'],
      ),
    );

Widget _host(WarningFeedDto feed) => ProviderScope(
      overrides: [caseWarningsProvider('case-1').overrideWith((ref) async => feed)],
      child: MaterialApp(
        theme: StuchkaTheme.light(),
        home: const WarningFeedPanel(caseId: 'case-1'),
      ),
    );

void main() {
  group('WarningFeedPanel (M12 风险预警)', () {
    testWidgets('renders a deadline tier with the INV-08 manual-confirm copy + law ref', (tester) async {
      await tester.pumpWidget(_host(_feed(items: [
        WarningItemDto(
          warningClass: 'deadline',
          kind: 'arbitration_general',
          tier: 't3_days',
          tierLabel: '3 天内到期',
          bufferedRemainingDays: 2,
          rawRemainingDays: 3,
          manualConfirmRequired: true,
          lawRefs: const ['law:中华人民共和国劳动争议调解仲裁法/v2007-12-29/§27'],
        ),
      ])));
      await tester.pumpAndSettle();
      expect(find.textContaining('劳动仲裁时效'), findsOneWidget);
      expect(find.textContaining('3 天内到期'), findsOneWidget);
      expect(find.textContaining('人工二次确认'), findsOneWidget);
      expect(find.textContaining('§27'), findsOneWidget);
    });

    testWidgets('always surfaces the evidence-loss honest seam (never fabricated)', (tester) async {
      await tester.pumpWidget(_host(_feed(items: const [])));
      await tester.pumpAndSettle();
      // No near-expiry warnings -> the explicit no-warning card.
      expect(find.textContaining('暂无临近到期的时效预警'), findsOneWidget);
      // The honest seam is declared, not faked.
      expect(find.textContaining('尚未建模'), findsOneWidget);
      expect(find.textContaining('third_party_retention_until'), findsOneWidget);
    });
  });
}
