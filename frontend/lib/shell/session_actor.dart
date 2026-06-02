import 'dart:math';

/// The acting subject id for audit `who` (INV-06). Stučka is a single-user local-first app, so the
/// actor is a stable per-process identifier. A real RFC-4122 v4 UUID is generated once and memoised;
/// the backend audit chain records it as the `who` of every editor step / acknowledgement.
String get sessionActorId => _cached ??= _generateUuidV4();

String? _cached;

String _generateUuidV4() {
  final rng = Random.secure();
  final b = List<int>.generate(16, (_) => rng.nextInt(256));
  b[6] = (b[6] & 0x0f) | 0x40; // version 4
  b[8] = (b[8] & 0x3f) | 0x80; // RFC 4122 variant
  String h(int i) => b[i].toRadixString(16).padLeft(2, '0');
  final hex = List<String>.generate(16, h);
  return '${hex[0]}${hex[1]}${hex[2]}${hex[3]}-'
      '${hex[4]}${hex[5]}-'
      '${hex[6]}${hex[7]}-'
      '${hex[8]}${hex[9]}-'
      '${hex[10]}${hex[11]}${hex[12]}${hex[13]}${hex[14]}${hex[15]}';
}
