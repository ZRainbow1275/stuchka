import 'dart:async';

import '../bridge/bridge_dispatcher.dart';
import '../bridge/bridge_protocol.dart';
import 'editor_bridge_host.dart';

/// Dart-side single-WebView tab router (FE02 §2.6): the shell hosts ONE WebView and multiplexes
/// document tabs inside it. It maps `tabId -> (docId, role)`, caps the open set at [maxTabs] with LRU
/// eviction, sends `tab.switch` / `tab.close` envelopes to the editor, and records editor-confirmed
/// `tab.opened` acks. (The editor side mirrors this in `editor/src/tab_router.ts`.)
class WebViewTabRouter {
  WebViewTabRouter(this._dispatcher, {this.maxTabs = 8}) {
    _openedSub = _dispatcher.on(BridgeMessageType.tabOpened).listen(_onOpened);
  }

  final BridgeDispatcher _dispatcher;
  final int maxTabs;
  late final StreamSubscription<BridgeEnvelope> _openedSub;

  /// tabId -> docId/role, in LRU order (insertion order of a LinkedHashMap; re-touched on switch).
  final Map<String, ({String docId, String role})> _tabs = {};
  final Set<String> _confirmed = {};

  int get openTabCount => _tabs.length;
  bool isConfirmed(String tabId) => _confirmed.contains(tabId);
  Iterable<String> get openTabIds => _tabs.keys;

  /// Activate (or open) a document tab. Re-touches LRU order, evicts the least-recent over [maxTabs],
  /// and tells the editor to switch.
  Future<void> switchTo({
    required String tabId,
    required String docId,
    required String role,
  }) async {
    // Re-touch: remove + re-insert so this tab becomes most-recently-used (tail).
    _tabs.remove(tabId);
    _tabs[tabId] = (docId: docId, role: role);
    await _evictIfNeeded();
    await _dispatcher.send(BridgeEnvelope(
      id: nextBridgeId(),
      type: BridgeMessageType.tabSwitch,
      payload: {'tabId': tabId, 'docId': docId, 'role': role},
    ));
  }

  /// Close a tab and tell the editor.
  Future<void> closeTab(String tabId) async {
    _tabs.remove(tabId);
    _confirmed.remove(tabId);
    await _dispatcher.send(BridgeEnvelope(
      id: nextBridgeId(),
      type: BridgeMessageType.tabClose,
      payload: {'tabId': tabId},
    ));
  }

  Future<void> _evictIfNeeded() async {
    while (_tabs.length > maxTabs) {
      // Least-recently-used = first key in insertion order (and not the just-touched tail).
      final victim = _tabs.keys.first;
      await closeTab(victim);
    }
  }

  void _onOpened(BridgeEnvelope env) {
    final tabId = env.payload['tabId'];
    if (tabId is String) _confirmed.add(tabId);
  }

  Future<void> dispose() => _openedSub.cancel();
}
