import 'package:flutter/material.dart';

import '../../../ipc/dto/dtos.dart';
import '../../../ipc/dto/enums.dart';
import '../../../theme/stuchka_icons.dart';
import '../../../theme/stuchka_theme.dart';

/// Fact card (spec 04 §4.4 / brief §3.3). Left 4px color bar maps [FactStatus]; date in mono.
/// Two action buttons: 补证据 / 生成主张. NO "建议你思考一下" empty guidance — action-oriented only.
class FactCard extends StatelessWidget {
  const FactCard({
    super.key,
    required this.fact,
    this.onCollectEvidence,
    this.onGenerateClaim,
  });

  final FactDto fact;
  final VoidCallback? onCollectEvidence;
  final VoidCallback? onGenerateClaim;

  static Color statusColor(StuchkaSemanticColors c, FactStatus s) => switch (s) {
        FactStatus.pending => c.riskMid,
        FactStatus.confirmed => c.riskLow,
        FactStatus.disputed => c.sealRed,
        FactStatus.deprecated => c.ash,
      };

  @override
  Widget build(BuildContext context) {
    final colors = StuchkaSemanticColors.of(context);
    final bar = statusColor(colors, fact.status);
    return Card(
      clipBehavior: Clip.antiAlias,
      child: IntrinsicHeight(
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Container(width: 4, color: bar),
            Expanded(
              child: Padding(
                padding: const EdgeInsets.all(12),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Row(
                      children: [
                        Text(fact.category.name,
                            style: Theme.of(context).textTheme.bodySmall),
                        const Spacer(),
                        Text(
                          fact.updatedAt?.toIso8601String().split('T').first ?? '',
                          style: const TextStyle(
                            fontFamily: 'JetBrainsMonoSC',
                            fontFeatures: [],
                          ),
                        ),
                      ],
                    ),
                    const SizedBox(height: 6),
                    Text(fact.content),
                    const SizedBox(height: 10),
                    Row(
                      children: [
                        TextButton.icon(
                          onPressed: onCollectEvidence,
                          icon: const Icon(StuchkaIcons.evidence, size: 16),
                          label: const Text('补证据'),
                        ),
                        const SizedBox(width: 8),
                        TextButton.icon(
                          onPressed: onGenerateClaim,
                          icon: const Icon(StuchkaIcons.arbitrate, size: 16),
                          label: const Text('生成主张'),
                        ),
                      ],
                    ),
                  ],
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

/// Center pane: the fact card stream.
class FactCardsView extends StatelessWidget {
  const FactCardsView({super.key, required this.facts, this.onCollectEvidence});
  final List<FactDto> facts;
  final void Function(FactDto fact)? onCollectEvidence;

  @override
  Widget build(BuildContext context) {
    if (facts.isEmpty) {
      return const Center(child: Text('尚无事实 · 在诊断页补充陈述后将在此显示'));
    }
    return ListView.builder(
      padding: const EdgeInsets.all(12),
      itemCount: facts.length,
      itemBuilder: (context, i) => Padding(
        padding: const EdgeInsets.only(bottom: 10),
        child: FactCard(
          fact: facts[i],
          onCollectEvidence: () => onCollectEvidence?.call(facts[i]),
        ),
      ),
    );
  }
}
