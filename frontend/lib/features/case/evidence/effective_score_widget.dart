import 'package:flutter/material.dart';

import '../../../ipc/dto/dtos.dart';
import '../../../theme/stuchka_theme.dart';

/// Four effective-score bands (spec 04 §4.6 / brief §3.5):
///   GREEN >= .8 / YELLOW .6-.8 / ORANGE .4-.6 / RED < .4.
enum EffectiveBand { green, yellow, orange, red }

EffectiveBand bandFor(double score) {
  if (score >= 0.8) return EffectiveBand.green;
  if (score >= 0.6) return EffectiveBand.yellow;
  if (score >= 0.4) return EffectiveBand.orange;
  return EffectiveBand.red;
}

/// EffectiveScoreWidget — shows the four-band score + the five dimensions
/// (source/temporal/integrity/relevance/authenticity). UI presents the rule-engine result only;
/// NO "我觉得这个证据…" AI copy.
class EffectiveScoreWidget extends StatelessWidget {
  const EffectiveScoreWidget({super.key, required this.score, required this.breakdown});

  final double score;
  final ScoreBreakdown breakdown;

  @override
  Widget build(BuildContext context) {
    final colors = StuchkaSemanticColors.of(context);
    final band = bandFor(score);
    final color = switch (band) {
      EffectiveBand.green => colors.riskLow,
      EffectiveBand.yellow => colors.riskMid,
      EffectiveBand.orange => colors.sealRedSoftFallback(colors),
      EffectiveBand.red => colors.sealRed,
    };
    final label = switch (band) {
      EffectiveBand.green => '强（GREEN）',
      EffectiveBand.yellow => '中（YELLOW）',
      EffectiveBand.orange => '弱（ORANGE）',
      EffectiveBand.red => '不足（RED）',
    };
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          children: [
            Container(
              padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
              decoration: BoxDecoration(color: color, borderRadius: BorderRadius.circular(4)),
              child: Text(label, style: TextStyle(color: colors.paper, fontSize: 12)),
            ),
            const SizedBox(width: 8),
            Text('效力评分 ${(score * 100).toStringAsFixed(0)}',
                style: const TextStyle(fontWeight: FontWeight.w700)),
          ],
        ),
        const SizedBox(height: 8),
        _Dim(label: '来源 source', value: breakdown.source, color: color),
        _Dim(label: '时序 temporal', value: breakdown.temporal, color: color),
        _Dim(label: '完整 integrity', value: breakdown.integrity, color: color),
        _Dim(label: '关联 relevance', value: breakdown.relevance, color: color),
        _Dim(label: '真实 authenticity', value: breakdown.authenticity, color: color),
      ],
    );
  }
}

class _Dim extends StatelessWidget {
  const _Dim({required this.label, required this.value, required this.color});
  final String label;
  final double value;
  final Color color;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 2),
      child: Row(
        children: [
          SizedBox(width: 130, child: Text(label, style: const TextStyle(fontSize: 12))),
          Expanded(
            child: LinearProgressIndicator(
              value: value.clamp(0, 1),
              color: color,
              backgroundColor: color.withValues(alpha: 0.15),
              minHeight: 8,
            ),
          ),
          const SizedBox(width: 8),
          SizedBox(width: 36, child: Text(value.toStringAsFixed(2), style: const TextStyle(fontSize: 12))),
        ],
      ),
    );
  }
}

extension on StuchkaSemanticColors {
  // ORANGE band uses a midpoint between riskMid and sealRed for clarity.
  Color sealRedSoftFallback(StuchkaSemanticColors c) =>
      Color.lerp(c.riskMid, c.sealRed, 0.5) ?? c.riskMid;
}
