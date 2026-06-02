import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:stuchka/features/case/evidence/recording_consent_gate.dart';
import 'package:stuchka/features/case/evidence/scene_card.dart';
import 'package:stuchka/theme/stuchka_theme.dart';

void main() {
  group('extractScene (M8 real EXIF/GPS extraction)', () {
    test('non-image bytes yield identity-only meta with an honest "no EXIF" flag', () async {
      final bytes = utf8.encode('this is plainly not an image');
      final scene = await extractScene('note.txt', bytes);
      expect(scene.fileName, 'note.txt');
      expect(scene.byteSize, bytes.length);
      expect(scene.exifRead, isFalse); // never fabricates EXIF
      expect(scene.hasGps, isFalse);
      expect(scene.capturedAt, isNull);
    });

    test('malformed bytes never throw (import must not crash)', () async {
      final scene = await extractScene('broken.jpg', const [0xFF, 0xD8, 0x00, 0x01, 0x02]);
      expect(scene.fileName, 'broken.jpg');
      expect(scene.hasGps, isFalse);
    });
  });

  group('SceneCard (M8 取证信息卡)', () {
    testWidgets('shows file identity + sha256 + the honest no-GPS seam', (tester) async {
      const scene = SceneMeta(fileName: 'photo.jpg', byteSize: 2048, exifRead: false);
      await tester.pumpWidget(MaterialApp(
        theme: StuchkaTheme.light(),
        home: const Scaffold(body: SceneCard(scene: scene, sha256: 'abcd1234')),
      ));
      await tester.pumpAndSettle();
      expect(find.text('取证信息卡'), findsOneWidget);
      expect(find.textContaining('photo.jpg'), findsOneWidget);
      expect(find.textContaining('abcd1234'), findsOneWidget);
      // No fabricated coordinate — the seam is declared explicitly.
      expect(find.textContaining('未在该文件中读取到 EXIF'), findsOneWidget);
    });
  });

  group('RecordingConsentGate (法律 P5 录音合法性确认门)', () {
    testWidgets('confirm is disabled until the legality box is ticked, then returns true',
        (tester) async {
      bool? outcome;
      await tester.pumpWidget(MaterialApp(
        theme: StuchkaTheme.light(),
        home: Scaffold(
          body: Builder(
            builder: (context) => Center(
              child: ElevatedButton(
                onPressed: () async => outcome = await RecordingConsentGate.show(context),
                child: const Text('open'),
              ),
            ),
          ),
        ),
      ));
      await tester.tap(find.text('open'));
      await tester.pumpAndSettle();

      expect(find.textContaining('录音 / 录像合法性确认'), findsOneWidget);

      // Confirm button starts disabled.
      final confirm = tester.widget<FilledButton>(
        find.widgetWithText(FilledButton, '确认并导入'),
      );
      expect(confirm.onPressed, isNull);

      // Tick the legality checkbox -> confirm enables.
      await tester.tap(find.byType(Checkbox));
      await tester.pumpAndSettle();
      final confirm2 = tester.widget<FilledButton>(
        find.widgetWithText(FilledButton, '确认并导入'),
      );
      expect(confirm2.onPressed, isNotNull);

      await tester.tap(find.widgetWithText(FilledButton, '确认并导入'));
      await tester.pumpAndSettle();
      expect(outcome, isTrue);
    });

    testWidgets('cancel returns false (no fabricated consent)', (tester) async {
      bool? outcome;
      await tester.pumpWidget(MaterialApp(
        theme: StuchkaTheme.light(),
        home: Scaffold(
          body: Builder(
            builder: (context) => Center(
              child: ElevatedButton(
                onPressed: () async => outcome = await RecordingConsentGate.show(context),
                child: const Text('open'),
              ),
            ),
          ),
        ),
      ));
      await tester.tap(find.text('open'));
      await tester.pumpAndSettle();
      await tester.tap(find.widgetWithText(TextButton, '取消'));
      await tester.pumpAndSettle();
      expect(outcome, isFalse);
    });
  });
}
