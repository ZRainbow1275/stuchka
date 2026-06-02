import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../ipc/dto/dtos.dart';
import '../../../theme/stuchka_icons.dart';
import '../../../theme/stuchka_theme.dart';
import '../case_providers.dart';

/// M12 风险预警 surface (prd/04 §4.8). Renders the real deadline-expiry warnings (10/3/1-day tiers)
/// from `GET /case/:id/warnings`, the regulation-change items (when present), and the honest
/// evidence-loss seam note. Lucide icons only; zero emoji. Every deadline tier is the engine's real
/// buffered-remaining-days output — the panel never fabricates a warning.
class WarningFeedPanel extends ConsumerWidget {
  const WarningFeedPanel({super.key, required this.caseId});

  final String caseId;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final colors = StuchkaSemanticColors.of(context);
    final async = ref.watch(caseWarningsProvider(caseId));
    return async.when(
      loading: () => const Center(child: Padding(padding: EdgeInsets.all(24), child: CircularProgressIndicator())),
      error: (e, _) => _card(
        context,
        icon: StuchkaIcons.alert,
        color: colors.sealRed,
        title: '预警加载失败',
        body: '$e',
      ),
      data: (feed) => _feed(context, colors, feed),
    );
  }

  Widget _feed(BuildContext context, StuchkaSemanticColors colors, WarningFeedDto feed) {
    final children = <Widget>[];
    if (feed.items.isEmpty) {
      children.add(_card(
        context,
        icon: StuchkaIcons.deadline,
        color: colors.riskLow,
        title: '暂无临近到期的时效预警',
        body: '当前案件的仲裁 / 监察时效距到期均超过 10 天（预警仅在 10 / 3 / 1 天内触发）。',
      ));
    } else {
      for (final item in feed.items) {
        children.add(_warningTile(context, colors, item));
      }
    }
    // The evidence-loss honest seam — declared, never fabricated.
    if (!feed.evidenceLossSeam.available) {
      children.add(_card(
        context,
        icon: StuchkaIcons.alert,
        color: colors.riskMid,
        title: '证据灭失预警：尚未建模',
        body: '${feed.evidenceLossSeam.reason}。'
            '所需字段：${feed.evidenceLossSeam.requiredFields.join(' / ')}。',
      ));
    }
    return ListView(
      padding: const EdgeInsets.all(16),
      children: children,
    );
  }

  Widget _warningTile(BuildContext context, StuchkaSemanticColors colors, WarningItemDto item) {
    final tierColor = switch (item.tier) {
      't1_day' => colors.sealRed,
      't3_days' => colors.riskHigh,
      _ => colors.riskMid,
    };
    return Card(
      margin: const EdgeInsets.only(bottom: 12),
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Icon(StuchkaIcons.deadline, color: tierColor, size: 20),
                const SizedBox(width: 8),
                Expanded(
                  child: Text(
                    '${_kindLabel(item.kind)} · ${item.tierLabel}',
                    style: Theme.of(context).textTheme.titleMedium,
                  ),
                ),
              ],
            ),
            const SizedBox(height: 8),
            Text('建议在 ${item.bufferedRemainingDays} 天内完成（原始剩余 ${item.rawRemainingDays} 天，'
                '已含 10% 安全余量）。'),
            const SizedBox(height: 4),
            Text(
              '本结果须经人工二次确认，系统不作时效承诺。',
              style: Theme.of(context).textTheme.bodySmall?.copyWith(color: colors.inkBlue),
            ),
            if (item.lawRefs.isNotEmpty) ...[
              const SizedBox(height: 6),
              Text(
                '依据：${item.lawRefs.join('；')}',
                style: const TextStyle(fontFamily: 'JetBrainsMonoSC', fontSize: 11),
              ),
            ],
          ],
        ),
      ),
    );
  }

  String _kindLabel(String kind) => switch (kind) {
        'arbitration_general' => '劳动仲裁时效',
        'arbitration_wage' => '拖欠工资仲裁时效',
        'inspection' => '劳动监察投诉时效',
        'enforcement' => '执行申请时效',
        _ => kind,
      };

  Widget _card(
    BuildContext context, {
    required IconData icon,
    required Color color,
    required String title,
    required String body,
  }) {
    return Card(
      margin: const EdgeInsets.only(bottom: 12),
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Icon(icon, color: color, size: 20),
                const SizedBox(width: 8),
                Expanded(child: Text(title, style: Theme.of(context).textTheme.titleMedium)),
              ],
            ),
            const SizedBox(height: 8),
            Text(body),
          ],
        ),
      ),
    );
  }
}

/// Standalone page hosting the [WarningFeedPanel] for the `/case/:id/warnings` route.
class CaseWarningsPage extends StatelessWidget {
  const CaseWarningsPage({super.key, required this.caseId});

  final String caseId;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('风险预警')),
      body: WarningFeedPanel(caseId: caseId),
    );
  }
}
