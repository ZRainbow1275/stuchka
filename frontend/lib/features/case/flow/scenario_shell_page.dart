import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';

import '../../../theme/stuchka_icons.dart';
import '../../../theme/stuchka_theme.dart';
import 'migrant_wage_scenario.dart';

/// /scenario/migrant-wage/:step[?caseId=] — the 农民工欠薪 guided-path shell (spec frontend/04 §4.9).
///
/// A REAL user-navigable route: the home "农民工欠薪向导" entry lands here at step 1; the
/// [ScenarioProgressBar] pinned to the top shows 当前步号 / 总步数 + 后退 + 跳过 (with the §4.9 skip
/// confirmation), and the body lets the user enter the step's real destination page — carrying the
/// preset / subtype / focus / templates query params the flow defines so the destination honours them.
class ScenarioShellPage extends StatelessWidget {
  const ScenarioShellPage({
    super.key,
    required this.stepId,
    required this.caseId,
  });

  final String stepId;
  final String caseId;

  static const MigrantWageScenarioFlow flow = MigrantWageScenarioFlow();

  @override
  Widget build(BuildContext context) {
    final colors = StuchkaSemanticColors.of(context);
    final step = flow.steps.firstWhere(
      (s) => s.id == stepId,
      orElse: () => flow.steps.first,
    );
    final destination = step.routeOf(caseId);
    final next = flow.next(step.id);

    return Scaffold(
      appBar: AppBar(title: const Text('农民工欠薪 · 主用户路径')),
      body: Column(
        children: [
          // Pinned path progress bar (spec §4.9). Back / Skip wired to the real flow.
          ScenarioProgressBar(
            flow: flow,
            currentStepId: step.id,
            caseId: caseId,
          ),
          const Divider(height: 1),
          Expanded(
            child: ListView(
              padding: const EdgeInsets.all(24),
              children: [
                Text('第 ${flow.stepNumber(step.id)} / ${flow.total} 步 · ${step.titleZh}',
                    style: Theme.of(context).textTheme.titleLarge),
                const SizedBox(height: 12),
                Container(
                  padding: const EdgeInsets.all(12),
                  decoration: BoxDecoration(
                    color: colors.rice,
                    borderRadius: BorderRadius.circular(8),
                  ),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      const Text('此步骤将进入以下页面（已带场景预设参数）：'),
                      const SizedBox(height: 6),
                      SelectableText(destination,
                          style: const TextStyle(fontFamily: 'JetBrainsMonoSC')),
                    ],
                  ),
                ),
                const SizedBox(height: 16),
                FilledButton.icon(
                  icon: const Icon(StuchkaIcons.chevronRight, size: 16),
                  label: Text('进入「${step.titleZh}」页面'),
                  onPressed: () => context.go(destination),
                ),
                if (next != null) ...[
                  const SizedBox(height: 8),
                  OutlinedButton.icon(
                    icon: const Icon(StuchkaIcons.chevronRight, size: 16),
                    label: Text('下一步：${next.titleZh}'),
                    onPressed: () => context.go(
                      caseId.isEmpty
                          ? '/scenario/migrant-wage/${next.id}'
                          : '/scenario/migrant-wage/${next.id}?caseId=$caseId',
                    ),
                  ),
                ],
              ],
            ),
          ),
        ],
      ),
    );
  }
}
