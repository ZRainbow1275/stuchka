/// First-run wizard steps (ai/01 §1.6 / brief §5). Each `required` step must pass
/// [OnboardingValidator.validate] before "下一步" is allowed.
enum OnboardingStep {
  pickPrimary,
  pickSecondary,
  overseasOptIn,
  downloadLocalQwen,
  kbInitialSync,
}

extension OnboardingStepInfo on OnboardingStep {
  bool get required => switch (this) {
        OnboardingStep.pickPrimary => true,
        OnboardingStep.pickSecondary => true,
        OnboardingStep.overseasOptIn => false, // default off
        OnboardingStep.downloadLocalQwen => false, // optional 5GB
        OnboardingStep.kbInitialSync => true, // blocking
      };

  String get title => switch (this) {
        OnboardingStep.pickPrimary => '选择主用 AI 通道',
        OnboardingStep.pickSecondary => '选择备用 AI 通道',
        OnboardingStep.overseasOptIn => '境外云数据出境授权（默认关闭）',
        OnboardingStep.downloadLocalQwen => '下载本地 Qwen 模型（可选 · 约 5GB）',
        OnboardingStep.kbInitialSync => '初始化知识库（必需）',
      };
}

/// Validation errors (ai/01 §1.6).
enum OnboardingError {
  secondaryMustDiffer,
  overseasNeedsSignature,
  kbSyncIncomplete,
  primaryNotPicked,
}

extension OnboardingErrorInfo on OnboardingError {
  String get messageZh => switch (this) {
        OnboardingError.secondaryMustDiffer => '备用通道不能与主用通道相同',
        OnboardingError.overseasNeedsSignature => '开启境外云需完成文字签名"我同意数据出境"',
        OnboardingError.kbSyncIncomplete => '请先完成知识库初始化同步',
        OnboardingError.primaryNotPicked => '请先选择主用 AI 通道',
      };
}

/// The mutable wizard draft validated step-by-step.
class OnboardingDraft {
  OnboardingDraft({
    this.primary,
    this.secondary,
    this.overseasEnabled = false,
    this.overseasSignature,
    this.localQwenDownloaded = false,
    this.kbSynced = false,
  });

  /// Primary / secondary channel identifiers (e.g. provider ids from the OnboardingSpec).
  String? primary;
  String? secondary;
  bool overseasEnabled;
  String? overseasSignature;
  bool localQwenDownloaded;
  bool kbSynced;
}

/// Stateless validator (the source of the testable contract). `validate` returns null when the
/// step passes, else the first [OnboardingError].
class OnboardingValidator {
  const OnboardingValidator();

  OnboardingError? validate(OnboardingStep step, OnboardingDraft d) {
    switch (step) {
      case OnboardingStep.pickPrimary:
        if (d.primary == null || d.primary!.isEmpty) {
          return OnboardingError.primaryNotPicked;
        }
        return null;
      case OnboardingStep.pickSecondary:
        // primary must already be picked, and secondary must differ.
        if (d.primary == null || d.primary!.isEmpty) {
          return OnboardingError.primaryNotPicked;
        }
        if (d.secondary != null && d.secondary == d.primary) {
          return OnboardingError.secondaryMustDiffer;
        }
        return null;
      case OnboardingStep.overseasOptIn:
        // Optional step; only fails if enabled WITHOUT a text signature (C-B-5).
        if (d.overseasEnabled &&
            (d.overseasSignature == null || d.overseasSignature!.trim() != '我同意数据出境')) {
          return OnboardingError.overseasNeedsSignature;
        }
        return null;
      case OnboardingStep.downloadLocalQwen:
        return null; // always passable (optional)
      case OnboardingStep.kbInitialSync:
        if (!d.kbSynced) return OnboardingError.kbSyncIncomplete;
        return null;
    }
  }

  /// True iff the whole wizard is complete (all required steps pass).
  bool isComplete(OnboardingDraft d) {
    for (final step in OnboardingStep.values) {
      if (step.required && validate(step, d) != null) return false;
    }
    // overseas opt-in, if enabled, still needs its signature even though the step itself is optional
    return validate(OnboardingStep.overseasOptIn, d) == null;
  }
}
