import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:shared_preferences/shared_preferences.dart';

/// Thin typed wrapper over `shared_preferences` for `app_preferences` (brief §6.6: front-end local
/// state only — theme_mode / text_scale_factor / case_pane_ratio / onboarding done / cloud memory).
/// Main data lives in the Rust core over channel B; this never touches the主库.
class StuchkaPrefs {
  StuchkaPrefs(this._sp);
  final SharedPreferences _sp;

  static const kTextScaleFactor = 'text_scale_factor';
  static const kThemeMode = 'theme_mode';
  static const kCasePaneRatio = 'case_pane_ratio';
  static const kSimpleMode = 'simple_mode';
  static const kOnboardingDone = 'onboarding_done';

  double? readDouble(String key) => _sp.getDouble(key);
  Future<void> writeDouble(String key, double v) => _sp.setDouble(key, v);

  String? readString(String key) => _sp.getString(key);
  Future<void> writeString(String key, String v) => _sp.setString(key, v);

  bool readBool(String key, {bool fallback = false}) => _sp.getBool(key) ?? fallback;
  Future<void> writeBool(String key, bool v) => _sp.setBool(key, v);

  List<double>? readDoubleList(String key) =>
      _sp.getStringList(key)?.map((s) => double.tryParse(s) ?? 0).toList();
  Future<void> writeDoubleList(String key, List<double> v) =>
      _sp.setStringList(key, v.map((d) => d.toString()).toList());

  Future<void> remove(String key) => _sp.remove(key);
}

/// Overridden in `main.dart` once `SharedPreferences.getInstance()` resolves.
final prefsProvider = Provider<StuchkaPrefs>((ref) {
  throw UnimplementedError('prefsProvider must be overridden after SharedPreferences load');
});
