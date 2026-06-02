import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:stuchka/bridge/bridge_protocol.dart';

/// S2 regression guard: the Dart [BridgeEnvelope] must be byte-for-byte wire-compatible with the
/// AUTHORITATIVE TS envelope in `Stučka/editor/src/bridge.ts`:
/// `{ id, type, schemaVersion: 1, ts, payload }` with dot-namespaced types. The TS side DROPS any
/// inbound envelope where `schemaVersion !== 1` (A4 downgrade guard); the Dart [decode] mirrors that
/// by THROWING on a non-1 schema version.
void main() {
  group('BridgeEnvelope wire compatibility with the TS canonical shape', () {
    test('a TS-shaped envelope round-trips (id/type/schemaVersion/ts/payload)', () {
      // The exact JSON the TS `postBack` emits for an editor.step.
      const raw =
          '{"id":"wv-abc-1","type":"editor.step","schemaVersion":1,'
          '"ts":"2026-06-02T12:00:00.000Z","payload":{"steps":[1,2,3],"clientId":7}}';

      final env = BridgeEnvelope.decode(raw);
      expect(env.id, 'wv-abc-1');
      expect(env.type, BridgeMessageType.editorStep);
      expect(env.type.wire, 'editor.step');
      expect(env.schemaVersion, 1);
      expect(env.ts, '2026-06-02T12:00:00.000Z');
      expect(env.payload['clientId'], 7);
      expect(env.payload['steps'], [1, 2, 3]);

      // Re-encode and confirm the JSON is structurally identical to the input.
      final reDecoded = jsonDecode(env.encode()) as Map<String, dynamic>;
      expect(reDecoded, jsonDecode(raw));
    });

    test('encode emits the five canonical keys with the dotted type string', () {
      final env = BridgeEnvelope(
        id: 'wv-x-1',
        type: BridgeMessageType.editorLocalUpdate,
        ts: '2026-06-02T00:00:00.000Z',
        payload: const {'update': 'base64=='},
      );
      final json = jsonDecode(env.encode()) as Map<String, dynamic>;
      expect(json.keys.toSet(), {'id', 'type', 'schemaVersion', 'ts', 'payload'});
      expect(json['type'], 'editor.localUpdate'); // the NEW WV->F upstream type
      expect(json['schemaVersion'], 1);
    });

    test('schemaVersion: 2 is rejected (A4 downgrade incompatible)', () {
      const raw =
          '{"id":"wv-abc-2","type":"editor.step","schemaVersion":2,'
          '"ts":"2026-06-02T12:00:00.000Z","payload":{}}';
      expect(() => BridgeEnvelope.decode(raw), throwsFormatException);
    });

    test('a missing schemaVersion is rejected', () {
      const raw = '{"id":"x","type":"editor.ready","ts":"t","payload":{}}';
      expect(() => BridgeEnvelope.decode(raw), throwsFormatException);
    });

    test('an unknown dotted type is rejected', () {
      const raw =
          '{"id":"x","type":"editor.bogus","schemaVersion":1,"ts":"t","payload":{}}';
      expect(() => BridgeEnvelope.decode(raw), throwsFormatException);
    });

    test('all canonical dotted types resolve via fromWire', () {
      const expected = <String>{
        'editor.ready',
        'editor.step',
        'editor.applyRemote',
        'editor.localUpdate',
        'editor.selection',
        'editor.cmd',
        'tab.switch',
        'tab.opened',
        'tab.close',
        'ime.compositionStart',
        'ime.compositionEnd',
        'merge.conflict',
        'merge.resolve',
        'audit.facetClick',
        'crisis.scan',
        'tabset.openExternal',
      };
      final actual = BridgeMessageType.values.map((t) => t.wire).toSet();
      expect(actual, expected);
      for (final w in expected) {
        expect(BridgeMessageType.fromWire(w).wire, w);
      }
    });
  });
}
