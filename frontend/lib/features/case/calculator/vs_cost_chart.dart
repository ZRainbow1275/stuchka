import 'package:fl_chart/fl_chart.dart';
import 'package:flutter/material.dart';

import '../../../theme/stuchka_theme.dart';

/// One bar in the winning-cost comparison.
class CostBar {
  const CostBar(this.label, this.amount);
  final String label;
  final double amount;
}

/// Winning-cost comparison chart (spec 04 §4.7 / brief §3.6). A HORIZONTAL bar chart (4 bars:
/// 预期收益 / 仲裁费 / 律师费区间 / 时间成本). 禁止饼图 (公文调性) — this is a `BarChart` with
/// `rotationQuarterTurns: 1` (rotated to horizontal), never a `PieChart`.
class VsCostChart extends StatelessWidget {
  const VsCostChart({super.key, required this.bars});
  final List<CostBar> bars;

  @override
  Widget build(BuildContext context) {
    final colors = StuchkaSemanticColors.of(context);
    final maxY = bars.isEmpty ? 1.0 : bars.map((b) => b.amount).reduce((a, b) => a > b ? a : b) * 1.2;
    return SizedBox(
      height: 220,
      child: BarChart(
        BarChartData(
          // Rotate the whole chart a quarter-turn → horizontal bars (not a pie).
          rotationQuarterTurns: 1,
          alignment: BarChartAlignment.spaceAround,
          maxY: maxY <= 0 ? 1 : maxY,
          barTouchData: BarTouchData(enabled: true),
          titlesData: FlTitlesData(
            topTitles: const AxisTitles(sideTitles: SideTitles(showTitles: false)),
            rightTitles: const AxisTitles(sideTitles: SideTitles(showTitles: false)),
            leftTitles: const AxisTitles(sideTitles: SideTitles(showTitles: false)),
            bottomTitles: AxisTitles(
              sideTitles: SideTitles(
                showTitles: true,
                reservedSize: 80,
                getTitlesWidget: (value, meta) {
                  final i = value.toInt();
                  if (i < 0 || i >= bars.length) return const SizedBox.shrink();
                  return Padding(
                    padding: const EdgeInsets.only(top: 4),
                    child: Text(bars[i].label, style: const TextStyle(fontSize: 11)),
                  );
                },
              ),
            ),
          ),
          gridData: const FlGridData(show: false),
          borderData: FlBorderData(show: false),
          barGroups: [
            for (var i = 0; i < bars.length; i++)
              BarChartGroupData(
                x: i,
                barRods: [
                  BarChartRodData(
                    toY: bars[i].amount,
                    color: i == 0 ? colors.riskLow : colors.inkBlue,
                    width: 18,
                    borderRadius: BorderRadius.circular(2),
                  ),
                ],
              ),
          ],
        ),
      ),
    );
  }
}
