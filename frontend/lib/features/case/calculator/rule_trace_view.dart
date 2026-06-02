import 'package:flutter/material.dart';

import '../../../theme/stuchka_theme.dart';

/// One rule-engine derivation step (mirror of rule-engine DerivationStep + the law refs that back
/// the whole RuleOutcome). Each step shows: 步骤N · 标题 + 条号 + 公式 + 输入 + 结果.
class RuleTrace {
  RuleTrace({
    required this.labelZh,
    required this.expression,
    required this.value,
    this.lawRefs = const [],
    this.inputs = const {},
  });

  final String labelZh;
  final String expression;
  final String value;
  final List<String> lawRefs; // D8 URN strings
  final Map<String, String> inputs;
}

/// RuleTraceView (spec 04 §4.7 / brief §3.6). Every step shows the rule-engine cited article + the
/// formula + inputs + intermediate value + result. Calculation automation level A — AI not involved.
class RuleTraceView extends StatelessWidget {
  const RuleTraceView({super.key, required this.traces});
  final List<RuleTrace> traces;

  @override
  Widget build(BuildContext context) {
    if (traces.isEmpty) {
      return const Padding(
        padding: EdgeInsets.all(16),
        child: Text('暂无计算 · 提交工资与期间后将逐步展示规则引擎推导'),
      );
    }
    return Column(
      children: [
        for (var i = 0; i < traces.length; i++) _StepCard(index: i + 1, trace: traces[i]),
      ],
    );
  }
}

class _StepCard extends StatelessWidget {
  const _StepCard({required this.index, required this.trace});
  final int index;
  final RuleTrace trace;

  @override
  Widget build(BuildContext context) {
    final colors = StuchkaSemanticColors.of(context);
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(12),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Text('步骤$index · ${trace.labelZh}',
                    style: const TextStyle(fontWeight: FontWeight.w700)),
                const Spacer(),
                Text(trace.value,
                    style: TextStyle(
                      fontFamily: 'JetBrainsMonoSC',
                      color: colors.inkBlue,
                      fontWeight: FontWeight.w700,
                    )),
              ],
            ),
            const SizedBox(height: 6),
            // formula code block
            Container(
              width: double.infinity,
              padding: const EdgeInsets.all(8),
              decoration: BoxDecoration(
                color: colors.rice,
                borderRadius: BorderRadius.circular(4),
              ),
              child: Text(trace.expression,
                  style: const TextStyle(fontFamily: 'JetBrainsMonoSC')),
            ),
            if (trace.inputs.isNotEmpty) ...[
              const SizedBox(height: 6),
              Table(
                columnWidths: const {0: IntrinsicColumnWidth()},
                children: trace.inputs.entries
                    .map((e) => TableRow(children: [
                          Padding(
                            padding: const EdgeInsets.only(right: 12, bottom: 2),
                            child: Text(e.key, style: Theme.of(context).textTheme.bodySmall),
                          ),
                          Text(e.value, style: const TextStyle(fontFamily: 'JetBrainsMonoSC')),
                        ]))
                    .toList(),
              ),
            ],
            if (trace.lawRefs.isNotEmpty) ...[
              const SizedBox(height: 6),
              Wrap(
                spacing: 6,
                runSpacing: 6,
                children: trace.lawRefs.map((r) => _LawRefBadge(refId: r)).toList(),
              ),
            ],
          ],
        ),
      ),
    );
  }
}

/// D8 URN law-ref badge.
class _LawRefBadge extends StatelessWidget {
  const _LawRefBadge({required this.refId});
  final String refId;

  @override
  Widget build(BuildContext context) {
    final colors = StuchkaSemanticColors.of(context);
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
      decoration: BoxDecoration(
        border: Border.all(color: colors.inkBlue, width: 0.6),
        borderRadius: BorderRadius.circular(4),
      ),
      child: Text(refId, style: TextStyle(fontSize: 12, color: colors.inkBlue)),
    );
  }
}
