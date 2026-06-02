import '../bridge/bridge_protocol.dart';

/// The Channel-A host surface (FE02 §2.1). Abstracts the WebView2 transport so the bridge
/// dispatcher / tab router / IME observer / editor step-relay controller can be exercised headless
/// against an in-memory fake, while the real [WebViewEditorHost] (webview_windows) drives the live
/// WebView2. CSP `connect-src 'none'` means Channel A never reaches the Rust core directly — a Step
/// received here is relayed over Channel B (`POST /document/:id/steps`) by the editor controller.
abstract class EditorBridgeHost {
  /// Decoded envelopes arriving FROM the WebView editor (Channel A inbound).
  Stream<BridgeEnvelope> get inbound;

  /// Send an envelope TO the WebView editor (Channel A outbound).
  Future<void> send(BridgeEnvelope envelope);

  /// Tear down the transport + streams.
  Future<void> dispose();
}

int _bridgeIdCounter = 0;

/// Monotonic in-process correlation id for Dart-originated envelopes (idempotency key, NOT a
/// D9 persistence key). ASCII only, no Emoji.
String nextBridgeId() => 'dart-${_bridgeIdCounter++}';
