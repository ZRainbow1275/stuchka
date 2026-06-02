import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'cooldown_store.dart';

/// The feature key locked by an INV-07 Level-3 cooldown (compliance/05 §5.2 / spec 05 §5.6.4):
/// all INV-10 high-risk buttons for the triggering user.
const String kHighRiskFeature = 'high_risk_decisions';

/// The local user id. R1a is single-user-per-machine; the cooldown is scoped to this id so a group
/// member's Level-3 never locks the rest of the group (C-B-9 / prd §1.7 / §5.4.4).
const String kLocalUserId = 'local_user';

/// A read-only snapshot of a cooldown for the UI.
class CooldownSnapshot {
  const CooldownSnapshot({required this.active, required this.remaining});
  final bool active;
  final Duration remaining;

  static const CooldownSnapshot inactive =
      CooldownSnapshot(active: false, remaining: Duration.zero);
}

/// Watches the [CooldownStore] for a feature, ticking once a second so the remaining-time readout
/// in the appeal card stays live. Auto-disposes when no widget is listening.
class CooldownNotifier extends Notifier<CooldownSnapshot> {
  CooldownNotifier(this.feature, {this.userId = kLocalUserId});
  final String feature;
  final String userId;

  Timer? _ticker;

  @override
  CooldownSnapshot build() {
    ref.onDispose(() => _ticker?.cancel());
    _ticker ??= Timer.periodic(const Duration(seconds: 1), (_) => _refresh());
    return _read();
  }

  CooldownSnapshot _read() {
    final entry = CooldownStore.instance.get(userId, feature);
    if (entry == null) return CooldownSnapshot.inactive;
    return CooldownSnapshot(active: entry.active, remaining: entry.remaining);
  }

  void _refresh() {
    final next = _read();
    if (next.active != state.active || next.remaining != state.remaining) {
      state = next;
    }
  }

  /// Begin a 24h cooldown for this feature/user (INV-07 Level-3, compliance/05 §5.2).
  void begin24h() {
    CooldownStore.instance.set(
      userId: userId,
      feature: feature,
      until: DateTime.now().add(const Duration(hours: 24)),
    );
    state = _read();
  }

  /// Clear the cooldown locally (误判申诉 — "用户即审计员，本机立即解除无需审批", compliance/05 §5.3).
  void releaseLocally() {
    CooldownStore.instance.clear(userId, feature);
    state = CooldownSnapshot.inactive;
  }
}

/// Family-keyed cooldown provider (keyed by feature). The default high-risk key is [kHighRiskFeature].
final cooldownProvider =
    NotifierProvider.family<CooldownNotifier, CooldownSnapshot, String>(
  CooldownNotifier.new,
);

/// A submitted misdetection appeal record (local-only; never uploaded — compliance/05 §5.3).
class AppealRecord {
  const AppealRecord({
    required this.reason,
    required this.statement,
    required this.submittedAt,
    required this.triggerEventId,
    this.uploadDiagnosticBundle = false,
  });

  final String reason;
  final String statement;
  final DateTime submittedAt;
  final String triggerEventId;
  final bool uploadDiagnosticBundle;
}

/// Holds appeal records for the session + performs the local release. The appeal releases the
/// cooldown on this machine immediately, with NO project-side approval (compliance/05 §5.3 / §5.2:
/// "提前解除：仅通过申诉误判通道，项目方无法干预").
class AppealController extends Notifier<List<AppealRecord>> {
  @override
  List<AppealRecord> build() => const [];

  /// Submit an appeal for [feature] and release its cooldown locally.
  AppealRecord submit({
    String reason = 'misdetection',
    String statement = '',
    String triggerEventId = '',
    bool uploadDiagnosticBundle = false,
    String feature = kHighRiskFeature,
  }) {
    final record = AppealRecord(
      reason: reason,
      statement: statement,
      submittedAt: DateTime.now(),
      triggerEventId: triggerEventId,
      uploadDiagnosticBundle: uploadDiagnosticBundle,
    );
    state = [...state, record];
    ref.read(cooldownProvider(feature).notifier).releaseLocally();
    return record;
  }
}

final appealProvider =
    NotifierProvider<AppealController, List<AppealRecord>>(AppealController.new);
