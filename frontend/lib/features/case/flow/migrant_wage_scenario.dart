// 主用户路径 · 农民工欠薪场景 (spec frontend/04 §4.9; prd §3.6 / §0512 §3.2.6 前 5 类做深).
//
// The migrant-wage arrears journey is the R1a flagship end-to-end path (§4.11: 端到端 全实装). It is
// modelled as an ordered list of [ScenarioStep]s — each step maps to a real workbench route — plus a
// fixed progress bar pinned to the top of every step page (当前步号 / 总步数 + 后退 + 跳过). Skipping
// pops a second confirmation ("跳过此步骤可能导致材料包缺失，是否继续？", spec §4.9 行动导向).
//
// This file is data + a presentation widget only; it holds no business logic (the diagnosis itself is
// the backend DiagnosisEngine, the calc the rule-engine). It never fabricates data — every route it
// emits is a real page wired in app/router.dart.

import 'package:flutter/widgets.dart';
import 'package:flutter/material.dart'
    show
        AlertDialog,
        FilledButton,
        TextButton,
        Theme,
        LinearProgressIndicator,
        Material,
        showDialog;
import 'package:go_router/go_router.dart';

import '../../../theme/stuchka_icons.dart';

/// One step on a guided scenario path (spec §4.9). `id` is a stable key; `routeOf` builds the real
/// router location for a given case id (the §4.9 illustrative `pageRoute` query strings are folded
/// into the actual `/case/:id/...` sub-routes wired in `app/router.dart`).
@immutable
class ScenarioStep {
  const ScenarioStep({
    required this.id,
    required this.titleZh,
    required this.routeBuilder,
    this.skippable = true,
  });

  /// Stable step id (matches the spec §4.9 step ids).
  final String id;

  /// Chinese step label shown in the progress bar.
  final String titleZh;

  /// Builds the concrete router location for this step given the active case id. The intake steps
  /// (identity / diagnose) precede a case id, so they ignore the argument.
  final String Function(String caseId) routeBuilder;

  /// Whether the step may be skipped (the diagnosis anchor is not skippable — without it the rest of
  /// the path cannot compute, spec §4.1.4 dispute-area is mandatory).
  final bool skippable;

  /// Resolve the route for [caseId].
  String routeOf(String caseId) => routeBuilder(caseId);
}

/// The 农民工欠薪 main user path (spec §4.9 — 8 steps, verbatim order). Preset query params follow
/// the spec (identity preset, diagnose subtype, evidence focus, calc preset, document templates).
class MigrantWageScenarioFlow {
  const MigrantWageScenarioFlow();

  /// The ordered steps. The two intake steps run before a case exists; the case steps use the real
  /// `/case/:id/...` routes. Query presets steer each page to the migrant-wage configuration.
  List<ScenarioStep> get steps => <ScenarioStep>[
        ScenarioStep(
          id: 'identity',
          titleZh: '选择身份',
          routeBuilder: (_) => '/intake/identity?preset=migrant_worker',
          skippable: false,
        ),
        ScenarioStep(
          id: 'diagnose',
          titleZh: '诊断争议',
          routeBuilder: (caseId) => caseId.isEmpty
              ? '/intake/diagnose?subtype=wage_arrears'
              : '/intake/diagnose?caseId=$caseId&subtype=wage_arrears',
          skippable: false,
        ),
        ScenarioStep(
          id: 'evidence',
          titleZh: '采集证据',
          routeBuilder: (caseId) =>
              '/case/$caseId/evidence?focus=written,chat,third_party',
        ),
        ScenarioStep(
          id: 'calc',
          titleZh: '计算金额',
          routeBuilder: (caseId) => '/case/$caseId/calc?preset=wage_arrears_50pct',
        ),
        ScenarioStep(
          id: 'deadline',
          titleZh: '核对时效',
          routeBuilder: (caseId) => '/case/$caseId?focus=deadline',
        ),
        ScenarioStep(
          id: 'procedure',
          titleZh: '程序对比',
          routeBuilder: (caseId) => '/case/$caseId/procedure-compare',
        ),
        ScenarioStep(
          id: 'documents',
          titleZh: '生成文书',
          routeBuilder: (caseId) =>
              '/case/$caseId/documents?templates=tpl_inspection,tpl_arbitration',
        ),
        ScenarioStep(
          id: 'export',
          titleZh: '导出卷宗',
          routeBuilder: (caseId) => '/case/$caseId/documents?action=export',
        ),
      ];

  /// Total step count (the progress bar denominator).
  int get total => steps.length;

  /// The 1-based index of a step id (for "第 N / total 步"), or -1 if unknown.
  int stepNumber(String stepId) {
    final i = steps.indexWhere((s) => s.id == stepId);
    return i < 0 ? -1 : i + 1;
  }

  /// The step after [stepId], or null when [stepId] is the final step.
  ScenarioStep? next(String stepId) {
    final i = steps.indexWhere((s) => s.id == stepId);
    if (i < 0 || i + 1 >= steps.length) return null;
    return steps[i + 1];
  }

  /// The step before [stepId], or null when [stepId] is the first step.
  ScenarioStep? previous(String stepId) {
    final i = steps.indexWhere((s) => s.id == stepId);
    if (i <= 0) return null;
    return steps[i - 1];
  }
}

/// The fixed path progress bar pinned to the top of every scenario step page (spec §4.9: 每个步骤
/// 页面顶部固定一条"路径进度条"，显示当前步号 / 总步数 + 后退 + 跳过).
///
/// Back navigates to the previous step; Skip pops the §4.9 confirmation dialog before advancing.
class ScenarioProgressBar extends StatelessWidget {
  const ScenarioProgressBar({
    super.key,
    required this.flow,
    required this.currentStepId,
    required this.caseId,
  });

  final MigrantWageScenarioFlow flow;
  final String currentStepId;
  final String caseId;

  @override
  Widget build(BuildContext context) {
    final n = flow.stepNumber(currentStepId);
    final total = flow.total;
    final prev = flow.previous(currentStepId);
    final current = flow.steps.firstWhere((s) => s.id == currentStepId);
    final scheme = Theme.of(context).colorScheme;

    return Material(
      color: scheme.surface,
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
        child: Row(
          children: [
            TextButton.icon(
              onPressed: prev == null
                  ? null
                  : () => context.go(prev.routeOf(caseId)),
              icon: const Icon(StuchkaIcons.chevronRight, size: 16),
              label: const Text('后退'),
            ),
            const SizedBox(width: 8),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text('第 $n / $total 步 · ${current.titleZh}'),
                  const SizedBox(height: 4),
                  LinearProgressIndicator(
                    value: total == 0 ? 0 : n / total,
                  ),
                ],
              ),
            ),
            const SizedBox(width: 8),
            TextButton(
              onPressed: current.skippable
                  ? () => _confirmSkip(context)
                  : null,
              child: const Text('跳过'),
            ),
          ],
        ),
      ),
    );
  }

  /// Spec §4.9 second confirmation before skipping a step.
  Future<void> _confirmSkip(BuildContext context) async {
    final proceed = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: const Text('跳过此步骤'),
        content: const Text('跳过此步骤可能导致材料包缺失，是否继续？'),
        actions: [
          FilledButton(
            onPressed: () => Navigator.of(ctx).pop(false),
            child: const Text('返回继续'),
          ),
          TextButton(
            onPressed: () => Navigator.of(ctx).pop(true),
            child: const Text('仍然跳过'),
          ),
        ],
      ),
    );
    if (proceed != true) return;
    final next = flow.next(currentStepId);
    if (next != null && context.mounted) {
      context.go(next.routeOf(caseId));
    }
  }
}
