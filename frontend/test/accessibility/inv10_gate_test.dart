import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:stuchka/features/accessibility/high_risk/countdown_button.dart';
import 'package:stuchka/features/accessibility/high_risk/disclaimer_payload.dart';
import 'package:stuchka/features/accessibility/high_risk/high_risk_gate.dart';
import 'package:stuchka/theme/stuchka_theme.dart';

Widget _wrap(Widget child) => ProviderScope(
      child: MaterialApp(theme: StuchkaTheme.light(), home: Scaffold(body: child)),
    );

void main() {
  group('INV-10 seven scenarios (spec 05 §5.7)', () {
    test('exactly 7 high-risk scenarios are defined', () {
      expect(HighRiskScenario.values.length, 7);
    });

    for (final scenario in HighRiskScenario.values) {
      testWidgets('renders the gate for ${scenario.name} with a non-collapsible disclaimer',
          (tester) async {
        await tester.pumpWidget(_wrap(
          HighRiskGate(scenario: scenario, onConfirmed: () {}),
        ));
        // Title + the full disclaimer text are present (non-collapsible — always rendered).
        expect(find.text(scenario.title), findsOneWidget);
        expect(find.text('我已完整阅读上方免责声明'), findsOneWidget);
        // The confirm button shows the 8s countdown initially.
        expect(find.byType(CountdownButton), findsOneWidget);
        // Cancel (返回) is present and is the default-focused action.
        expect(find.text('返回'), findsOneWidget);
      });
    }
  });

  group('CountdownButton 8-second cooldown (spec 05 §5.7.2)', () {
    testWidgets('starts at 8s, disabled before the countdown reaches 0', (tester) async {
      var pressed = false;
      await tester.pumpWidget(_wrap(
        CountdownButton(seconds: 8, enabled: true, label: '确认', onPressed: () => pressed = true),
      ));
      // Label shows the countdown at open (8s).
      expect(find.textContaining('8s'), findsOneWidget);
      final btn0 = tester.widget<FilledButton>(find.byType(FilledButton));
      expect(btn0.onPressed, isNull, reason: 'disabled before countdown completes');

      // Advance 8 seconds.
      for (var i = 0; i < 8; i++) {
        await tester.pump(const Duration(seconds: 1));
      }
      final btn8 = tester.widget<FilledButton>(find.byType(FilledButton));
      expect(btn8.onPressed, isNotNull, reason: 'enabled after 8s when checkbox already read');

      await tester.tap(find.byType(FilledButton));
      expect(pressed, isTrue);
    });

    testWidgets('stays disabled at 0s when not enabled (checkbox unticked)', (tester) async {
      await tester.pumpWidget(_wrap(
        CountdownButton(seconds: 8, enabled: false, label: '确认', onPressed: () {}),
      ));
      for (var i = 0; i < 8; i++) {
        await tester.pump(const Duration(seconds: 1));
      }
      final btn = tester.widget<FilledButton>(find.byType(FilledButton));
      expect(btn.onPressed, isNull, reason: 'must stay disabled until the checkbox is ticked');
    });
  });

  group('Gate confirm path requires checkbox + 8s', () {
    testWidgets('ticking the checkbox + waiting 8s enables confirm; confirm fires the callback',
        (tester) async {
      var confirmed = false;
      await tester.pumpWidget(_wrap(
        HighRiskGate(scenario: HighRiskScenario.resignAdvice, onConfirmed: () => confirmed = true),
      ));

      // Before ticking, confirm is disabled even after time passes.
      for (var i = 0; i < 8; i++) {
        await tester.pump(const Duration(seconds: 1));
      }
      var confirmBtn = tester.widget<FilledButton>(find.byType(FilledButton));
      expect(confirmBtn.onPressed, isNull);

      // Tick the read checkbox.
      await tester.tap(find.byType(CheckboxListTile));
      await tester.pump();
      confirmBtn = tester.widget<FilledButton>(find.byType(FilledButton));
      expect(confirmBtn.onPressed, isNotNull, reason: 'checkbox read + 8s elapsed → enabled');

      await tester.tap(find.byType(FilledButton));
      await tester.pump();
      expect(confirmed, isTrue);
    });
  });
}
