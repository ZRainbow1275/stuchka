import 'dart:convert';

/// Channel-A bridge message types — the dot-namespaced canonical union (spec §2.3.2). This is the
/// byte-for-byte Dart mirror of the AUTHORITATIVE TS union in `Stučka/editor/src/bridge.ts`
/// (`BridgeMessageType`). The wire value is the dotted string (e.g. `editor.step`).
///
/// `editor.localUpdate` is the WebView->Flutter upstream type the editor side now emits (a local
/// editor update relayed to the shell); it is included here per the channel-A contract.
enum BridgeMessageType {
  editorReady('editor.ready'),
  editorStep('editor.step'),
  editorApplyRemote('editor.applyRemote'),
  editorLocalUpdate('editor.localUpdate'),
  editorSelection('editor.selection'),
  editorCmd('editor.cmd'),
  tabSwitch('tab.switch'),
  tabOpened('tab.opened'),
  tabClose('tab.close'),
  imeCompositionStart('ime.compositionStart'),
  imeCompositionEnd('ime.compositionEnd'),
  mergeConflict('merge.conflict'),
  mergeResolve('merge.resolve'),
  auditFacetClick('audit.facetClick'),
  crisisScan('crisis.scan'),
  tabsetOpenExternal('tabset.openExternal');

  const BridgeMessageType(this.wire);

  /// The on-wire dotted string (matches the TS `BridgeMessageType` union member).
  final String wire;

  /// Resolve a wire string to its [BridgeMessageType]; throws [FormatException] when unknown so an
  /// unrecognised type cannot be silently mis-handled.
  static BridgeMessageType fromWire(String wire) {
    for (final t in BridgeMessageType.values) {
      if (t.wire == wire) return t;
    }
    throw FormatException('unknown bridge message type: $wire');
  }
}

/// Channel-A bridge envelope (brief §1.4 / §7; spec §2.3.1). The WebView<->Tiptap WebMessageChannel
/// carries these JSON envelopes. Channel A NEVER reaches the Rust core directly (CSP
/// connect-src 'none'); a Step received here is relayed by the shell over channel B
/// (`POST /document/:id/steps`). R1a defines the protocol shape; the editor-core串联 lives in the
/// 03-editor-workbench subtask.
///
/// This is the byte-for-byte Dart mirror of the AUTHORITATIVE TS `BridgeEnvelope` interface in
/// `Stučka/editor/src/bridge.ts`:
/// `{ id: string, type: BridgeMessageType, schemaVersion: 1, ts: ISO8601 string, payload: object }`.
/// The TS side DROPS any inbound envelope where `schemaVersion !== 1` (the A4 downgrade guard); the
/// Dart [decode] mirrors that by THROWING on a non-1 schema version so a downgrade is incompatible.
class BridgeEnvelope {
  BridgeEnvelope({
    required this.id,
    required this.type,
    this.payload = const {},
    String? ts,
    this.schemaVersion = currentSchemaVersion,
  }) : ts = ts ?? DateTime.now().toUtc().toIso8601String();

  /// The only supported envelope schema version. Monotonic; a downgrade is incompatible (A4).
  static const int currentSchemaVersion = 1;

  /// In-process correlation id (idempotency key; explicitly NOT a D9 persistence key).
  final String id;

  /// The dot-namespaced message type (mirrors the TS `BridgeMessageType` union).
  final BridgeMessageType type;

  /// Monotonic schema version. Always [currentSchemaVersion] on the wire in R1.
  final int schemaVersion;

  /// ISO 8601 timestamp string.
  final String ts;

  final Map<String, dynamic> payload;

  Map<String, dynamic> toJson() => {
        'id': id,
        'type': type.wire,
        'schemaVersion': schemaVersion,
        'ts': ts,
        'payload': payload,
      };

  String encode() => jsonEncode(toJson());

  /// Decode a TS-shaped envelope. Throws [FormatException] when the JSON is not an object, when the
  /// `schemaVersion` is absent or != [currentSchemaVersion] (mirrors the TS A4 downgrade guard),
  /// or when the `type` is not a known [BridgeMessageType].
  static BridgeEnvelope decode(String raw) {
    final m = jsonDecode(raw);
    if (m is! Map<String, dynamic>) {
      throw const FormatException('bridge envelope is not a JSON object');
    }
    final schema = m['schemaVersion'];
    if (schema is! int || schema != currentSchemaVersion) {
      // A4: downgrade incompatible — reject rather than mis-handle.
      throw FormatException(
        'unsupported bridge schemaVersion $schema (expected $currentSchemaVersion)',
      );
    }
    final id = m['id'];
    if (id is! String || id.isEmpty) {
      throw const FormatException('bridge envelope missing string id');
    }
    final typeRaw = m['type'];
    if (typeRaw is! String) {
      throw const FormatException('bridge envelope missing string type');
    }
    final ts = m['ts'];
    if (ts is! String || ts.isEmpty) {
      throw const FormatException('bridge envelope missing string ts');
    }
    return BridgeEnvelope(
      id: id,
      type: BridgeMessageType.fromWire(typeRaw),
      schemaVersion: schema,
      ts: ts,
      payload: (m['payload'] as Map?)?.cast<String, dynamic>() ?? const {},
    );
  }
}
