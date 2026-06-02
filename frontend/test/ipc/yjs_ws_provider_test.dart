import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:stuchka/ipc/rust_core_handshake.dart';
import 'package:stuchka/ipc/yjs_ws_provider.dart';
import 'package:web_socket/web_socket.dart';
import 'package:web_socket_channel/adapter_web_socket_channel.dart';
import 'package:web_socket_channel/web_socket_channel.dart';

/// S1 regression guard: the backend Bearer middleware (crates/api/src/ipc/auth.rs::require_bearer)
/// fronts the WS upgrade and reads the token ONLY from the `Authorization: Bearer <token>` header.
/// The cross-platform `WebSocketChannel.connect` cannot set headers, so the provider MUST build the
/// channel via the `dart:io` path with the Authorization header present. We inject a capturing
/// factory (the seam) so the headers are asserted without opening a live socket.
void main() {
  final token = 'ab' * 64; // 128 hex chars, like the real READY token
  final hs = RustCoreHandshake(port: 54321, token: token);

  group('YjsWsProvider sets the Bearer header on the WS upgrade', () {
    test('connectSync passes Authorization: Bearer <token> + the sync subprotocol', () {
      Uri? capturedUri;
      Iterable<String>? capturedProtocols;
      Map<String, dynamic>? capturedHeaders;

      final provider = YjsWsProvider(
        hs,
        channelFactory: (uri, {protocols, headers}) {
          capturedUri = uri;
          capturedProtocols = protocols;
          capturedHeaders = headers;
          return _pendingChannel();
        },
      );

      provider.connectSync('doc-123');

      expect(capturedUri.toString(), 'ws://127.0.0.1:54321/ws/sync/doc-123');
      expect(capturedProtocols, contains('stuchka-sync-1'));
      expect(capturedHeaders, isNotNull);
      expect(capturedHeaders!['Authorization'], 'Bearer $token');
    });

    test('connectStatus passes Authorization: Bearer <token>', () {
      Uri? capturedUri;
      Map<String, dynamic>? capturedHeaders;

      final provider = YjsWsProvider(
        hs,
        channelFactory: (uri, {protocols, headers}) {
          capturedUri = uri;
          capturedHeaders = headers;
          return _pendingChannel();
        },
      );

      provider.connectStatus();

      expect(capturedUri.toString(), 'ws://127.0.0.1:54321/ws/status');
      expect(capturedHeaders, isNotNull);
      expect(capturedHeaders!['Authorization'], 'Bearer $token');
    });

    test('upgradeHeaders exposes exactly the Bearer header', () {
      final provider = YjsWsProvider(hs);
      final headers = provider.upgradeHeaders();
      expect(headers, {'Authorization': 'Bearer $token'});
    });
  });
}

/// Returns a REAL [WebSocketChannel] (the package's own [AdapterWebSocketChannel]) whose underlying
/// [WebSocket] never completes — so no live socket is opened, yet the seam gets a genuine channel
/// instance back (not a mock). This is exactly the shape `connect` returns synchronously while its
/// connection is still pending.
WebSocketChannel _pendingChannel() =>
    AdapterWebSocketChannel(Completer<WebSocket>().future);
