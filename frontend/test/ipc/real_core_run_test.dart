import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:stuchka/ipc/rust_core_client.dart';
import 'package:stuchka/ipc/rust_core_launcher.dart';

/// Live IPC end-to-end test against the REAL built `stuchka-core` binary (D1).
///
/// Gated on the binary existing under the backend cargo target dir; when absent (CI without a Rust
/// build) the test is skipped rather than failing. When present it:
///   1. forks the child + reads the real READY line;
///   2. self-checks `GET /health` over the Bearer HTTP channel;
///   3. asserts a missing/garbage Bearer token is rejected (E_INVALID_TOKEN / 401);
///   4. cleanly shuts the child down.
void main() {
  final exeName = Platform.isWindows ? 'stuchka-core.exe' : 'stuchka-core';
  String? binPath;
  for (final c in [
    '../backend/target/debug/$exeName',
    '../backend/target/release/$exeName',
  ]) {
    if (File(c).existsSync()) {
      binPath = File(c).absolute.path;
      break;
    }
  }

  test('real stuchka-core handshake + /health + Bearer rejection', () async {
    final launcher = RustCoreLauncher(
      executablePath: binPath!,
      workingDirectory: File(binPath).parent.path,
    );
    final hs = await launcher.spawn();
    expect(hs.token.length, 128);
    expect(hs.port, greaterThan(0));

    try {
      final client = RustCoreClient(hs);
      final health = await client.health();
      expect(health.isReady, isTrue);

      // A wrong Bearer token must be rejected (auth_bearer contract).
      final badHs = hs;
      final badClient = RustCoreClient(badHs);
      badClient.raw.options.headers['Authorization'] = 'Bearer deadbeef';
      await expectLater(
        badClient.health(),
        throwsA(isA<RustCoreApiException>().having((e) => e.code, 'code', 'E_INVALID_TOKEN')),
      );
    } finally {
      await launcher.shutdown();
    }
  }, skip: binPath == null ? 'stuchka-core binary not built (run cargo build -p api)' : false);
}
