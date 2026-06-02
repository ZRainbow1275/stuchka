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

/// Spec §3.x key phrases that MUST appear verbatim in each arm (compliance/05 §3 文案硬编码).
const Map<HighRiskScenario, List<String>> _keyPhrases = {
  HighRiskScenario.resignAdvice: ['主动离职', '《劳动合同法》第三十六条', '反 HR 立场是道德姿态'],
  HighRiskScenario.settlementBelow80: ['和解协议', '规则引擎计算结果', '确定性计算', '12348'],
  HighRiskScenario.abandonClaim: ['仲裁请求项', '不能在同一仲裁程序中再次提出', '12351'],
  HighRiskScenario.groupRepresentativeAuth: ['群体案件代表授权', '代表人的处置行为对全体被代表人发生法律效力'],
  HighRiskScenario.criminalReportExport: ['拒不支付劳动报酬罪', '《刑法》第二百七十六条之一', '前置程序'],
  HighRiskScenario.protectedScenario: ['特殊保护情形', '医疗期', '工伤', '12338'],
  HighRiskScenario.groupCreation: ['群组案件', '授权链', '12351'],
};

/// BYTE-VERBATIM anchors (compliance/05 §3 "禁止任何形式的修改（含标点）"). Each entry is copied
/// character-for-character from the spec's `### 3.x` fenced block — including the spec's ASCII
/// STRAIGHT double quotes (`"…"`, code point 0x22, NOT typographic `“…”`) and the exact line
/// breaks + indentation. The test asserts each appears as an exact substring of the rendered arm,
/// so any quote substitution or indentation collapse re-introduced later is caught.
const Map<HighRiskScenario, List<String>> _verbatimAnchors = {
  // §3.1 — the title carries the spec's ASCII straight quotes; the §3.1.3 alternative-path bullet
  // keeps the spec's 5-space sub-bullet indentation across the wrapped line.
  HighRiskScenario.resignAdvice: [
    '重要提示：关于您当前考虑的"主动离职"决定',
    '1. 一旦您主动以"个人原因" 提出离职，依据《劳动合同法》第三十六条，\n'
        '   您将不再有权依据本法第四十六条、第四十七条要求经济补偿金；',
    '   - 以"被迫解除劳动合同" 为由（《劳动合同法》第三十八条）提出解除，\n'
        '     并在通知中明确列举用人单位的违法事实，',
  ],
  // §3.2 — the determinacy clause + the spec's 3-space indented numbered body.
  HighRiskScenario.settlementBelow80: [
    '2. 本系统的规则引擎计算结果仅是基于您输入事实的确定性计算，\n'
        '   不包括以下不确定因素：',
    '   - 拒绝任何"现场签字、不给带回家研究" 的施压。',
  ],
  // §3.3 — the irrevocability sentence + the 诱导 bullet (ASCII straight quotes).
  HighRiskScenario.abandonClaim: [
    '1. 一旦放弃，该请求项不能在同一仲裁程序中再次提出。',
    '   - 被对方"以撤销其他主张为条件诱导" → 这通常对您不利，',
  ],
  // §3.4 — the multi-line legal-effect clause keeps its 3-space indentation.
  HighRiskScenario.groupRepresentativeAuth: [
    '   依据《民事诉讼法》代表人诉讼条款，\n'
        '   代表人的处置行为对全体被代表人发生法律效力，',
    '   - 任何"和解 / 放弃请求 / 降低主张" 的处置，',
  ],
  // §3.5 — the charge name with ASCII straight quotes + the 责令支付通知书 prerequisite bullet.
  HighRiskScenario.criminalReportExport: [
    '您正在导出针对"拒不支付劳动报酬罪"（《刑法》第二百七十六条之一）',
    '   - 先经劳动监察大队（劳动保障监察部门）投诉并取得"责令支付通知书"，',
  ],
  // §3.6 — the 劳动争议处理路径 clause keeps its straight quotes + indentation.
  HighRiskScenario.protectedScenario: [
    '1. 上述情形受多部法律的额外保护，\n'
        '   常规的"劳动争议处理路径" 在这些场景下存在重要差异：',
  ],
};

void main() {
  group('INV-10 seven scenarios (spec 05 §5.7)', () {
    test('exactly 7 high-risk scenarios are defined', () {
      expect(HighRiskScenario.values.length, 7);
    });

    // C2: the wired-vs-deferred partition is explicit (no silent drop). The 4 wired scenarios have
    // a real trigger site in this build; S-04 + group-creation are deferred to the group-case module.
    test('wired + deferred scenarios are explicit and disjoint', () {
      expect(
        kWiredHighRiskScenarios.intersection(kDeferredHighRiskScenarios),
        isEmpty,
      );
      expect(kWiredHighRiskScenarios, contains(HighRiskScenario.resignAdvice));
      expect(kWiredHighRiskScenarios, contains(HighRiskScenario.settlementBelow80));
      expect(kWiredHighRiskScenarios, contains(HighRiskScenario.abandonClaim));
      expect(kWiredHighRiskScenarios, contains(HighRiskScenario.criminalReportExport));
      expect(kDeferredHighRiskScenarios, contains(HighRiskScenario.groupRepresentativeAuth));
      expect(kDeferredHighRiskScenarios, contains(HighRiskScenario.groupCreation));
    });

    // C1: every arm's verbatim disclaimer is >= 200 chars and contains its spec key phrases.
    for (final scenario in HighRiskScenario.values) {
      test('${scenario.name} disclaimer is >= 200 chars and matches §3 key phrases', () {
        // Sample payload exercises the interpolation slots (S-02/S-03/S-04/S-06).
        const payload = DisclaimerPayload(
          expectedAmount: '60000',
          settlementAmount: '30000',
          ratio: '50',
          requestedItemName: '未签合同二倍工资差额',
          amount: '48000',
          confidence: '较高',
          grantOrAccept: '接受',
          roleSpecificDescription: '您将作为被代表方接受代表人的程序处置。',
          detectedCategories: '医疗期、工伤',
        );
        final text = scenario.fullDisclaimer(payload);
        expect(text.length, greaterThanOrEqualTo(200),
            reason: '${scenario.name} measured ${text.length} chars');
        for (final phrase in _keyPhrases[scenario]!) {
          expect(text, contains(phrase.trimRight()),
              reason: '${scenario.name} missing key phrase: $phrase');
        }
      });
    }

    // The >= 200-char floor holds even with NO payload (slots fall back to 未提供).
    test('every arm >= 200 chars even with an empty payload', () {
      for (final scenario in HighRiskScenario.values) {
        expect(scenario.fullDisclaimer().length, greaterThanOrEqualTo(200),
            reason: scenario.name);
      }
    });

    // FIDELITY: each §3 arm's STATIC text is BYTE-EXACT to the spec (compliance/05 §3 含标点). The
    // anchors are copied character-for-character from the spec fence (ASCII straight quotes + exact
    // line breaks + indentation); exact-substring containment catches any quote substitution or
    // indentation collapse.
    for (final entry in _verbatimAnchors.entries) {
      test('${entry.key.name} static text is byte-verbatim to compliance/05 §3', () {
        final text = entry.key.fullDisclaimer();
        for (final anchor in entry.value) {
          expect(text, contains(anchor),
              reason: '${entry.key.name} must contain the spec byte-verbatim:\n$anchor');
        }
      });
    }

    // FIDELITY: the spec §3 fences use ASCII straight double quotes only — typographic Chinese
    // quotes (“ ” U+201C/U+201D) MUST NOT appear in any §3 arm (they would be a 含标点 violation).
    test('no typographic curly double quotes in any spec §3 arm', () {
      const leftCurly = '“'; // “
      const rightCurly = '”'; // ”
      const specArms = <HighRiskScenario>[
        HighRiskScenario.resignAdvice,
        HighRiskScenario.settlementBelow80,
        HighRiskScenario.abandonClaim,
        HighRiskScenario.groupRepresentativeAuth,
        HighRiskScenario.criminalReportExport,
        HighRiskScenario.protectedScenario,
      ];
      for (final scenario in specArms) {
        // Exercise the slots too, so an interpolated value never re-introduces curly quotes.
        const payload = DisclaimerPayload(
          expectedAmount: '60000',
          settlementAmount: '30000',
          ratio: '50',
          requestedItemName: '未签合同二倍工资差额',
          amount: '48000',
          confidence: '较高',
          grantOrAccept: '接受',
          roleSpecificDescription: '您将作为被代表方接受代表人的程序处置。',
          detectedCategories: '医疗期、工伤',
        );
        final text = scenario.fullDisclaimer(payload);
        expect(text.contains(leftCurly), isFalse,
            reason: '${scenario.name} contains a typographic “ (must be ASCII ")');
        expect(text.contains(rightCurly), isFalse,
            reason: '${scenario.name} contains a typographic ” (must be ASCII ")');
        // And the rendered text must NOT carry Markdown emphasis markers.
        expect(text.contains('**'), isFalse,
            reason: '${scenario.name} must not render literal Markdown ** emphasis');
      }
    });

    for (final scenario in HighRiskScenario.values) {
      testWidgets('renders the gate for ${scenario.name} with a non-collapsible disclaimer',
          (tester) async {
        await tester.pumpWidget(_wrap(
          HighRiskGate(scenario: scenario, onConfirmed: (_) {}),
        ));
        expect(find.text(scenario.title), findsOneWidget);
        expect(find.text('我已完整阅读上方免责声明'), findsOneWidget);
        expect(find.byType(CountdownButton), findsOneWidget);
        // Cancel (返回) is present and is the default-focused action.
        expect(find.text('返回'), findsOneWidget);
      });
    }
  });

  group('CountdownButton two-stage cooldown (compliance/05 §2.2 step5 / §5.1 / §9.1)', () {
    testWidgets('stage-1: starts at 8s, disabled before the read countdown reaches 0',
        (tester) async {
      await tester.pumpWidget(_wrap(
        CountdownButton(seconds: 8, enabled: true, label: '确认', onPressed: (_) {}),
      ));
      expect(find.textContaining('8s'), findsOneWidget);
      final btn0 = tester.widget<FilledButton>(find.byType(FilledButton));
      expect(btn0.onPressed, isNull, reason: 'disabled before the 8s read countdown completes');

      for (var i = 0; i < 8; i++) {
        await tester.pump(const Duration(seconds: 1));
      }
      final btn8 = tester.widget<FilledButton>(find.byType(FilledButton));
      expect(btn8.onPressed, isNotNull, reason: 'first press enabled after 8s when checkbox read');
    });

    testWidgets('stays disabled at 0s when not enabled (checkbox unticked)', (tester) async {
      await tester.pumpWidget(_wrap(
        CountdownButton(seconds: 8, enabled: false, label: '确认', onPressed: (_) {}),
      ));
      for (var i = 0; i < 8; i++) {
        await tester.pump(const Duration(seconds: 1));
      }
      final btn = tester.widget<FilledButton>(find.byType(FilledButton));
      expect(btn.onPressed, isNull, reason: 'must stay disabled until the checkbox is ticked');
    });

    testWidgets(
        'C3: first press reveals 再次确认; commit gated a further 3s; fires timings with '
        'requires_second_press + gap>=3000 + total>=11000', (tester) async {
      ConfirmTimings? got;
      await tester.pumpWidget(_wrap(
        CountdownButton(seconds: 8, enabled: true, label: '确认', onPressed: (t) => got = t),
      ));

      // Stage 1: wait out the 8s read countdown, then press.
      for (var i = 0; i < 8; i++) {
        await tester.pump(const Duration(seconds: 1));
      }
      await tester.tap(find.byType(FilledButton));
      await tester.pump();

      // The 再次确认 bar appears and its commit is disabled (3s gate).
      expect(find.text('再次确认：'), findsOneWidget);
      final commit0 = tester.widget<FilledButton>(find.byKey(const Key('inv10_second_confirm')));
      expect(commit0.onPressed, isNull, reason: 'second press gated for 3s');

      // Before 3s elapse the commit must remain disabled.
      await tester.pump(const Duration(seconds: 2));
      final commit2 = tester.widget<FilledButton>(find.byKey(const Key('inv10_second_confirm')));
      expect(commit2.onPressed, isNull);

      // After the 3s gate, commit enables and firing it reports the timings.
      await tester.pump(const Duration(seconds: 1));
      final commit3 = tester.widget<FilledButton>(find.byKey(const Key('inv10_second_confirm')));
      expect(commit3.onPressed, isNotNull);
      await tester.tap(find.byKey(const Key('inv10_second_confirm')));
      await tester.pump();

      expect(got, isNotNull);
      expect(got!.requiresSecondPress, isTrue);
      expect(got!.secondPressGapMs, greaterThanOrEqualTo(3000));
      expect(got!.totalFlowMs, greaterThanOrEqualTo(11000));
    });
  });

  group('Gate confirm path requires checkbox + the two-stage cooldown', () {
    testWidgets('ticking the checkbox + 8s + second press fires the callback', (tester) async {
      ConfirmTimings? confirmed;
      await tester.pumpWidget(_wrap(
        HighRiskGate(
          scenario: HighRiskScenario.resignAdvice,
          onConfirmed: (t) => confirmed = t,
        ),
      ));

      // Before ticking, the first press stays disabled even after time passes.
      for (var i = 0; i < 8; i++) {
        await tester.pump(const Duration(seconds: 1));
      }
      var firstBtn = tester.widget<FilledButton>(find.byType(FilledButton));
      expect(firstBtn.onPressed, isNull);

      // Tick the read checkbox → first press enables (scroll it into view; the verbatim disclaimer
      // is long and the checkbox sits below the fold).
      await tester.ensureVisible(find.byType(CheckboxListTile));
      await tester.pump();
      await tester.tap(find.byType(CheckboxListTile));
      await tester.pump();
      firstBtn = tester.widget<FilledButton>(find.byType(FilledButton));
      expect(firstBtn.onPressed, isNotNull, reason: 'checkbox read + 8s elapsed → first press enabled');

      // First press reveals the second-confirm bar; commit after the 3s gate.
      await tester.tap(find.byType(FilledButton));
      await tester.pump();
      await tester.pump(const Duration(seconds: 3));
      await tester.tap(find.byKey(const Key('inv10_second_confirm')));
      await tester.pump();

      expect(confirmed, isNotNull);
      expect(confirmed!.totalFlowMs, greaterThanOrEqualTo(11000));
    });
  });
}
