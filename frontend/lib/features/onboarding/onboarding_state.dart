import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'onboarding_validator.dart';

/// Mutable onboarding draft state (Riverpod 3 Notifier).
class OnboardingController extends Notifier<OnboardingDraft> {
  final OnboardingValidator validator = const OnboardingValidator();

  @override
  OnboardingDraft build() => OnboardingDraft();

  void setPrimary(String? v) {
    state = OnboardingDraft(
      primary: v,
      secondary: state.secondary,
      overseasEnabled: state.overseasEnabled,
      overseasSignature: state.overseasSignature,
      localQwenDownloaded: state.localQwenDownloaded,
      kbSynced: state.kbSynced,
    );
  }

  void setSecondary(String? v) {
    state = OnboardingDraft(
      primary: state.primary,
      secondary: v,
      overseasEnabled: state.overseasEnabled,
      overseasSignature: state.overseasSignature,
      localQwenDownloaded: state.localQwenDownloaded,
      kbSynced: state.kbSynced,
    );
  }

  void setOverseas(bool enabled, {String? signature}) {
    state = OnboardingDraft(
      primary: state.primary,
      secondary: state.secondary,
      overseasEnabled: enabled,
      overseasSignature: signature ?? state.overseasSignature,
      localQwenDownloaded: state.localQwenDownloaded,
      kbSynced: state.kbSynced,
    );
  }

  void setLocalQwen(bool downloaded) {
    state = OnboardingDraft(
      primary: state.primary,
      secondary: state.secondary,
      overseasEnabled: state.overseasEnabled,
      overseasSignature: state.overseasSignature,
      localQwenDownloaded: downloaded,
      kbSynced: state.kbSynced,
    );
  }

  void setKbSynced(bool synced) {
    state = OnboardingDraft(
      primary: state.primary,
      secondary: state.secondary,
      overseasEnabled: state.overseasEnabled,
      overseasSignature: state.overseasSignature,
      localQwenDownloaded: state.localQwenDownloaded,
      kbSynced: synced,
    );
  }

  OnboardingError? validate(OnboardingStep step) => validator.validate(step, state);
  bool get isComplete => validator.isComplete(state);
}

final onboardingControllerProvider =
    NotifierProvider<OnboardingController, OnboardingDraft>(OnboardingController.new);
