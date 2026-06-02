import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../theme/stuchka_icons.dart';
import '../../../theme/stuchka_theme.dart';
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

/// INV-10 forced-disclaimer gate (spec 05 §5.7.1). The three engineering invariants:
///   1. 8-second non-skippable cooldown (CountdownButton);
///   2. non-collapsible full disclaimer;
///   3. mandatory "我已完整阅读" checkbox before the confirm can enable.
/// The default focus is the cancel ("返回") button (autofocus), never confirm.
class HighRiskGate extends ConsumerWidget {
  const HighRiskGate({
    super.key,
    required this.scenario,
    required this.onConfirmed,
  });

  final HighRiskScenario scenario;
  final VoidCallback onConfirmed;

  /// Show the gate as a modal dialog. Returns true iff the user confirmed.
  static Future<bool> show(BuildContext context, HighRiskScenario scenario) async {
    var confirmed = false;
    await showDialog<void>(
      context: context,
      barrierDismissible: false,
      builder: (_) => HighRiskGate(
        scenario: scenario,
        onConfirmed: () => confirmed = true,
      ),
    );
    return confirmed;
  }

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final read = ref.watch(highRiskReadProvider(scenario));
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
              // Non-collapsible full disclaimer.
              Text(
                scenario.fullDisclaimer,
                style: Theme.of(context).textTheme.bodyMedium,
              ),
              const SizedBox(height: 16),
              const _HotlinePanel(),
              const SizedBox(height: 16),
              CheckboxListTile(
                value: read,
                contentPadding: EdgeInsets.zero,
                controlAffinity: ListTileControlAffinity.leading,
                activeColor: colors.sealRed,
                onChanged: (v) =>
                    ref.read(highRiskReadProvider(scenario).notifier).setRead(v ?? false),
                title: const Text('我已完整阅读上方免责声明'),
              ),
            ],
          ),
        ),
      ),
      actions: [
        // Default focus = 返回 (cancel), per INV-10.
        TextButton(
          autofocus: true,
          onPressed: () => Navigator.pop(context),
          child: const Text('返回'),
        ),
        CountdownButton(
          seconds: 8,
          enabled: read,
          label: '我已知风险并确认',
          onPressed: () {
            onConfirmed();
            Navigator.pop(context);
          },
        ),
      ],
    );
  }
}

class _HotlinePanel extends StatelessWidget {
  const _HotlinePanel();

  @override
  Widget build(BuildContext context) {
    final colors = StuchkaSemanticColors.of(context);
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
          ...kLegalAidHotlines.map(
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
