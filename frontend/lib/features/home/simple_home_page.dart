import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../../theme/stuchka_icons.dart';
import '../../theme/stuchka_theme.dart';
import '../case/case_providers.dart';

/// Simple mode home — three big buttons (spec 05 §5.3). Each button >= 96dp tall, label >= 22pt.
class SimpleHomePage extends ConsumerWidget {
  const SimpleHomePage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final activeCase = ref.watch(activeCaseProvider);
    return Scaffold(
      body: SafeArea(
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 32, vertical: 48),
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              SimpleBigButton(
                icon: StuchkaIcons.diagnose,
                label: '开始诊断',
                onTap: () => context.go('/intake/identity'),
              ),
              const SizedBox(height: 24),
              SimpleBigButton(
                icon: StuchkaIcons.evidence,
                label: '继续案件',
                enabled: activeCase != null,
                subLabel: activeCase?.shortLabel ?? '尚无案件',
                onTap: activeCase == null ? null : () => context.go('/case/${activeCase.id}'),
              ),
              const SizedBox(height: 24),
              SimpleBigButton(
                icon: StuchkaIcons.help,
                label: '求助',
                seal: true,
                onTap: () => context.go('/intake/identity'),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// A >=96dp tall big button with a >=22pt label (spec 05 §5.3 sizing contract).
class SimpleBigButton extends StatelessWidget {
  const SimpleBigButton({
    super.key,
    required this.icon,
    required this.label,
    this.subLabel,
    this.onTap,
    this.enabled = true,
    this.seal = false,
  });

  static const double minHeight = 96;
  static const double labelSize = 22;

  final IconData icon;
  final String label;
  final String? subLabel;
  final VoidCallback? onTap;
  final bool enabled;
  final bool seal;

  @override
  Widget build(BuildContext context) {
    final colors = StuchkaSemanticColors.of(context);
    final bg = seal ? colors.sealRed : colors.inkBlue;
    return Semantics(
      button: true,
      label: label,
      enabled: enabled,
      child: SizedBox(
        width: double.infinity,
        child: Material(
          color: enabled ? bg : colors.ash,
          borderRadius: BorderRadius.circular(StuchkaSpacing.of(context).radiusCard),
          child: InkWell(
            onTap: enabled ? onTap : null,
            borderRadius: BorderRadius.circular(StuchkaSpacing.of(context).radiusCard),
            child: ConstrainedBox(
              constraints: const BoxConstraints(minHeight: minHeight),
              child: Padding(
                padding: const EdgeInsets.symmetric(horizontal: 24, vertical: 16),
                child: Row(
                  children: [
                    Icon(icon, color: colors.paper, size: 36),
                    const SizedBox(width: 20),
                    Expanded(
                      child: Column(
                        mainAxisSize: MainAxisSize.min,
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            label,
                            style: TextStyle(
                              color: colors.paper,
                              fontSize: labelSize,
                              fontWeight: FontWeight.w700,
                            ),
                          ),
                          if (subLabel != null)
                            Padding(
                              padding: const EdgeInsets.only(top: 4),
                              child: Text(
                                subLabel!,
                                style: TextStyle(color: colors.paper, fontSize: 15),
                              ),
                            ),
                        ],
                      ),
                    ),
                  ],
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}
