import 'package:flutter_test/flutter_test.dart';
import 'package:stuchka/features/accessibility/high_risk/disclaimer_payload.dart';
import 'package:stuchka/features/accessibility/hotlines.dart';

/// C5: the §6 法援 / 工会 / 心理援助 resource list is complete + scenario-scoped (compliance/05 §6).
/// One shared source of truth — every render site reads from [LegalAidResources].
void main() {
  group('compliance/05 §6 resource completeness', () {
    test('every §6 code / web link is present in the shared list', () {
      final codes = LegalAidResources.all.map((h) => h.code).toSet();
      // Phone numbers.
      for (final c in ['12348', '12351', '12320', '12333', '12338', '12345']) {
        expect(codes, contains(c), reason: 'missing hotline $c');
      }
      // Web links (full URLs, not Markdown — compliance/05 §6 avoids PDF link loss).
      expect(codes, contains('https://www.acla.org.cn/'));
      expect(codes, contains('https://www.12348.gov.cn/'));
    });
  });

  group('per-scenario scoping (compliance/05 §6 适用 column)', () {
    test('every scenario gets 法援 + 工会 + the two 公益法援 web links', () {
      for (final s in HighRiskScenario.values) {
        final codes = LegalAidResources.forScenario(s).map((h) => h.code).toList();
        expect(codes, contains('12348'));
        expect(codes, contains('12351'));
        expect(codes, contains('https://www.acla.org.cn/'));
        expect(codes, contains('https://www.12348.gov.cn/'));
      }
    });

    test('S-05 criminal report adds 劳动监察 12333', () {
      final codes = LegalAidResources.forScenario(HighRiskScenario.criminalReportExport)
          .map((h) => h.code)
          .toList();
      expect(codes, contains('12333'));
    });

    test('S-06 protected scenario adds 妇联 12338 / 未成年 12345 / 监察 12333 / 心理 12320', () {
      final codes = LegalAidResources.forScenario(HighRiskScenario.protectedScenario)
          .map((h) => h.code)
          .toList();
      for (final c in ['12338', '12345', '12333', '12320']) {
        expect(codes, contains(c));
      }
    });

    test('INV-07 crisis hotlines lead with 心理援助 12320', () {
      expect(kCrisisHotlines.first.code, '12320');
      expect(kCrisisHotlines.map((h) => h.code), contains('12351'));
      expect(kCrisisHotlines.map((h) => h.code), contains('12348'));
    });
  });
}
