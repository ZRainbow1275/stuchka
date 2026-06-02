import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'app/prefs.dart';
import 'app/stuchka_app.dart';
import 'ipc/rust_core_handshake.dart';
import 'ipc/rust_core_launcher.dart';
import 'ipc/rust_core_providers.dart';

/// main(): fork the Rust core → read READY → ProviderScope override → runApp (D1 boot, brief §7).
///
/// The core binary `stuchka-core(.exe)` is expected beside the Flutter executable. If it cannot be
/// found or fails to hand-shake, a recoverable startup-error screen is shown instead of crashing
/// (the user can retry or quit) — the shell never falls back to a fake backend.
Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();

  final sp = await SharedPreferences.getInstance();
  final prefs = StuchkaPrefs(sp);

  await _boot(prefs);
}

Future<void> _boot(StuchkaPrefs prefs) async {
  final launcher = RustCoreLauncher(executablePath: _resolveCoreBinary());

  RustCoreHandshake? handshake;
  String? launchError;
  try {
    handshake = await launcher.spawn();
    // Self-check the backend port is accepting before showing the UI.
    final reachable = await _verifyReachable(handshake);
    if (!reachable) {
      launchError = '后端已启动但回环端口自检未通过';
    }
  } on RustCoreLaunchException catch (e) {
    launchError = e.message;
  } catch (e) {
    launchError = '$e';
  }

  // Ensure the child is reaped when the parent receives an interrupt (SIGTERM → SIGKILL fallback).
  ProcessSignal.sigint.watch().listen((_) async {
    await launcher.shutdown();
    exit(0);
  });

  if (handshake == null || launchError != null) {
    runApp(_StartupErrorApp(
      message: launchError ?? '后端启动失败',
      onRetry: () => _boot(prefs),
    ));
    return;
  }

  runApp(
    ProviderScope(
      overrides: [
        prefsProvider.overrideWithValue(prefs),
        rustCoreHandshakeProvider.overrideWithValue(handshake),
      ],
      child: const StuchkaApp(),
    ),
  );
}

Future<bool> _verifyReachable(RustCoreHandshake hs) async {
  try {
    final socket =
        await Socket.connect('127.0.0.1', hs.port, timeout: const Duration(seconds: 3));
    socket.destroy();
    return true;
  } catch (_) {
    return false;
  }
}

/// Resolve the `stuchka-core` binary path beside the Flutter executable, with dev fallbacks.
String _resolveCoreBinary() {
  final exeName = Platform.isWindows ? 'stuchka-core.exe' : 'stuchka-core';
  final beside = File(
    '${File(Platform.resolvedExecutable).parent.path}${Platform.pathSeparator}$exeName',
  );
  if (beside.existsSync()) return beside.path;

  // Dev fallback: the backend cargo target dir (debug build).
  final candidates = [
    '../backend/target/debug/$exeName',
    '../backend/target/release/$exeName',
  ];
  for (final c in candidates) {
    if (File(c).existsSync()) return File(c).absolute.path;
  }
  // Last resort: rely on PATH resolution.
  return exeName;
}

/// Minimal app shown when the backend cannot be started — recoverable, never a fake fallback.
class _StartupErrorApp extends StatelessWidget {
  const _StartupErrorApp({required this.message, required this.onRetry});
  final String message;
  final VoidCallback onRetry;

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      debugShowCheckedModeBanner: false,
      home: Scaffold(
        body: Center(
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 480),
            child: Column(
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                const Text('后端启动失败',
                    style: TextStyle(fontSize: 22, fontWeight: FontWeight.w700)),
                const SizedBox(height: 12),
                Padding(
                  padding: const EdgeInsets.symmetric(horizontal: 24),
                  child: Text(message, textAlign: TextAlign.center),
                ),
                const SizedBox(height: 20),
                FilledButton(onPressed: onRetry, child: const Text('重试')),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
