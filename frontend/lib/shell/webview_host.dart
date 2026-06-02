import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:webview_windows/webview_windows.dart';

import '../bridge/bridge_protocol.dart';
import 'editor_bridge_host.dart';
import 'webview_isolate_codec.dart';

/// The REAL Channel-A WebView2 host (FE02 §2.1/§2.4), backed by `webview_windows`.
///
/// Lifecycle: [initialize] creates the `WebviewController`, denies popups, injects the
/// `window.__STUCHKA_BOOT__` config (read by `assets/web/editor/boot.js` to call `StuchkaEditor.boot`)
/// BEFORE the page loads via `addScriptToExecuteOnDocumentCreated`, subscribes to `webMessage` (the
/// `window.chrome.webview.postMessage` channel), then loads the editor `index.html` from the bundled
/// Flutter asset over `file://`. The page's `<meta http-equiv="Content-Security-Policy" ... connect-src 'none'>`
/// (assets/web/editor/index.html) keeps Channel A off the network — steps reach the core only via
/// Channel B. Place the [Webview] widget for [controller] in the widget tree.
///
/// R1 is Windows-only (WebView2). Inbound envelopes are decoded via the [WebViewIsolateCodec] so a
/// large base64 Yjs update does not block the UI thread.
class WebViewEditorHost implements EditorBridgeHost {
  WebViewEditorHost({
    required this.caseId,
    required this.docId,
    required this.kbHash,
    WebViewIsolateCodec? codec,
  }) : _codec = codec ?? WebViewIsolateCodec();

  final String caseId;
  final String docId;
  final String kbHash;
  final WebViewIsolateCodec _codec;

  final WebviewController controller = WebviewController();
  final StreamController<BridgeEnvelope> _inbound = StreamController<BridgeEnvelope>.broadcast();
  StreamSubscription<dynamic>? _msgSub;
  bool _ready = false;

  bool get isReady => _ready;

  @override
  Stream<BridgeEnvelope> get inbound => _inbound.stream;

  /// Build, configure, and navigate the WebView2 to the editor bundle. Throws on a non-Windows host
  /// (WebView2 is the R1 surface) or any controller failure (never silently degrades).
  Future<void> initialize() async {
    if (!Platform.isWindows) {
      throw UnsupportedError('WebView2 editor host is Windows-only in R1 (FE02 §2.2)');
    }
    await controller.initialize();
    await controller.setPopupWindowPolicy(WebviewPopupWindowPolicy.deny);

    final bootJson = jsonEncode({'caseId': caseId, 'docId': docId, 'kbHash': kbHash});
    // Inject the boot config on EVERY document-created event BEFORE scripts run, so boot.js can read
    // window.__STUCHKA_BOOT__ and call StuchkaEditor.boot (no post-load executeScript needed -> IME safe).
    await controller.addScriptToExecuteOnDocumentCreated('window.__STUCHKA_BOOT__ = $bootJson;');

    _msgSub = controller.webMessage.listen(_onWebMessage, onError: _onWebError);
    await controller.loadUrl(editorIndexUrl());
    _ready = true;
  }

  Future<void> _onWebMessage(dynamic raw) async {
    try {
      // webview_windows delivers either the raw posted string or a decoded object; normalise to JSON.
      final str = raw is String ? raw : jsonEncode(raw);
      final env = await _codec.decode(str);
      _inbound.add(env);
    } catch (e) {
      // A4 downgrade / malformed envelope: surface, never silently mis-handle.
      if (!_inbound.isClosed) _inbound.addError(e);
    }
  }

  void _onWebError(Object error, StackTrace _) {
    if (!_inbound.isClosed) _inbound.addError(error);
  }

  @override
  Future<void> send(BridgeEnvelope envelope) => controller.postWebMessage(envelope.encode());

  /// `file://` URL of the editor `index.html` shipped under the Windows bundle's flutter_assets.
  static String editorIndexUrl() {
    final exeDir = File(Platform.resolvedExecutable).parent.path;
    final index = [
      exeDir,
      'data',
      'flutter_assets',
      'assets',
      'web',
      'editor',
      'index.html',
    ].join(Platform.pathSeparator);
    return Uri.file(index).toString();
  }

  @override
  Future<void> dispose() async {
    await _msgSub?.cancel();
    if (!_inbound.isClosed) await _inbound.close();
    await controller.dispose();
  }
}
