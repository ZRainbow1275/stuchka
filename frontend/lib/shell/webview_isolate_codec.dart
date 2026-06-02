import 'dart:isolate';

import '../bridge/bridge_protocol.dart';

/// Off-main-thread envelope decode (FE02 §2.5). Small Channel-A messages decode inline (cheap); a
/// large message (a 200 KB+ base64 Yjs update in `editor.localUpdate` / `editor.applyRemote`) is
/// decoded on a worker isolate so the JSON parse does not blow the 16 ms frame budget.
///
/// `Isolate.run` spawns within the isolate GROUP, so the parsed [BridgeEnvelope] (plain data:
/// String / int / Map) is returned to the main thread by copy.
class WebViewIsolateCodec {
  WebViewIsolateCodec({this.offloadThresholdBytes = 16 * 1024});

  /// Inbound payloads at or above this many UTF-16 code units decode on a worker isolate.
  final int offloadThresholdBytes;

  Future<BridgeEnvelope> decode(String raw) async {
    if (raw.length < offloadThresholdBytes) {
      return BridgeEnvelope.decode(raw);
    }
    return Isolate.run(() => BridgeEnvelope.decode(raw));
  }

  /// Whether [decode] would offload [raw] to a worker isolate (exposed for tests/metrics).
  bool wouldOffload(String raw) => raw.length >= offloadThresholdBytes;
}
