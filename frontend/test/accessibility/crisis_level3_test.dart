import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:stuchka/features/accessibility/crisis/cooldown_providers.dart';
import 'package:stuchka/features/accessibility/crisis/cooldown_store.dart';
import 'package:stuchka/features/accessibility/crisis/crisis_detector.dart';
import 'package:stuchka/features/accessibility/crisis/crisis_level3_dialog.dart';
import 'package:stuchka/theme/stuchka_theme.dart';

/// INV-07 Level-3 (severe) 24h cooldown + misdetection appeal (compliance/05 §4.2 / §5.2 / §9.2).
void main() {
  setUp(() => CooldownStore.instance.clearAll());
  tearDown(() => CooldownStore.instance.clearAll());

  group('Level-3 copy (compliance/05 §4.2 / §4.5 / §9.2)', () {
    test('body is verbatim §4.2 Level-3 and language-neutral (no 诊断/判定)', () {
      expect(kCrisisLevel3Body, contains('您的安全是最重要的'));
      expect(kCrisisLevel3Body, contains('12320'));
      expect(kCrisisLevel3Body, contains('010-82951332'));
      expect(kCrisisLevel3Body, contains('24 小时内将暂停'));
      expect(kCrisisLevel3Body, contains('申诉误判'));
      // §9.2 language-neutrality: zero 诊断 / 判定.
      expect(RegExp('诊断|判定').allMatches(kCrisisLevel3Body).length, 0);
    });

    test('severe detector routes 想死/自残 to severe (not mid)', () {
      const d = CrisisDetector();
      expect(d.scan('我不想活了'), CrisisLevel.severe);
      expect(d.scan('想死'), CrisisLevel.severe);
      expect(d.scan('觉得没意义'), CrisisLevel.mid);
    });
  });

  group('Level3CooldownGate + appeal (compliance/05 §5.2 / §5.3)', () {
    testWidgets('begin24h locks high-risk; the gate banner shows; appeal releases locally',
        (tester) async {
      final container = ProviderContainer();

      await tester.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: MaterialApp(
            theme: StuchkaTheme.light(),
            home: const Scaffold(body: Level3CooldownGate()),
          ),
        ),
      );

      // Inactive → renders nothing.
      expect(find.textContaining('冷静期生效中'), findsNothing);

      // Begin the 24h cooldown (what CrisisLevel3Dialog does on accept).
      container.read(cooldownProvider(kHighRiskFeature).notifier).begin24h();
      await tester.pump();
      expect(container.read(cooldownProvider(kHighRiskFeature)).active, isTrue);
      expect(find.textContaining('冷静期生效中'), findsOneWidget);
      expect(find.textContaining('申诉误判'), findsOneWidget);

      // Appeal releases the cooldown locally with no approval (用户即审计员).
      await tester.tap(find.textContaining('申诉误判'));
      await tester.pump();
      expect(container.read(cooldownProvider(kHighRiskFeature)).active, isFalse);
      expect(container.read(appealProvider).length, 1);
      expect(find.textContaining('冷静期生效中'), findsNothing);

      // Tear down the widget tree (so the gate stops watching) then dispose the container,
      // cancelling the cooldown ticker (onDispose) before the timers-pending invariant check.
      await tester.pumpWidget(const SizedBox.shrink());
      container.dispose();
    });

    testWidgets('Level-3 dialog renders the bottom hotline disclaimer + 申诉误判 entry',
        (tester) async {
      await tester.pumpWidget(
        ProviderScope(
          child: MaterialApp(
            theme: StuchkaTheme.light(),
            home: Consumer(
              builder: (context, ref, _) => Scaffold(
                body: Center(
                  child: ElevatedButton(
                    onPressed: () => CrisisLevel3Dialog.show(context, ref),
                    child: const Text('open'),
                  ),
                ),
              ),
            ),
          ),
        ),
      );
      await tester.tap(find.text('open'));
      await tester.pumpAndSettle();

      // §4.5 bottom disclaimer present + the R1-必死 appeal entry + accept button.
      expect(find.textContaining('与本系统无任何合作关系'), findsOneWidget);
      expect(find.text('申诉误判'), findsOneWidget);
      expect(find.text('我已拨打热线'), findsOneWidget);
    });
  });
}
