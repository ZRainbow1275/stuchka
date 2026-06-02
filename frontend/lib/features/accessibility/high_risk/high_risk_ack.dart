import 'dart:developer' as developer;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../ipc/rust_core_client.dart';
import '../../../ipc/rust_core_providers.dart';
import 'countdown_button.dart';
import 'disclaimer_payload.dart';

/// Persist an INV-10 high-risk disclaimer acknowledgement to the independent audit.sqlite hash
/// chain (compliance/05 §2.3 audit four-tuple, INV-06 append-only) via `POST /audit/ack`.
///
/// This is the SINGLE place every `HighRiskGate.show` call site routes the [ConfirmTimings] the
/// gate returns on the SECOND (final) confirm, so the acknowledgement (who/when/why=high_risk_ack/
/// what=scene + timings) is AUDITED instead of dropped. The post is best-effort: it must always be
/// ATTEMPTED (never silently dropped), but a transient backend write failure is logged rather than
/// surfaced — the gate has already enforced the cooldown + double-confirm, and blocking the user on
/// an audit transport hiccup would harm the worker the system serves. The backend rejects a
/// sub-8000ms cooldown (the §2.3 floor) with a 400, which is logged for diagnosis.
Future<void> postHighRiskAck(
  WidgetRef ref, {
  required String caseId,
  required HighRiskScenario scenario,
  required ConfirmTimings timings,
}) async {
  final client = ref.read(rustCoreClientProvider);
  try {
    await client.ackHighRisk(
      caseId: caseId,
      sceneId: scenario.sceneId,
      timings: timings,
    );
  } on RustCoreApiException catch (e) {
    developer.log(
      'INV-10 high-risk acknowledgement audit-append failed '
      '(scene=${scenario.sceneId}, case=$caseId): ${e.code} ${e.message}',
      name: 'high_risk_ack',
    );
  } catch (e) {
    developer.log(
      'INV-10 high-risk acknowledgement audit-append failed '
      '(scene=${scenario.sceneId}, case=$caseId): $e',
      name: 'high_risk_ack',
    );
  }
}
