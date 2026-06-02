import 'package:flutter_test/flutter_test.dart';
import 'package:stuchka/features/accessibility/crisis/crisis_detector.dart';
import 'package:stuchka/features/accessibility/crisis/emotion_trend.dart';

void main() {
  // Fixed base instant (no DateTime.now() in assertions — deterministic).
  final t0 = DateTime.utc(2026, 1, 1, 9);
  DateTime at(int i) => t0.add(Duration(minutes: i));

  group('EmotionTrend (M11 历史会话情绪)', () {
    test('a single concerning turn is not yet "sustained"', () {
      final tr = EmotionTrend();
      tr.record(CrisisLevel.light, at(0));
      expect(tr.sustainedConcern, isFalse);
      expect(tr.recentPeak, CrisisLevel.light);
    });

    test('three concerning turns in the window read as sustained concern', () {
      final tr = EmotionTrend();
      tr.record(CrisisLevel.light, at(0));
      tr.record(CrisisLevel.light, at(1));
      tr.record(CrisisLevel.mid, at(2));
      expect(tr.sustainedConcern, isTrue);
      expect(tr.recentPeak, CrisisLevel.mid);
    });

    test('sustained concern persists even when the LATEST turn is calm (the M11 point)', () {
      final tr = EmotionTrend();
      tr.record(CrisisLevel.light, at(0));
      tr.record(CrisisLevel.mid, at(1));
      tr.record(CrisisLevel.light, at(2));
      tr.record(CrisisLevel.none, at(3)); // one neutral message...
      // ...but the recent window still holds 3 concerning signals -> support should persist.
      expect(tr.sustainedConcern, isTrue);
    });

    test('an all-calm session never triggers concern', () {
      final tr = EmotionTrend();
      for (var i = 0; i < 6; i++) {
        tr.record(CrisisLevel.none, at(i));
      }
      expect(tr.sustainedConcern, isFalse);
      expect(tr.recentPeak, CrisisLevel.none);
    });

    test('concern fades once it ages out of the recent window', () {
      final tr = EmotionTrend(window: 5, sustainThreshold: 3);
      // Three concerning turns early on...
      tr.record(CrisisLevel.light, at(0));
      tr.record(CrisisLevel.light, at(1));
      tr.record(CrisisLevel.light, at(2));
      expect(tr.sustainedConcern, isTrue);
      // ...then five calm turns push them out of the 5-sample window.
      for (var i = 3; i < 8; i++) {
        tr.record(CrisisLevel.none, at(i));
      }
      expect(tr.sustainedConcern, isFalse, reason: 'old concern ages out of the window');
    });

    test('clear drops the session trajectory (never persisted)', () {
      final tr = EmotionTrend();
      tr.record(CrisisLevel.mid, at(0));
      tr.clear();
      expect(tr.samples, isEmpty);
      expect(tr.sustainedConcern, isFalse);
    });
  });
}
