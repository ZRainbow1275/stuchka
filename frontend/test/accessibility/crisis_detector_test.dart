import 'package:flutter_test/flutter_test.dart';
import 'package:stuchka/features/accessibility/crisis/crisis_detector.dart';

void main() {
  const d = CrisisDetector();

  group('CrisisDetector levels (spec 05 §5.6.1)', () {
    test('light keywords', () {
      for (final k in CrisisDetector.lightKeywords) {
        expect(d.scan('今天$k了'), CrisisLevel.light, reason: k);
      }
    });

    test('mid keywords', () {
      for (final k in CrisisDetector.midKeywords) {
        expect(d.scan('我觉得$k'), CrisisLevel.mid, reason: k);
      }
    });

    test('severe keywords', () {
      for (final k in CrisisDetector.severeKeywords) {
        expect(d.scan('有时候$k'), CrisisLevel.severe, reason: k);
      }
    });

    test('severe wins precedence over lighter phrases', () {
      expect(d.scan('太累了，甚至想死'), CrisisLevel.severe);
      expect(d.scan('算了，不想活了'), CrisisLevel.severe);
    });

    test('false-positive / neutral set returns none', () {
      const neutral = [
        '我要申请劳动仲裁',
        '公司欠我三个月工资',
        '请帮我计算经济补偿',
        '证据已经准备好了',
        '',
      ];
      for (final t in neutral) {
        expect(d.scan(t), CrisisLevel.none, reason: t);
      }
    });
  });
}
