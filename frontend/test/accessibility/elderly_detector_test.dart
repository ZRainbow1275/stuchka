import 'package:flutter_test/flutter_test.dart';
import 'package:stuchka/features/accessibility/elderly_detector.dart';

void main() {
  group('ElderlyDetector (spec 05 §5.5 — language-neutral)', () {
    test('identity trigger: RetiredRehired marks detected', () {
      var fired = false;
      final d = ElderlyDetector(onDetected: () => fired = true);
      d.observeIdentityIsRetiredRehired(true);
      expect(d.detected, isTrue);
      expect(fired, isTrue);
    });

    test('non-retired identity does not mark', () {
      final d = ElderlyDetector();
      d.observeIdentityIsRetiredRehired(false);
      expect(d.detected, isFalse);
    });

    test('timing trigger fires on the 3rd sample when mean > 2x reference median', () {
      final d = ElderlyDetector(referenceMedianMs: 10000);
      expect(d.observeTask(const TaskTiming(elapsedMs: 30000)), isFalse); // 1 sample
      expect(d.observeTask(const TaskTiming(elapsedMs: 30000)), isFalse); // 2 samples
      expect(d.observeTask(const TaskTiming(elapsedMs: 30000)), isTrue); // 3rd → 30k > 20k
      expect(d.detected, isTrue);
    });

    test('timing trigger does not fire when within 2x', () {
      final d = ElderlyDetector(referenceMedianMs: 10000);
      d.observeTask(const TaskTiming(elapsedMs: 12000));
      d.observeTask(const TaskTiming(elapsedMs: 12000));
      final fired = d.observeTask(const TaskTiming(elapsedMs: 12000)); // 12k < 20k
      expect(fired, isFalse);
      expect(d.detected, isFalse);
    });
  });
}
