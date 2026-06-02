import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:stuchka/ipc/rust_core_handshake.dart';
import 'package:stuchka/ipc/rust_core_launcher.dart';

void main() {
  group('RustCoreHandshake.parse', () {
    test('parses the real backend READY line (port + token only)', () {
      // Verbatim shape from crates/core/src/spawn.rs::Handshake::ready_line().
      final token = 'ab' * 64; // 128 hex chars
      final line = 'READY{"port":54321,"token":"$token"}';
      final hs = RustCoreHandshake.parse(line);
      expect(hs, isNotNull);
      expect(hs!.port, 54321);
      expect(hs.token, token);
      expect(hs.schema, 1); // default when absent on the wire
      expect(hs.baseUrl, 'http://127.0.0.1:54321');
      expect(hs.wsBaseUrl, 'ws://127.0.0.1:54321');
    });

    test('returns null for a non-READY line', () {
      expect(RustCoreHandshake.parse('some log line'), isNull);
      expect(RustCoreHandshake.parse('{"port":1}'), isNull);
    });

    test('throws on malformed READY JSON', () {
      expect(() => RustCoreHandshake.parse('READY{not json'), throwsStateError);
    });

    test('throws when schema != 1', () {
      final token = 'cd' * 64;
      expect(
        () => RustCoreHandshake.parse('READY{"port":1,"token":"$token","schema":2}'),
        throwsStateError,
      );
    });

    test('throws on missing token', () {
      expect(() => RustCoreHandshake.parse('READY{"port":1}'), throwsStateError);
    });
  });

  group('RustCoreLauncher with a mock subprocess', () {
    test('resolves the handshake from a stdout READY line', () async {
      final launcher = RustCoreLauncher(
        executablePath: 'mock-core',
        spawner: (exe, args, {workingDirectory, environment}) async =>
            _MockProcess(readyAfter: Duration.zero),
      );
      final hs = await launcher.spawn();
      expect(hs.port, 40000);
      expect(hs.token.length, 128);
    });

    test('times out and kills the child when no READY arrives', () async {
      var killed = false;
      final launcher = RustCoreLauncher(
        executablePath: 'mock-core',
        readyTimeout: const Duration(milliseconds: 150),
        spawner: (exe, args, {workingDirectory, environment}) async =>
            _MockProcess(readyAfter: const Duration(seconds: 10), onKill: () => killed = true),
      );
      await expectLater(launcher.spawn(), throwsA(isA<RustCoreLaunchException>()));
      expect(killed, isTrue, reason: 'the child must be killed on a READY timeout');
    });

    test('surfaces an early exit before READY as a launch failure', () async {
      final launcher = RustCoreLauncher(
        executablePath: 'mock-core',
        spawner: (exe, args, {workingDirectory, environment}) async =>
            _MockProcess(readyAfter: null, exitImmediately: true),
      );
      await expectLater(launcher.spawn(), throwsA(isA<RustCoreLaunchException>()));
    });
  });
}

/// A mock [Process] that emits a READY line (or not) on stdout. Stands in for the real
/// `stuchka-core` so the handshake contract is testable without the built binary.
class _MockProcess implements Process {
  _MockProcess({this.readyAfter, this.exitImmediately = false, this.onKill});

  final Duration? readyAfter;
  final bool exitImmediately;
  final void Function()? onKill;

  final _stdout = StreamController<List<int>>();
  final _stderr = StreamController<List<int>>();
  final _exit = Completer<int>();

  bool _started = false;

  void _start() {
    if (_started) return;
    _started = true;
    if (exitImmediately) {
      _stderr.add(utf8.encode('boot failed\n'));
      _exit.complete(1);
      _stdout.close();
      _stderr.close();
      return;
    }
    if (readyAfter != null) {
      Future.delayed(readyAfter!, () {
        if (_stdout.isClosed) return;
        final token = 'ef' * 64;
        _stdout.add(utf8.encode('READY{"port":40000,"token":"$token"}\n'));
      });
    }
  }

  @override
  Stream<List<int>> get stdout {
    _start();
    return _stdout.stream;
  }

  @override
  Stream<List<int>> get stderr {
    _start();
    return _stderr.stream;
  }

  @override
  Future<int> get exitCode {
    _start();
    return _exit.future;
  }

  @override
  bool kill([ProcessSignal signal = ProcessSignal.sigterm]) {
    onKill?.call();
    if (!_exit.isCompleted) _exit.complete(-9);
    if (!_stdout.isClosed) _stdout.close();
    if (!_stderr.isClosed) _stderr.close();
    return true;
  }

  @override
  int get pid => 4242;

  @override
  IOSink get stdin => throw UnimplementedError();
}
