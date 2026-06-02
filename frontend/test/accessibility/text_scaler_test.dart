import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:stuchka/app/prefs.dart';
import 'package:stuchka/features/accessibility/text_scaler_controller.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  Future<ProviderContainer> makeContainer() async {
    SharedPreferences.setMockInitialValues({});
    final sp = await SharedPreferences.getInstance();
    return ProviderContainer(overrides: [prefsProvider.overrideWithValue(StuchkaPrefs(sp))]);
  }

  test('default is 1.0 linear scaler', () async {
    final c = await makeContainer();
    final scaler = c.read(textScalerProvider);
    expect(scaler.scale(10), 10.0);
  });

  test('accepts the four canonical factors 0.85 / 1.0 / 1.2 / 1.5', () async {
    final c = await makeContainer();
    for (final f in [0.85, 1.0, 1.2, 1.5]) {
      c.read(textScalerProvider.notifier).setUserOverride(f);
      expect(c.read(textScalerProvider).scale(100), closeTo(100 * f, 1e-6));
    }
  });

  test('asserts out-of-range overrides (>1.5 / <0.85)', () async {
    final c = await makeContainer();
    expect(() => c.read(textScalerProvider.notifier).setUserOverride(1.6), throwsA(isA<AssertionError>()));
    expect(() => c.read(textScalerProvider.notifier).setUserOverride(0.5), throwsA(isA<AssertionError>()));
  });

  test('bounds constants are 0.85 and 1.50', () {
    expect(kMinTextScale, 0.85);
    expect(kMaxTextScale, 1.50);
  });

  test('resetToSystem returns to 1.0 and drops the persisted override', () async {
    final c = await makeContainer();
    c.read(textScalerProvider.notifier).setUserOverride(1.2);
    c.read(textScalerProvider.notifier).resetToSystem();
    expect(c.read(textScalerProvider).scale(10), 10.0);
  });
}
