import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../app/prefs.dart';

/// Three-tier accessibility floor #2: simple mode (spec 05 §5.3). When on, the router redirects to
/// the three-big-button SimpleHomePage and the workbench hides the side panels + status bar.
class SimpleModeController extends Notifier<bool> {
  @override
  bool build() => ref.read(prefsProvider).readBool(StuchkaPrefs.kSimpleMode);

  void enable() => _set(true);
  void disable() => _set(false);
  void toggle() => _set(!state);

  /// Elderly detector "suggests" simple mode (non-forcing): we only enable if not already set off
  /// by the user this session. The suggestion is honoured by enabling here.
  void suggestEnable() => _set(true);

  void _set(bool v) {
    state = v;
    ref.read(prefsProvider).writeBool(StuchkaPrefs.kSimpleMode, v);
  }
}

final simpleModeProvider = NotifierProvider<SimpleModeController, bool>(
  SimpleModeController.new,
);
