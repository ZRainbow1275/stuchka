/// A per-user cooldown lock (spec 05 §5.6.4). Locks ONLY the triggering user's high-risk
/// operations — never the rest of a group (§0529 prd §1.7 / §5.4.4). In-memory for R1a (front-end
/// session scope); the persisted/audited cooldown is owned by the backend in a later subtask.
class CooldownEntry {
  CooldownEntry({required this.until});
  final DateTime until;

  bool get active => DateTime.now().isBefore(until);
  Duration get remaining {
    final r = until.difference(DateTime.now());
    return r.isNegative ? Duration.zero : r;
  }
}

class CooldownStore {
  CooldownStore._();
  static final CooldownStore instance = CooldownStore._();

  final Map<String, CooldownEntry> _entries = {};

  String _key(String userId, String feature) => '$userId::$feature';

  void set({required String userId, required String feature, required DateTime until}) {
    _entries[_key(userId, feature)] = CooldownEntry(until: until);
  }

  CooldownEntry? get(String userId, String feature) {
    final e = _entries[_key(userId, feature)];
    if (e == null) return null;
    if (!e.active) {
      _entries.remove(_key(userId, feature));
      return null;
    }
    return e;
  }

  bool isActive(String userId, String feature) => get(userId, feature)?.active ?? false;

  /// Release a single user's cooldown for a feature (误判申诉 local release, spec 05 §5.6.4 /
  /// compliance/05 §5.3). Scoped to the (userId, feature) pair so a group member's release never
  /// touches another member's lock.
  void clear(String userId, String feature) => _entries.remove(_key(userId, feature));

  void clearAll() => _entries.clear();
}
