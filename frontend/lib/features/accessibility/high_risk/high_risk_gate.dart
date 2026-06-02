import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../theme/stuchka_icons.dart';
import '../../../theme/stuchka_theme.dart';
import '../crisis/cooldown_providers.dart';
import '../crisis/crisis_level3_dialog.dart';
import '../hotlines.dart';
import 'countdown_button.dart';
import 'disclaimer_payload.dart';

/// Per-open gate state: whether the "我已完整阅读" checkbox is ticked. Family-keyed by scenario so
/// each open starts fresh (auto-dispose is the riverpod-3 default).
final highRiskReadProvider =
    NotifierProvider.family<HighRiskReadController, bool, HighRiskScenario>(
  HighRiskReadController.new,
);

class HighRiskReadController extends Notifier<bool> {
  HighRiskReadController(this.scenario);
  final HighRiskScenario scenario;

  @override
  bool build() => false;
  void setRead(bool v) => state = v;
}

/// INV-10 forced-disclaimer gate (compliance/05 §2.2 / §5.1 / spec 05 §5.7.1). Invariants:
///   1. an 8-second non-skippable read cooldown + a mandatory SECOND press behind a further 3s gate
///      (CountdownButton — total flow >= 11s, compliance/05 §5.1);
///   2. a non-collapsible, verbatim full disclaimer (>= 200 chars, compliance/05 §3);
///   3. a mandatory "我已完整阅读" checkbox before the confirm can enable;
///   4. the default focus is the cancel ("返回") button (autofocus), never confirm.
///
/// When the triggering user is under an INV-07 Level-3 24h cooldown (compliance/05 §5.2) the gate
/// surfaces the misdetection-appeal card at the top and BLOCKS confirmation until released.
class HighRiskGate extends ConsumerWidget {
  const HighRiskGate({
    super.key,
    required this.scenario,
    required this.onConfirmed,
    this.payload = DisclaimerPayload.none,
  });

  final HighRiskScenario scenario;
  final DisclaimerPayload payload;

  /// Fired once on the SECOND confirm press, carrying the measured cooldown/flow timings
  /// (compliance/05 §2.3 audit four-tuple `cooldown_actual_ms`).
  final void Function(ConfirmTimings timings) onConfirmed;

  /// Show the gate as a modal dialog. Returns the [ConfirmTimings] iff the user confirmed, else null.
  static Future<ConfirmTimings?> show(
    BuildContext context,
    HighRiskScenario scenario, {
    DisclaimerPayload payload = DisclaimerPayload.none,
  }) async {
    ConfirmTimings? result;
    await showDialog<void>(
      context: context,
      barrierDismissible: false,
      builder: (_) => HighRiskGate(
        scenario: scenario,
        payload: payload,
        onConfirmed: (t) => result = t,
      ),
    );
    return result;
  }

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final read = ref.watch(highRiskReadProvider(scenario));
    final cooldown = ref.watch(cooldownProvider(kHighRiskFeature));
    final colors = StuchkaSemanticColors.of(context);

    return AlertDialog(
      title: Row(
        children: [
          Icon(StuchkaIcons.alert, color: Theme.of(context).colorScheme.error),
          const SizedBox(width: 8),
          Expanded(child: Text(scenario.title)),
        ],
      ),
      content: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 560),
        child: SingleChildScrollView(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            mainAxisSize: MainAxisSize.min,
            children: [
              // INV-07 Level-3 24h cooldown lock + misdetection appeal (compliance/05 §5.2 / §5.3).
              if (cooldown.active) ...[
                CooldownAppealCard(
                  remaining: cooldown.remaining,
                  onAppeal: () =>
                      ref.read(appealProvider.notifier).submit(reason: 'misdetection'),
                ),
                const SizedBox(height: 16),
              ],
              // Non-collapsible verbatim full disclaimer (>= 200 chars).
              Text(
                scenario.fullDisclaimer(payload),
                style: Theme.of(context).textTheme.bodyMedium,
              ),
              const SizedBox(height: 16),
              _HotlinePanel(scenario: scenario),
              const SizedBox(height: 16),
              CheckboxListTile(
                value: read,
                contentPadding: EdgeInsets.zero,
                controlAffinity: ListTileControlAffinity.leading,
                activeColor: colors.sealRed,
                onChanged: cooldown.active
                    ? null
                    : (v) => ref
                        .read(highRiskReadProvider(scenario).notifier)
                        .setRead(v ?? false),
                title: const Text('我已完整阅读上方免责声明'),
              ),
            ],
          ),
        ),
      ),
      actions: [
        // Default focus = 返回 (cancel), per compliance/05 §5.1 强制焦点.
        TextButton(
          autofocus: true,
          onPressed: () => Navigator.pop(context),
          child: const Text('返回'),
        ),
        // Confirm requires: checkbox read + 8s + a second press behind a 3s gate. Hard-blocked while
        // a Level-3 cooldown is active (compliance/05 §5.2).
        CountdownButton(
          seconds: 8,
          secondPressGapSeconds: 3,
          enabled: read && !cooldown.active,
          label: '我已知风险并确认',
          onPressed: (timings) {
            onConfirmed(timings);
            Navigator.pop(context);
          },
        ),
      ],
    );
  }
}

/// The per-scenario hotline panel (compliance/05 §6: scoped subset of the shared resource list).
class _HotlinePanel extends StatelessWidget {
  const _HotlinePanel({required this.scenario});
  final HighRiskScenario scenario;

  @override
  Widget build(BuildContext context) {
    final colors = StuchkaSemanticColors.of(context);
    final hotlines = LegalAidResources.forScenario(scenario);
    return Container(
      width: double.infinity,
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: colors.rice,
        borderRadius: BorderRadius.circular(8),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Icon(StuchkaIcons.help, size: 16, color: colors.inkBlue),
              const SizedBox(width: 6),
              const Text('需要帮助？可联系法律援助 / 工会：',
                  style: TextStyle(fontWeight: FontWeight.w500)),
            ],
          ),
          const SizedBox(height: 6),
          ...hotlines.map(
            (h) => Padding(
              padding: const EdgeInsets.symmetric(vertical: 2),
              child: Row(
                children: [
                  Icon(StuchkaIcons.phone, size: 14, color: colors.inkBlue),
                  const SizedBox(width: 6),
                  Text(h.code, style: const TextStyle(fontWeight: FontWeight.w700)),
                  const SizedBox(width: 6),
                  Expanded(child: Text(h.desc)),
                ],
              ),
            ),
          ),
          const SizedBox(height: 6),
          Text(kHotlineDisclaimer, style: Theme.of(context).textTheme.bodySmall),
        ],
      ),
    );
  }
}
