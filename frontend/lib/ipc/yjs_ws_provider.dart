import 'package:web_socket_channel/io.dart';
import 'package:web_socket_channel/web_socket_channel.dart';

import 'rust_core_handshake.dart';

/// Factory signature for opening a WS channel. Defaults to the `dart:io`-backed
/// [IOWebSocketChannel.connect] (R1 is Windows-desktop only, so `dart:io` is always present). A
/// seam is exposed so a unit test can inspect the request headers without opening a live socket.
typedef WsChannelFactory = WebSocketChannel Function(
  Uri uri, {
  Iterable<String>? protocols,
  Map<String, dynamic>? headers,
});

/// Channel-B Yjs sync socket factory (`ws://127.0.0.1:<port>/ws/sync/:doc_id`, Sync-1) and the
/// degrade/status socket (`/ws/status`).
///
/// The backend Bearer middleware (D1 · crates/api/src/ipc/auth.rs::require_bearer) fronts **all**
/// routes — including the WS upgrade for `/ws/sync/:doc_id` and `/ws/status`. It reads the token
/// ONLY from the `Authorization: Bearer <token>` request header (there is no `?token=` query path),
/// and additionally requires a loopback `Host` and a loopback-or-absent `Origin`
/// (anti DNS-rebinding / anti cross-site). The cross-platform
/// `WebSocketChannel.connect(uri, protocols: ...)` CANNOT set request headers, so it is rejected
/// 401 on every upgrade. We therefore use the VM-only [IOWebSocketChannel.connect], which forwards
/// the `headers` map to `dart:io`'s `WebSocket.connect` and so sets the `Authorization` header on
/// the HTTP upgrade. `dart:io` already sets `Host` to the connected loopback authority (passes the
/// loopback-host gate) and sends no `Origin` (the absent-Origin gate passes), so only the Bearer
/// header must be added here.
///
/// R1a wires the connection seam; the binary Yjs update framing belongs to the editor subtask
/// (03-editor-workbench). Here we expose a typed connector so the document route + status bar can
/// open the sockets with the correct loopback URL + Bearer header.
class YjsWsProvider {
  YjsWsProvider(this.handshake, {WsChannelFactory? channelFactory})
      : _channelFactory = channelFactory ?? _defaultChannelFactory;

  final RustCoreHandshake handshake;
  final WsChannelFactory _channelFactory;

  /// The default factory builds the real `dart:io`-backed channel with the request headers applied.
  static WebSocketChannel _defaultChannelFactory(
    Uri uri, {
    Iterable<String>? protocols,
    Map<String, dynamic>? headers,
  }) =>
      IOWebSocketChannel.connect(uri, protocols: protocols, headers: headers);

  /// The request headers required to pass the backend Bearer middleware on a WS upgrade.
  ///
  /// Exposed (and used by [connectSync] / [connectStatus]) so a unit test can assert the
  /// `Authorization: Bearer <token>` header is present without opening a live socket.
  Map<String, dynamic> upgradeHeaders() => {
        'Authorization': 'Bearer ${handshake.token}',
      };

  /// Open the Yjs sync socket for a document (`/ws/sync/:doc_id`, subprotocol `stuchka-sync-1`).
  WebSocketChannel connectSync(String docId) {
    final uri = Uri.parse('${handshake.wsBaseUrl}/ws/sync/$docId');
    return _channelFactory(
      uri,
      protocols: const ['stuchka-sync-1'],
      headers: upgradeHeaders(),
    );
  }

  /// Open the status socket (degrade level / KB version push + cross-border confirm).
  WebSocketChannel connectStatus() {
    final uri = Uri.parse('${handshake.wsBaseUrl}/ws/status');
    return _channelFactory(uri, headers: upgradeHeaders());
  }
}
