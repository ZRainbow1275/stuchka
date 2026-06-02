import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'rust_core_handshake.dart';

/// Raised when the child process fails to announce READY within the timeout, exits early, or
/// emits an invalid handshake. The Dart shell surfaces "后端启动失败" on this.
class RustCoreLaunchException implements Exception {
  RustCoreLaunchException(this.message);
  final String message;
  @override
  String toString() => 'RustCoreLaunchException: $message';
}

/// Forks the `stuchka-core` child, reads the single `READY{...}` stdout line, and owns the child's
/// lifecycle (D1 · backend/01 §1.1). On the parent exiting it sends SIGTERM (Win: taskkill) then a
/// SIGKILL fallback after a grace period.
///
/// The launcher is process-backend agnostic: in tests a [ProcessSpawner] can stand in for a real
/// `Process.start`, letting the handshake contract be exercised against a mock subprocess that
/// emits a READY line (test/ipc/ipc_handshake_test.dart) as well as the real built binary.
typedef ProcessSpawner = Future<Process> Function(
  String executable,
  List<String> arguments, {
  String? workingDirectory,
  Map<String, String>? environment,
});

class RustCoreLauncher {
  RustCoreLauncher({
    required this.executablePath,
    this.arguments = const [],
    this.workingDirectory,
    this.environment,
    this.readyTimeout = const Duration(seconds: 10),
    this.shutdownGrace = const Duration(seconds: 2),
    ProcessSpawner? spawner,
  }) : _spawn = spawner ?? _defaultSpawn;

  /// Absolute path to the `stuchka-core` (`.exe` on Windows) binary, expected beside the Flutter
  /// executable.
  final String executablePath;
  final List<String> arguments;
  final String? workingDirectory;
  final Map<String, String>? environment;

  /// READY handshake timeout. backend/01 §1.1.2 allows up to 10s; the brief floor is 6s. We use
  /// 10s as the default ceiling and kill the child on miss.
  final Duration readyTimeout;

  /// Grace between SIGTERM and the SIGKILL fallback.
  final Duration shutdownGrace;

  final ProcessSpawner _spawn;

  Process? _process;
  RustCoreHandshake? _handshake;
  StreamSubscription<String>? _stdoutSub;
  StreamSubscription<String>? _stderrSub;
  final List<String> _stderrTail = [];

  Process? get process => _process;
  RustCoreHandshake? get handshake => _handshake;

  static Future<Process> _defaultSpawn(
    String executable,
    List<String> arguments, {
    String? workingDirectory,
    Map<String, String>? environment,
  }) {
    return Process.start(
      executable,
      arguments,
      workingDirectory: workingDirectory,
      environment: environment,
      // The READY line is on stdout; tracing goes to stderr — keep them separate.
      mode: ProcessStartMode.normal,
    );
  }

  /// Fork the child and resolve once the READY handshake is parsed. Kills the child and throws
  /// [RustCoreLaunchException] on timeout, early exit, or a malformed/`schema != 1` READY line.
  Future<RustCoreHandshake> spawn() async {
    if (_process != null) {
      throw StateError('RustCoreLauncher already spawned');
    }
    final Process proc;
    try {
      proc = await _spawn(
        executablePath,
        arguments,
        workingDirectory: workingDirectory,
        environment: environment,
      );
    } on ProcessException catch (e) {
      throw RustCoreLaunchException('无法启动后端进程: ${e.message}');
    }
    _process = proc;

    final readyCompleter = Completer<RustCoreHandshake>();

    _stdoutSub = proc.stdout
        .transform(utf8.decoder)
        .transform(const LineSplitter())
        .listen((line) {
      if (readyCompleter.isCompleted) return;
      try {
        final hs = RustCoreHandshake.parse(line);
        if (hs != null) {
          _handshake = hs;
          readyCompleter.complete(hs);
        }
      } on StateError catch (e) {
        readyCompleter.completeError(RustCoreLaunchException('握手无效: ${e.message}'));
      }
    });

    _stderrSub = proc.stderr
        .transform(utf8.decoder)
        .transform(const LineSplitter())
        .listen((line) {
      _stderrTail.add(line);
      if (_stderrTail.length > 50) _stderrTail.removeAt(0);
    });

    // Early-exit guard: if the child dies before READY, surface the failure with the stderr tail.
    unawaited(proc.exitCode.then((code) {
      if (!readyCompleter.isCompleted) {
        readyCompleter.completeError(
          RustCoreLaunchException(
            '后端进程在握手前退出（code $code）: ${_stderrTail.join(" / ")}',
          ),
        );
      }
    }));

    try {
      return await readyCompleter.future.timeout(
        readyTimeout,
        onTimeout: () {
          // Timeout → kill the child so it does not leak.
          _killNow();
          throw RustCoreLaunchException(
            '后端启动失败：${readyTimeout.inSeconds}s 内未收到 READY',
          );
        },
      );
    } catch (_) {
      // On any failure ensure the child is reaped.
      _killNow();
      rethrow;
    }
  }

  /// Graceful shutdown: SIGTERM (Win uses taskkill via [Process.kill] mapping), then SIGKILL after
  /// the grace window if the child is still alive.
  Future<void> shutdown() async {
    final proc = _process;
    if (proc == null) return;
    await _stdoutSub?.cancel();
    await _stderrSub?.cancel();

    // First, polite termination.
    proc.kill(ProcessSignal.sigterm);
    final exited = await proc.exitCode
        .timeout(shutdownGrace, onTimeout: () => -1)
        .then((c) => c)
        .catchError((_) => -1);
    if (exited == -1) {
      // Grace elapsed → hard kill.
      proc.kill(ProcessSignal.sigkill);
    }
    _process = null;
  }

  void _killNow() {
    final proc = _process;
    if (proc == null) return;
    _stdoutSub?.cancel();
    _stderrSub?.cancel();
    proc.kill(ProcessSignal.sigkill);
    _process = null;
  }
}
