import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../app/prefs.dart';

/// Three-tier accessibility floor #3: dark eye-care (spec 05 §5.4 / spec 01 §1.3).
/// Follows the system by default; user may switch to light/dark manually. NO time-of-day
/// auto-switching (forbidden — avoids screen jumps while editing公文 at night).
class ThemeModeController extends Notifier<ThemeMode> {
  @override
  ThemeMode build() {
    final saved = ref.read(prefsProvider).readString(StuchkaPrefs.kThemeMode);
    return switch (saved) {
      'light' => ThemeMode.light,
      'dark' => ThemeMode.dark,
      _ => ThemeMode.system,
    };
  }

  void setMode(ThemeMode mode) {
    state = mode;
    ref.read(prefsProvider).writeString(StuchkaPrefs.kThemeMode, mode.name);
  }
}

final themeModeProvider = NotifierProvider<ThemeModeController, ThemeMode>(
  ThemeModeController.new,
);
