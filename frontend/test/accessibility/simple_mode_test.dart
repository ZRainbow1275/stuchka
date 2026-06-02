import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:stuchka/features/home/simple_home_page.dart';
import 'package:stuchka/theme/stuchka_icons.dart';
import 'package:stuchka/theme/stuchka_theme.dart';

void main() {
  group('SimpleBigButton sizing (spec 05 §5.3)', () {
    testWidgets('renders >= 96dp tall with a >= 22pt label, visible and tappable',
        (tester) async {
      var tapped = false;
      await tester.pumpWidget(
        MaterialApp(
          theme: StuchkaTheme.light(),
          home: Scaffold(
            body: SimpleBigButton(
              icon: StuchkaIcons.diagnose,
              label: '开始诊断',
              onTap: () => tapped = true,
            ),
          ),
        ),
      );

      expect(find.text('开始诊断'), findsOneWidget);

      // >= 96dp tall.
      final size = tester.getSize(find.byType(SimpleBigButton));
      expect(size.height, greaterThanOrEqualTo(SimpleBigButton.minHeight));
      expect(SimpleBigButton.minHeight, 96);

      // Label font size >= 22pt.
      final text = tester.widget<Text>(find.text('开始诊断'));
      expect(text.style?.fontSize, greaterThanOrEqualTo(22));
      expect(SimpleBigButton.labelSize, greaterThanOrEqualTo(22));

      // Tappable.
      await tester.tap(find.byType(SimpleBigButton));
      expect(tapped, isTrue);
    });

    testWidgets('disabled button is not tappable', (tester) async {
      var tapped = false;
      await tester.pumpWidget(
        MaterialApp(
          theme: StuchkaTheme.light(),
          home: Scaffold(
            body: SimpleBigButton(
              icon: StuchkaIcons.caseRoot,
              label: '继续案件',
              enabled: false,
              onTap: () => tapped = true,
            ),
          ),
        ),
      );
      await tester.tap(find.byType(SimpleBigButton));
      expect(tapped, isFalse);
    });
  });
}
