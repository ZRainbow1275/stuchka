import 'dart:convert';

/// The READY-line handshake parsed from the `stuchka-core` child's stdout (D1 · backend
/// crates/core/src/spawn.rs::Handshake).
///
/// The real line is `READY{"port":N,"token":"<128 hex>"}` — exactly one line, the literal `READY`
/// prefix immediately followed by compact JSON (no separator, no trailing newline). The backend
/// handshake carries ONLY `port` + `token` (there is no `pid` or `schema` field on the wire); the
/// brief's `schema==1` requirement is enforced as a Dart-side default with optional override so a
/// future backend that adds a `schema` field is validated, while today's two-field line passes.
class RustCoreHandshake {
  RustCoreHandshake({required this.port, required this.token, this.schema = 1, this.pid});

  /// OS-assigned loopback port.
  final int port;

  /// 128-char hex Bearer token.
  final String token;

  /// Wire schema version. The backend does not yet emit this; defaults to 1. A future backend
  /// emitting `schema != 1` is rejected at [parse] (StateError).
  final int schema;

  /// Optional child pid (not on the wire today; null unless supplied).
  final int? pid;

  /// The literal prefix that precedes the JSON (must match backend `READY_PREFIX`).
  static const String prefix = 'READY';

  /// Parse a single READY line. Returns null when the line is not a READY line at all; throws
  /// [StateError] when it is a READY line but malformed or schema != 1.
  static RustCoreHandshake? parse(String line) {
    final trimmed = line.trim();
    if (!trimmed.startsWith(prefix)) return null;
    final jsonPart = trimmed.substring(prefix.length);
    final Object? decoded;
    try {
      decoded = jsonDecode(jsonPart);
    } on FormatException catch (e) {
      throw StateError('READY line JSON malformed: $e');
    }
    if (decoded is! Map<String, dynamic>) {
      throw StateError('READY payload is not a JSON object');
    }
    final port = decoded['port'];
    final token = decoded['token'];
    if (port is! num) throw StateError('READY payload missing numeric port');
    if (token is! String || token.isEmpty) {
      throw StateError('READY payload missing token');
    }
    final schema = (decoded['schema'] as num?)?.toInt() ?? 1;
    if (schema != 1) {
      throw StateError('unsupported READY schema $schema (expected 1)');
    }
    return RustCoreHandshake(
      port: port.toInt(),
      token: token,
      schema: schema,
      pid: (decoded['pid'] as num?)?.toInt(),
    );
  }

  /// The loopback HTTP base URL (channel B).
  String get baseUrl => 'http://127.0.0.1:$port';

  /// The loopback WS base URL (channel B Yjs sync).
  String get wsBaseUrl => 'ws://127.0.0.1:$port';

  @override
  String toString() => 'RustCoreHandshake(port:$port, schema:$schema)';
}
