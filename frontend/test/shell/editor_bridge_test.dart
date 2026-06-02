import 'dart:ui' show Offset, Rect;

import 'package:flutter_test/flutter_test.dart';
import 'package:stuchka/bridge/bridge_dispatcher.dart';
import 'package:stuchka/bridge/bridge_ime_observer.dart';
import 'package:stuchka/bridge/bridge_protocol.dart';
import 'package:stuchka/shell/webview_isolate_codec.dart';
import 'package:stuchka/shell/webview_tab_router.dart';

import 'fake_editor_bridge_host.dart';

BridgeEnvelope env(BridgeMessageType type, Map<String, dynamic> payload) =>
    BridgeEnvelope(id: 'test-${type.wire}', type: type, payload: payload);

void main() {
  group('BridgeDispatcher fan-out (FE02 §2.1)', () {
    test('routes an inbound envelope to its per-type stream only', () async {
      final host = FakeEditorBridgeHost();
      final d = BridgeDispatcher(host);
      final steps = <BridgeEnvelope>[];
      final tabs = <BridgeEnvelope>[];
      d.on(BridgeMessageType.editorStep).listen(steps.add);
      d.on(BridgeMessageType.tabOpened).listen(tabs.add);

      host.emit(env(BridgeMessageType.editorStep, {'docId': 'd1'}));
      host.emit(env(BridgeMessageType.tabOpened, {'tabId': 't1'}));
      await Future<void>.delayed(Duration.zero);

      expect(steps.map((e) => e.type), [BridgeMessageType.editorStep]);
      expect(tabs.map((e) => e.type), [BridgeMessageType.tabOpened]);
      await d.dispose();
    });

    test('send forwards to the host', () async {
      final host = FakeEditorBridgeHost();
      final d = BridgeDispatcher(host);
      await d.send(env(BridgeMessageType.editorApplyRemote, {'updateBytes': 'AAAA'}));
      expect(host.sent.single.type, BridgeMessageType.editorApplyRemote);
      await d.dispose();
    });
  });

  group('WebViewTabRouter (FE02 §2.6 single-WebView, LRU<=8)', () {
    test('switchTo sends tab.switch and tracks open tabs', () async {
      final host = FakeEditorBridgeHost();
      final d = BridgeDispatcher(host);
      final router = WebViewTabRouter(d);
      await router.switchTo(tabId: 't1', docId: 'd1', role: 'editor');
      expect(host.sent.single.type, BridgeMessageType.tabSwitch);
      expect(host.sent.single.payload['docId'], 'd1');
      expect(router.openTabCount, 1);
      await router.dispose();
      await d.dispose();
    });

    test('evicts the least-recently-used tab past the cap and emits tab.close', () async {
      final host = FakeEditorBridgeHost();
      final d = BridgeDispatcher(host);
      final router = WebViewTabRouter(d, maxTabs: 2);
      await router.switchTo(tabId: 'a', docId: 'da', role: 'editor');
      await router.switchTo(tabId: 'b', docId: 'db', role: 'editor');
      await router.switchTo(tabId: 'a', docId: 'da', role: 'editor'); // re-touch a -> b is LRU
      await router.switchTo(tabId: 'c', docId: 'dc', role: 'editor'); // over cap -> evict b
      expect(router.openTabIds, containsAll(<String>['a', 'c']));
      expect(router.openTabIds, isNot(contains('b')));
      final closes = host.sent.where((e) => e.type == BridgeMessageType.tabClose).toList();
      expect(closes.single.payload['tabId'], 'b');
      await router.dispose();
      await d.dispose();
    });

    test('records editor-confirmed tab.opened', () async {
      final host = FakeEditorBridgeHost();
      final d = BridgeDispatcher(host);
      final router = WebViewTabRouter(d);
      host.emit(env(BridgeMessageType.tabOpened, {'tabId': 't9'}));
      await Future<void>.delayed(Duration.zero);
      expect(router.isConfirmed('t9'), isTrue);
      await router.dispose();
      await d.dispose();
    });
  });

  group('BridgeImeObserver (FE02 §2.7 IME caret anchoring)', () {
    test('translates editor-local caret by the WebView origin into a global rect', () async {
      final host = FakeEditorBridgeHost();
      final d = BridgeDispatcher(host);
      Rect? captured;
      final ime = BridgeImeObserver(
        d,
        webViewOrigin: () => const Offset(100, 200),
        setCaret: (r) => captured = r,
      );
      host.emit(env(BridgeMessageType.imeCompositionStart, {'x': 5, 'y': 6, 'height': 18}));
      await Future<void>.delayed(Duration.zero);
      expect(captured, const Rect.fromLTWH(105, 206, 1.0, 18));
      expect(ime.lastCaret, isNotNull);

      host.emit(env(BridgeMessageType.imeCompositionEnd, const {}));
      await Future<void>.delayed(Duration.zero);
      expect(ime.lastCaret, isNull);
      await ime.dispose();
      await d.dispose();
    });

    test('ignores a malformed composition payload (never guesses)', () async {
      final host = FakeEditorBridgeHost();
      final d = BridgeDispatcher(host);
      var calls = 0;
      final ime = BridgeImeObserver(
        d,
        webViewOrigin: () => Offset.zero,
        setCaret: (_) => calls++,
      );
      host.emit(env(BridgeMessageType.imeCompositionStart, {'x': 5})); // missing y/height
      await Future<void>.delayed(Duration.zero);
      expect(calls, 0);
      await ime.dispose();
      await d.dispose();
    });
  });

  group('WebViewIsolateCodec (FE02 §2.5 off-main-thread decode)', () {
    test('decodes a small envelope inline', () async {
      final codec = WebViewIsolateCodec();
      final raw = env(BridgeMessageType.editorStep, {'docId': 'd1'}).encode();
      expect(codec.wouldOffload(raw), isFalse);
      final decoded = await codec.decode(raw);
      expect(decoded.type, BridgeMessageType.editorStep);
      expect(decoded.payload['docId'], 'd1');
    });

    test('decodes a large envelope on a worker isolate', () async {
      final codec = WebViewIsolateCodec(offloadThresholdBytes: 1024);
      final big = 'A' * 4096; // simulate a large base64 Yjs update
      final raw = env(BridgeMessageType.editorLocalUpdate, {'updateBytes': big}).encode();
      expect(codec.wouldOffload(raw), isTrue);
      final decoded = await codec.decode(raw);
      expect(decoded.type, BridgeMessageType.editorLocalUpdate);
      expect(decoded.payload['updateBytes'], big);
    });
  });
}
