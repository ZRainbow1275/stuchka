import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../features/accessibility/text_scaler_controller.dart';
import '../features/accessibility/theme_mode_controller.dart';
import '../theme/stuchka_theme.dart';
import 'router.dart';

/// Root MaterialApp.router. Wraps the whole tree in a MediaQuery that applies the clamped
/// textScaler (spec 01 §1.5 / spec 05 §5.2) and follows the theme-mode controller.
class StuchkaApp extends ConsumerWidget {
  const StuchkaApp({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final router = ref.watch(routerProvider);
    final themeMode = ref.watch(themeModeProvider);

    return MaterialApp.router(
      title: 'Stučka',
      debugShowCheckedModeBanner: false,
      theme: StuchkaTheme.light(),
      darkTheme: StuchkaTheme.dark(),
      themeMode: themeMode,
      routerConfig: router,
      localizationsDelegates: const [
        GlobalMaterialLocalizations.delegate,
        GlobalWidgetsLocalizations.delegate,
        GlobalCupertinoLocalizations.delegate,
      ],
      supportedLocales: const [Locale('zh', 'CN'), Locale('en', 'US')],
      builder: (context, child) {
        final scaler = ref.watch(textScalerProvider);
        return MediaQuery(
          data: MediaQuery.of(context).copyWith(textScaler: scaler),
          child: child!,
        );
      },
    );
  }
}
