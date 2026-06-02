import 'dart:async';

import '../shell/editor_bridge_host.dart';
import 'bridge_protocol.dart';

/// Main-thread fan-out of Channel-A inbound envelopes into per-type streams (FE02 §2.1
/// bridge_dispatcher). Riverpod controllers / the editor step-relay subscribe via [on]; outbound
/// sends go through [send]. Unknown / schema-mismatched envelopes never arrive here — the host's
/// decode (BridgeEnvelope.decode) already throws on them (A4 downgrade guard).
class BridgeDispatcher {
  BridgeDispatcher(this._host) {
    _sub = _host.inbound.listen(_route, onError: _onError);
  }

  final EditorBridgeHost _host;
  late final StreamSubscription<BridgeEnvelope> _sub;
  final Map<BridgeMessageType, StreamController<BridgeEnvelope>> _byType = {};
  final StreamController<Object> _errors = StreamController<Object>.broadcast();

  /// Broadcast stream of envelopes of a single [type].
  Stream<BridgeEnvelope> on(BridgeMessageType type) => _byType
      .putIfAbsent(type, () => StreamController<BridgeEnvelope>.broadcast())
      .stream;

  /// Decode/transport errors surfaced by the host (e.g. an A4 schema-version drop).
  Stream<Object> get errors => _errors.stream;

  void _route(BridgeEnvelope env) {
    final controller = _byType[env.type];
    if (controller != null && controller.hasListener) {
      controller.add(env);
    }
  }

  void _onError(Object error, StackTrace _) {
    if (_errors.hasListener) _errors.add(error);
  }

  /// Send an envelope to the editor.
  Future<void> send(BridgeEnvelope env) => _host.send(env);

  Future<void> dispose() async {
    await _sub.cancel();
    for (final c in _byType.values) {
      await c.close();
    }
    await _errors.close();
  }
}
