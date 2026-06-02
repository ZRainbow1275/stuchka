import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'rust_core_client.dart';
import 'rust_core_handshake.dart';
import 'yjs_ws_provider.dart';

/// The READY handshake, injected at `runApp` via a ProviderScope override once `main.dart` has
/// forked the core and parsed the READY line. Reading it before injection is a programming error.
final rustCoreHandshakeProvider = Provider<RustCoreHandshake>((ref) {
  throw UnimplementedError(
    'rustCoreHandshakeProvider must be overridden in ProviderScope after the READY handshake',
  );
});

/// The channel-B HTTP client (Bearer injected from the handshake).
final rustCoreClientProvider = Provider<RustCoreClient>((ref) {
  final hs = ref.watch(rustCoreHandshakeProvider);
  final client = RustCoreClient(hs);
  ref.onDispose(() => client.raw.close(force: true));
  return client;
});

/// The channel-B WS connector (Yjs sync + status).
final yjsWsProvider = Provider<YjsWsProvider>((ref) {
  final hs = ref.watch(rustCoreHandshakeProvider);
  return YjsWsProvider(hs);
});
