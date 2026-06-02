import 'package:flutter_test/flutter_test.dart';
import 'package:stuchka/features/onboarding/onboarding_validator.dart';

void main() {
  const v = OnboardingValidator();

  group('OnboardingValidator (ai/01 §1.6)', () {
    test('pickPrimary fails when no primary chosen', () {
      expect(v.validate(OnboardingStep.pickPrimary, OnboardingDraft()),
          OnboardingError.primaryNotPicked);
      expect(v.validate(OnboardingStep.pickPrimary, OnboardingDraft(primary: 'deepseek')), isNull);
    });

    test('pickSecondary fails when secondary == primary (SecondaryMustDiffer)', () {
      final d = OnboardingDraft(primary: 'deepseek', secondary: 'deepseek');
      expect(v.validate(OnboardingStep.pickSecondary, d), OnboardingError.secondaryMustDiffer);
    });

    test('pickSecondary passes when secondary differs (or is null)', () {
      expect(
        v.validate(OnboardingStep.pickSecondary, OnboardingDraft(primary: 'deepseek', secondary: 'qwen')),
        isNull,
      );
      expect(
        v.validate(OnboardingStep.pickSecondary, OnboardingDraft(primary: 'deepseek')),
        isNull,
      );
    });

    test('overseasOptIn needs the exact signature when enabled', () {
      final noSig = OnboardingDraft(overseasEnabled: true);
      expect(v.validate(OnboardingStep.overseasOptIn, noSig),
          OnboardingError.overseasNeedsSignature);

      final wrongSig = OnboardingDraft(overseasEnabled: true, overseasSignature: '同意');
      expect(v.validate(OnboardingStep.overseasOptIn, wrongSig),
          OnboardingError.overseasNeedsSignature);

      final okSig = OnboardingDraft(overseasEnabled: true, overseasSignature: '我同意数据出境');
      expect(v.validate(OnboardingStep.overseasOptIn, okSig), isNull);

      // Disabled (default off) always passes.
      expect(v.validate(OnboardingStep.overseasOptIn, OnboardingDraft()), isNull);
    });

    test('downloadLocalQwen is optional (always passes)', () {
      expect(v.validate(OnboardingStep.downloadLocalQwen, OnboardingDraft()), isNull);
    });

    test('kbInitialSync is blocking until synced', () {
      expect(v.validate(OnboardingStep.kbInitialSync, OnboardingDraft()),
          OnboardingError.kbSyncIncomplete);
      expect(v.validate(OnboardingStep.kbInitialSync, OnboardingDraft(kbSynced: true)), isNull);
    });

    test('isComplete requires all required steps + primary != secondary', () {
      // primary == secondary blocks completion.
      final bad = OnboardingDraft(primary: 'a', secondary: 'a', kbSynced: true);
      expect(v.isComplete(bad), isFalse);

      final good = OnboardingDraft(primary: 'a', secondary: 'b', kbSynced: true);
      expect(v.isComplete(good), isTrue);

      // kb not synced blocks completion.
      final noKb = OnboardingDraft(primary: 'a', secondary: 'b');
      expect(v.isComplete(noKb), isFalse);

      // overseas enabled without signature blocks completion.
      final overseas = OnboardingDraft(
        primary: 'a',
        secondary: 'b',
        kbSynced: true,
        overseasEnabled: true,
      );
      expect(v.isComplete(overseas), isFalse);
    });

    test('required flags match the ai/01 §1.6 spec', () {
      expect(OnboardingStep.pickPrimary.required, isTrue);
      expect(OnboardingStep.pickSecondary.required, isTrue);
      expect(OnboardingStep.overseasOptIn.required, isFalse);
      expect(OnboardingStep.downloadLocalQwen.required, isFalse);
      expect(OnboardingStep.kbInitialSync.required, isTrue);
    });
  });
}
