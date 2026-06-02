import 'dart:async';
import 'dart:ui' show Offset, Rect;

import 'bridge_dispatcher.dart';
import 'bridge_protocol.dart';

/// Applies a composition caret rectangle in GLOBAL (Flutter window) coordinates. The real wiring
/// passes the platform sink (TextInput.setEditableSizeAndTransform / setCaretRect equivalents); tests
/// inject a recorder.
typedef CaretRectSink = void Function(Rect globalCaret);

/// Bridges the WebView editor's IME composition events to the Flutter text-input layer so the
/// native IME candidate window anchors to the caret even though text lives inside the WebView
/// (FE02 §2.7). The editor emits `ime.compositionStart {x, y, height}` in WebView-local pixels;
/// this observer translates by the WebView's global origin and forwards a global caret rect, then
/// clears it on `ime.compositionEnd`.
class BridgeImeObserver {
  BridgeImeObserver(
    this._dispatcher, {
    required this.webViewOrigin,
    required this.setCaret,
  }) {
    _startSub = _dispatcher.on(BridgeMessageType.imeCompositionStart).listen(_onStart);
    _endSub = _dispatcher.on(BridgeMessageType.imeCompositionEnd).listen(_onEnd);
  }

  final BridgeDispatcher _dispatcher;

  /// The WebView host's global top-left offset (editor-local px -> global px).
  final Offset Function() webViewOrigin;
  final CaretRectSink setCaret;

  late final StreamSubscription<BridgeEnvelope> _startSub;
  late final StreamSubscription<BridgeEnvelope> _endSub;

  Rect? _lastCaret;
  Rect? get lastCaret => _lastCaret;

  void _onStart(BridgeEnvelope env) {
    final p = env.payload;
    final x = (p['x'] as num?)?.toDouble();
    final y = (p['y'] as num?)?.toDouble();
    final h = (p['height'] as num?)?.toDouble();
    if (x == null || y == null || h == null) return; // malformed -> ignore, never guess
    final origin = webViewOrigin();
    final caret = Rect.fromLTWH(origin.dx + x, origin.dy + y, 1.0, h);
    _lastCaret = caret;
    setCaret(caret);
  }

  void _onEnd(BridgeEnvelope env) {
    _lastCaret = null;
  }

  Future<void> dispose() async {
    await _startSub.cancel();
    await _endSub.cancel();
  }
}
