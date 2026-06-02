import 'dart:async';

import 'package:stuchka/bridge/bridge_protocol.dart';
import 'package:stuchka/shell/editor_bridge_host.dart';

/// In-memory [EditorBridgeHost] for headless tests. `emit` injects an inbound envelope (as if from
/// the WebView editor); `sent` records every outbound envelope (as if posted to the WebView). This
/// is a transport test-double for the unrenderable WebView2 surface — NOT a mock of any business
/// logic (the dispatcher / tab router / IME observer / step-relay controller under test run for
/// real, and the step relay hits the real RustCoreClient over a recording HTTP adapter).
class FakeEditorBridgeHost implements EditorBridgeHost {
  final StreamController<BridgeEnvelope> _inbound = StreamController<BridgeEnvelope>.broadcast();
  final List<BridgeEnvelope> sent = [];
  bool disposed = false;

  @override
  Stream<BridgeEnvelope> get inbound => _inbound.stream;

  @override
  Future<void> send(BridgeEnvelope envelope) async {
    sent.add(envelope);
  }

  /// Inject an envelope as if it arrived from the editor.
  void emit(BridgeEnvelope envelope) => _inbound.add(envelope);

  @override
  Future<void> dispose() async {
    disposed = true;
    await _inbound.close();
  }
}
