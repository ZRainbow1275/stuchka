import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../app/prefs.dart';

/// Hard clamp bounds (spec 05 §5.2 / brief). > 1.50 breaks公文排版 + ProseMirror node rects;
/// < 0.85 is illegibly small. Both bounds are asserted, not silently coerced.
const double kMinTextScale = 0.85;
const double kMaxTextScale = 1.50;

/// Three-tier accessibility floor #1: font scaling (Riverpod 3 Notifier; spec 05 §5.2).
/// Default follows the system; a user override is clamped to [kMinTextScale, kMaxTextScale].
class TextScalerController extends Notifier<TextScaler> {
  @override
  TextScaler build() {
    final saved = ref.read(prefsProvider).readDouble(StuchkaPrefs.kTextScaleFactor);
    if (saved != null) {
      return TextScaler.linear(_clampFactor(saved));
    }
    return const TextScaler.linear(1.0);
  }

  /// Apply a user override. Out-of-range input trips an assert in debug and is clamped in release.
  void setUserOverride(double factor) {
    assert(
      factor >= kMinTextScale && factor <= kMaxTextScale,
      'text scale must be in [$kMinTextScale, $kMaxTextScale], got $factor',
    );
    final clamped = _clampFactor(factor);
    state = TextScaler.linear(clamped);
    ref.read(prefsProvider).writeDouble(StuchkaPrefs.kTextScaleFactor, clamped);
  }

  /// Reset to the system text scaler (drop the persisted override).
  void resetToSystem() {
    state = const TextScaler.linear(1.0);
    ref.read(prefsProvider).remove(StuchkaPrefs.kTextScaleFactor);
  }

  static double _clampFactor(double f) => f.clamp(kMinTextScale, kMaxTextScale);
}

final textScalerProvider = NotifierProvider<TextScalerController, TextScaler>(
  TextScalerController.new,
);
