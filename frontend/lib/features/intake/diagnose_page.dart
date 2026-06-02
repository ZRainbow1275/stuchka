import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../../ipc/dto/diagnose_dto.dart';
import '../../ipc/dto/dtos.dart';
import '../../ipc/dto/enums.dart';
import '../../ipc/rust_core_client.dart';
import '../../ipc/rust_core_providers.dart';
import '../../theme/stuchka_icons.dart';
import '../../theme/stuchka_theme.dart';
import '../accessibility/crisis/crisis_banners.dart';
import '../accessibility/crisis/crisis_detector.dart';
import '../accessibility/crisis/crisis_level3_dialog.dart';
import '../accessibility/high_risk/disclaimer_payload.dart';
import '../accessibility/high_risk/high_risk_ack.dart';
import '../accessibility/high_risk/high_risk_gate.dart';
import '../case/case_providers.dart';
import 'diagnose_question_tree.dart';

/// /intake/diagnose — the M1 诊断 page (spec prd §4.1 / frontend/04 §4.8).
///
/// PRIMARY path: a deterministic ≤7-question 问诊树 (prd §4.1.4) whose answers feed `POST /diagnose`
/// (the backend deterministic DiagnosisEngine, item A). The structured §4.1.2 DiagnosisOutput is
/// then rendered: subcategory + coverage_tier + recommended_procedures (主/并行/备用) + next_actions.
///
/// SECONDARY assist: the free-text LLM query is kept (it was the old primary path) but demoted to a
/// collapsible "补充自由描述" panel — never the primary diagnosis. The INV-07 crisis detector still
/// runs inline on any text the user types.
class DiagnosePage extends ConsumerStatefulWidget {
  const DiagnosePage({super.key, required this.caseId});
  final String caseId;

  @override
  ConsumerState<DiagnosePage> createState() => _DiagnosePageState();
}

class _DiagnosePageState extends ConsumerState<DiagnosePage> {
  final _detector = const CrisisDetector();
  final _answers = <DiagnoseAnswer>[];

  CaseAggregateDto? _caseAgg;
  bool _loadingCase = true;
  String? _loadError;

  bool _busy = false;
  String? _error;
  DiagnosisOutput? _output;

  // Secondary free-text assist.
  final _assist = TextEditingController();
  CrisisLevel _crisis = CrisisLevel.none;
  bool _assistExpanded = false;
  bool _assistBusy = false;
  LlmQueryResp? _assistResp;

  @override
  void initState() {
    super.initState();
    _loadCase();
  }

  @override
  void dispose() {
    _assist.dispose();
    super.dispose();
  }

  Future<void> _loadCase() async {
    final client = ref.read(rustCoreClientProvider);
    try {
      final agg = await client.getCase(widget.caseId);
      if (mounted) setState(() => _caseAgg = agg);
    } on RustCoreApiException catch (e) {
      if (mounted) setState(() => _loadError = '${e.code}: ${e.message}');
    } catch (e) {
      if (mounted) setState(() => _loadError = '$e');
    } finally {
      if (mounted) setState(() => _loadingCase = false);
    }
  }

  DiagnoseQuestionTree? get _tree {
    final agg = _caseAgg;
    if (agg == null) return null;
    return DiagnoseQuestionTree(identityType: agg.caseDto.identityType);
  }

  void _answer(DiagnoseQuestion q, String optionId, {bool skipped = false}) {
    setState(() {
      _answers
        ..removeWhere((a) => a.questionId == q.id)
        ..add(DiagnoseAnswer(questionId: q.id, optionId: optionId, skipped: skipped));
    });
    final state = _tree!.advance(_answers);
    if (state.isComplete) _runDiagnose();
  }

  void _restart() {
    setState(() {
      _answers.clear();
      _output = null;
      _error = null;
    });
  }

  Future<void> _runDiagnose() async {
    final agg = _caseAgg;
    if (agg == null) return;
    setState(() {
      _busy = true;
      _error = null;
    });
    final client = ref.read(rustCoreClientProvider);
    final c = agg.caseDto;
    try {
      final req = DiagnoseReq(
        caseId: widget.caseId,
        identityType: c.identityType,
        province: c.province,
        city: c.city,
        caseOccurredAt: c.caseOccurredAt,
        answers: List.of(_answers),
        kbVersionHash: c.kbVersionHash,
        freeText: _assist.text.trim().isEmpty ? null : _assist.text.trim(),
      );
      final out = await client.diagnose(req);
      if (mounted) setState(() => _output = out);
      ref.invalidate(caseAggregateProvider(widget.caseId));
    } on RustCoreApiException catch (e) {
      if (mounted) setState(() => _error = '${e.code}: ${e.message}');
    } catch (e) {
      if (mounted) setState(() => _error = '$e');
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  void _onAssistChanged(String t) {
    final lvl = _detector.scan(t);
    if (lvl != _crisis) setState(() => _crisis = lvl);
    // INV-07 三级响应 (compliance/05 §4.2 / §9.2): severe -> Level-3 (24h cooldown + appeal);
    // mid -> Level-2 soft reminder. Severe is NOT treated as mid.
    if (lvl == CrisisLevel.severe) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) CrisisLevel3Dialog.show(context, ref);
      });
    } else if (lvl == CrisisLevel.mid) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) CrisisLevel2Dialog.show(context);
      });
    }
  }

  Future<void> _runAssist() async {
    if (_assist.text.trim().isEmpty) return;
    setState(() => _assistBusy = true);
    final client = ref.read(rustCoreClientProvider);
    try {
      // Record the statement as a fact (real path), then ask the dispatcher for a free-text assist.
      await client.createFact(
        widget.caseId,
        CreateFactReq(
          category: FactCategory.other,
          statement: _assist.text.trim(),
          source: FactSource.userInput,
        ),
      );
      final resp = await client.llmQuery(
        LlmQueryReq(caseId: widget.caseId, prompt: _assist.text.trim()),
      );
      if (mounted) setState(() => _assistResp = resp);
    } on RustCoreApiException catch (e) {
      if (mounted) setState(() => _error = '${e.code}: ${e.message}');
    } catch (e) {
      if (mounted) setState(() => _error = '$e');
    } finally {
      if (mounted) setState(() => _assistBusy = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final colors = StuchkaSemanticColors.of(context);
    return Scaffold(
      appBar: AppBar(
        title: const Text('初步诊断'),
        actions: [
          TextButton.icon(
            icon: const Icon(StuchkaIcons.chevronRight),
            label: const Text('进入工作台'),
            onPressed: () => context.go('/case/${widget.caseId}'),
          ),
        ],
      ),
      body: _buildBody(colors),
    );
  }

  Widget _buildBody(StuchkaSemanticColors colors) {
    if (_loadingCase) {
      return const Center(child: CircularProgressIndicator());
    }
    if (_loadError != null) {
      return Center(
        child: Padding(
          padding: const EdgeInsets.all(24),
          child: Text('无法加载案件：$_loadError', style: TextStyle(color: colors.riskHigh)),
        ),
      );
    }

    final tree = _tree!;
    final state = tree.advance(_answers);

    return Column(
      children: [
        if (_crisis == CrisisLevel.light)
          CrisisLevel1Banner(onDismiss: () => setState(() => _crisis = CrisisLevel.none)),
        Expanded(
          child: ListView(
            padding: const EdgeInsets.all(16),
            children: [
              _ProgressHeader(
                asked: state.askedCount,
                total: tree.questions.length,
                heuristic: state.heuristicMode,
              ),
              const SizedBox(height: 16),
              if (_output == null && !_busy)
                _QuestionCard(
                  state: state,
                  onAnswer: _answer,
                )
              else if (_busy)
                const Padding(
                  padding: EdgeInsets.symmetric(vertical: 32),
                  child: Center(child: CircularProgressIndicator()),
                ),
              if (_error != null) ...[
                const SizedBox(height: 12),
                Text(_error!, style: TextStyle(color: colors.riskHigh)),
              ],
              if (_output != null) ...[
                const SizedBox(height: 16),
                _DiagnosisResultView(
                  output: _output!,
                  caseId: widget.caseId,
                  onRestart: _restart,
                ),
              ],
              const SizedBox(height: 24),
              _AssistPanel(
                controller: _assist,
                expanded: _assistExpanded,
                busy: _assistBusy,
                resp: _assistResp,
                onToggle: () => setState(() => _assistExpanded = !_assistExpanded),
                onChanged: _onAssistChanged,
                onSubmit: _runAssist,
              ),
            ],
          ),
        ),
      ],
    );
  }
}

/// The fixed progress header for the 问诊树 (current step / total + heuristic-mode notice).
class _ProgressHeader extends StatelessWidget {
  const _ProgressHeader({required this.asked, required this.total, required this.heuristic});
  final int asked;
  final int total;
  final bool heuristic;

  @override
  Widget build(BuildContext context) {
    final colors = StuchkaSemanticColors.of(context);
    final fraction = total == 0 ? 0.0 : (asked / total).clamp(0.0, 1.0);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          children: [
            Icon(StuchkaIcons.diagnose, size: 18, color: colors.inkBlue),
            const SizedBox(width: 8),
            Text('问诊进度 $asked / $total',
                style: const TextStyle(fontWeight: FontWeight.w700)),
          ],
        ),
        const SizedBox(height: 6),
        ClipRRect(
          borderRadius: BorderRadius.circular(4),
          child: LinearProgressIndicator(
            value: fraction,
            minHeight: 6,
            backgroundColor: colors.rice,
            valueColor: AlwaysStoppedAnimation<Color>(colors.inkBlue),
          ),
        ),
        if (heuristic) ...[
          const SizedBox(height: 8),
          Container(
            padding: const EdgeInsets.all(8),
            decoration: BoxDecoration(
              color: colors.rice,
              borderRadius: BorderRadius.circular(6),
              border: Border(left: BorderSide(color: colors.riskMid, width: 3)),
            ),
            child: Text(
              '您已跳过较多问题，系统将以较低置信度给出方向性参考（可随时补充信息以提高准确度）。',
              style: Theme.of(context).textTheme.bodySmall,
            ),
          ),
        ],
      ],
    );
  }
}

/// Renders the current 问诊树 question (one question at a time, with a 跳过此问 affordance).
class _QuestionCard extends StatelessWidget {
  const _QuestionCard({required this.state, required this.onAnswer});
  final DiagnoseTreeState state;
  final void Function(DiagnoseQuestion q, String optionId, {bool skipped}) onAnswer;

  @override
  Widget build(BuildContext context) {
    final q = state.current;
    final colors = StuchkaSemanticColors.of(context);
    if (q == null) {
      return const SizedBox.shrink();
    }
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(q.title, style: Theme.of(context).textTheme.titleMedium),
            if (q.subtitle != null) ...[
              const SizedBox(height: 4),
              Text(q.subtitle!, style: Theme.of(context).textTheme.bodySmall),
            ],
            const SizedBox(height: 12),
            ...q.options.map(
              (o) => Padding(
                padding: const EdgeInsets.only(bottom: 8),
                child: Material(
                  color: colors.rice,
                  borderRadius: BorderRadius.circular(8),
                  child: InkWell(
                    borderRadius: BorderRadius.circular(8),
                    onTap: () => onAnswer(q, o.id),
                    child: Padding(
                      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 14),
                      child: Row(
                        children: [
                          Icon(StuchkaIcons.chevronRight, size: 16, color: colors.inkBlue),
                          const SizedBox(width: 8),
                          Expanded(
                            child: Column(
                              crossAxisAlignment: CrossAxisAlignment.start,
                              children: [
                                Text(o.label),
                                if (o.hint != null)
                                  Text(o.hint!, style: Theme.of(context).textTheme.bodySmall),
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
            if (q.skippable)
              Align(
                alignment: Alignment.centerRight,
                child: TextButton.icon(
                  icon: const Icon(StuchkaIcons.chevronRight, size: 16),
                  label: const Text('跳过此问'),
                  onPressed: () => onAnswer(q, '__skip__', skipped: true),
                ),
              ),
          ],
        ),
      ),
    );
  }
}

/// Renders the structured §4.1.2 DiagnosisOutput.
class _DiagnosisResultView extends ConsumerWidget {
  const _DiagnosisResultView({
    required this.output,
    required this.caseId,
    required this.onRestart,
  });
  final DiagnosisOutput output;
  final String caseId;
  final VoidCallback onRestart;

  /// S-01 主动离职 (INV-10 real trigger). Gates the verbatim S-01 disclaimer + 8s+3s cooldown, then
  /// AUDITS the acknowledgement (scene + timings) on confirm (INV-06 / compliance/05 §2.3).
  Future<void> _resignAdvice(BuildContext context, WidgetRef ref) async {
    final timings = await HighRiskGate.show(context, HighRiskScenario.resignAdvice);
    if (timings == null) return; // user cancelled the gate
    await postHighRiskAck(
      ref,
      caseId: caseId,
      scenario: HighRiskScenario.resignAdvice,
      timings: timings,
    );
  }

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final colors = StuchkaSemanticColors.of(context);
    if (output.outOfScope) {
      return Card(
        child: Padding(
          padding: const EdgeInsets.all(16),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Row(
                children: [
                  Icon(StuchkaIcons.alert, color: colors.abstain),
                  const SizedBox(width: 8),
                  Text('暂无法给出确定结论',
                      style: TextStyle(color: colors.abstain, fontWeight: FontWeight.w700)),
                ],
              ),
              const SizedBox(height: 8),
              if (output.reasons.isNotEmpty)
                ...output.reasons.map((r) => Padding(
                      padding: const EdgeInsets.symmetric(vertical: 2),
                      child: Text('· $r'),
                    )),
              const SizedBox(height: 8),
              _NextActionsList(actions: output.nextActions, caseId: caseId),
              const SizedBox(height: 12),
              OutlinedButton.icon(
                icon: const Icon(StuchkaIcons.refresh, size: 16),
                label: const Text('重新问诊'),
                onPressed: onRestart,
              ),
            ],
          ),
        ),
      );
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        // 1. Conclusion: subcategory + coverage tier + confidence.
        Card(
          child: Padding(
            padding: const EdgeInsets.all(16),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text('诊断结论', style: Theme.of(context).textTheme.titleMedium),
                const SizedBox(height: 8),
                _kv(context, '身份分流', output.identityType.labelZh),
                if (output.subcategoryLabel != null)
                  _kv(context, '争议子类', output.subcategoryLabel!)
                else if (output.disputeCategoryId != null)
                  _kv(context, '争议子类 ID', output.disputeCategoryId!),
                _kv(context, '覆盖深度',
                    output.coverageTier == CoverageTier.makeDeep ? '做深（深度覆盖）' : '做能用（基础覆盖）'),
                _kv(context, '置信度', '${(output.confidence * 100).toStringAsFixed(0)}%'),
                _kv(context, '覆盖标签', _coverageTagLabel(output.coverageTag)),
              ],
            ),
          ),
        ),
        const SizedBox(height: 12),
        // 2. recommended_procedures 主 / 并行 / 备用.
        _ProceduresView(rec: output.recommendedProcedures, caseId: caseId),
        const SizedBox(height: 12),
        // 3. next_actions.
        Card(
          child: Padding(
            padding: const EdgeInsets.all(16),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text('下一步建议动作', style: Theme.of(context).textTheme.titleMedium),
                const SizedBox(height: 8),
                _NextActionsList(actions: output.nextActions, caseId: caseId),
              ],
            ),
          ),
        ),
        const SizedBox(height: 12),
        Row(
          children: [
            FilledButton.icon(
              icon: const Icon(StuchkaIcons.chevronRight, size: 16),
              label: const Text('进入工作台继续'),
              onPressed: () =>
                  _go(context, '/case/$caseId'),
            ),
            const SizedBox(width: 12),
            OutlinedButton.icon(
              icon: const Icon(StuchkaIcons.refresh, size: 16),
              label: const Text('重新问诊'),
              onPressed: onRestart,
            ),
          ],
        ),
        const SizedBox(height: 12),
        // S-01 主动离职 (INV-10 real trigger site, compliance/05 §1 S-01). The "按建议主动离职"
        // action gates the verbatim S-01 disclaimer + 8s+3s cooldown, then AUDITS the
        // acknowledgement on confirm (INV-06 / compliance/05 §2.3).
        OutlinedButton.icon(
          icon: Icon(StuchkaIcons.alert, size: 16, color: colors.riskHigh),
          label: const Text('我考虑按系统提示主动离职'),
          onPressed: () => _resignAdvice(context, ref),
        ),
      ],
    );
  }

  static Widget _kv(BuildContext context, String k, String v) {
    final colors = StuchkaSemanticColors.of(context);
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 3),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SizedBox(
            width: 96,
            child: Text(k, style: TextStyle(color: colors.ash)),
          ),
          Expanded(child: Text(v, style: const TextStyle(fontWeight: FontWeight.w500))),
        ],
      ),
    );
  }

  static String _coverageTagLabel(CoverageTag t) => switch (t) {
        CoverageTag.exact => '精确匹配',
        CoverageTag.approximate => '近似匹配',
        CoverageTag.boundary => '边界情形',
        CoverageTag.unknown => '未知（超出范围）',
      };
}

void _go(BuildContext context, String route) {
  // Local helper that uses go_router via the BuildContext extension.
  GoRouter.of(context).go(route);
}

/// The 主路径 / 并行 / 备用 three-column procedure comparison (spec frontend/04 §4.8).
class _ProceduresView extends StatelessWidget {
  const _ProceduresView({required this.rec, required this.caseId});
  final RecommendedProcedures rec;
  final String caseId;

  @override
  Widget build(BuildContext context) {
    if (rec.isEmpty) return const SizedBox.shrink();
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text('推荐程序（主路径 / 并行 / 备用）',
                style: Theme.of(context).textTheme.titleMedium),
            const SizedBox(height: 12),
            LayoutBuilder(
              builder: (context, constraints) {
                final cols = <Widget>[
                  if (rec.main != null)
                    _ProcedureColumn(role: '主路径', procedure: rec.main!, caseId: caseId),
                  if (rec.parallel != null)
                    _ProcedureColumn(role: '并行', procedure: rec.parallel!, caseId: caseId),
                  if (rec.fallback != null)
                    _ProcedureColumn(role: '备用', procedure: rec.fallback!, caseId: caseId),
                ];
                if (constraints.maxWidth < 560) {
                  return Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: [
                      for (var i = 0; i < cols.length; i++) ...[
                        if (i > 0) const Divider(),
                        cols[i],
                      ],
                    ],
                  );
                }
                return IntrinsicHeight(
                  child: Row(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      for (var i = 0; i < cols.length; i++) ...[
                        if (i > 0) const VerticalDivider(),
                        Expanded(child: cols[i]),
                      ],
                    ],
                  ),
                );
              },
            ),
          ],
        ),
      ),
    );
  }
}

class _ProcedureColumn extends StatelessWidget {
  const _ProcedureColumn({
    required this.role,
    required this.procedure,
    required this.caseId,
  });
  final String role;
  final RecommendedProcedure procedure;
  final String caseId;

  @override
  Widget build(BuildContext context) {
    final colors = StuchkaSemanticColors.of(context);
    final roleColor = switch (role) {
      '主路径' => colors.inkBlue,
      '并行' => colors.riskMid,
      _ => colors.ash,
    };
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 8),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Container(
            padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
            decoration: BoxDecoration(
              color: roleColor,
              borderRadius: BorderRadius.circular(4),
            ),
            child: Text(role, style: TextStyle(color: colors.paper, fontSize: 12)),
          ),
          const SizedBox(height: 6),
          Text(procedure.name, style: const TextStyle(fontWeight: FontWeight.w700)),
          if (procedure.expectedDuration != null) ...[
            const SizedBox(height: 4),
            Text('预计时长：${procedure.expectedDuration}',
                style: Theme.of(context).textTheme.bodySmall),
          ],
          if (procedure.deadlineNote != null)
            Text('时效：${procedure.deadlineNote}', style: Theme.of(context).textTheme.bodySmall),
          if (procedure.winProbabilityBand != null)
            Text('胜诉概率：${procedure.winProbabilityBand}',
                style: Theme.of(context).textTheme.bodySmall),
          if (procedure.note != null) ...[
            const SizedBox(height: 4),
            Text(procedure.note!, style: Theme.of(context).textTheme.bodySmall),
          ],
          const SizedBox(height: 8),
          OutlinedButton.icon(
            icon: const Icon(StuchkaIcons.document, size: 14),
            label: const Text('生成对应文书'),
            onPressed: () => _go(context, '/case/$caseId/documents'),
          ),
        ],
      ),
    );
  }
}

class _NextActionsList extends StatelessWidget {
  const _NextActionsList({required this.actions, required this.caseId});
  final List<DiagnoseNextAction> actions;
  final String caseId;

  @override
  Widget build(BuildContext context) {
    final colors = StuchkaSemanticColors.of(context);
    if (actions.isEmpty) {
      return Text('暂无具体动作建议。', style: Theme.of(context).textTheme.bodySmall);
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: actions.map((a) {
        return Padding(
          padding: const EdgeInsets.symmetric(vertical: 3),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Icon(StuchkaIcons.check, size: 16, color: colors.riskLow),
              const SizedBox(width: 8),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(a.label),
                    if (a.detail != null)
                      Text(a.detail!, style: Theme.of(context).textTheme.bodySmall),
                  ],
                ),
              ),
              if (a.route != null)
                TextButton(
                  onPressed: () => _go(context, a.route!),
                  child: const Text('前往'),
                ),
            ],
          ),
        );
      }).toList(),
    );
  }
}

/// The secondary free-text assist (collapsible; never the primary diagnosis path).
class _AssistPanel extends StatelessWidget {
  const _AssistPanel({
    required this.controller,
    required this.expanded,
    required this.busy,
    required this.resp,
    required this.onToggle,
    required this.onChanged,
    required this.onSubmit,
  });

  final TextEditingController controller;
  final bool expanded;
  final bool busy;
  final LlmQueryResp? resp;
  final VoidCallback onToggle;
  final ValueChanged<String> onChanged;
  final Future<void> Function() onSubmit;

  @override
  Widget build(BuildContext context) {
    final colors = StuchkaSemanticColors.of(context);
    return Card(
      color: colors.rice,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          ListTile(
            leading: Icon(StuchkaIcons.sourceInferred, color: colors.inkBlue),
            title: const Text('补充自由描述（辅助参考）'),
            subtitle: const Text('问诊树是主要诊断路径；此处的 AI 自由问答仅作补充，不替代结构化诊断。'),
            trailing: Icon(expanded ? StuchkaIcons.chevronDown : StuchkaIcons.chevronRight),
            onTap: onToggle,
          ),
          if (expanded)
            Padding(
              padding: const EdgeInsets.fromLTRB(16, 0, 16, 16),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  TextField(
                    controller: controller,
                    maxLines: 4,
                    onChanged: onChanged,
                    decoration: const InputDecoration(
                      labelText: '可在此补充自然语言描述',
                      border: OutlineInputBorder(),
                    ),
                  ),
                  const SizedBox(height: 8),
                  FilledButton.icon(
                    onPressed: busy ? null : () => onSubmit(),
                    icon: busy
                        ? const SizedBox(
                            width: 16, height: 16, child: CircularProgressIndicator(strokeWidth: 2))
                        : const Icon(StuchkaIcons.send),
                    label: const Text('提交补充描述'),
                  ),
                  if (resp != null) ...[
                    const SizedBox(height: 12),
                    _AssistResult(resp: resp!),
                  ],
                ],
              ),
            ),
        ],
      ),
    );
  }
}

class _AssistResult extends StatelessWidget {
  const _AssistResult({required this.resp});
  final LlmQueryResp resp;

  @override
  Widget build(BuildContext context) {
    final colors = StuchkaSemanticColors.of(context);
    return Container(
      width: double.infinity,
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: colors.paper,
        borderRadius: BorderRadius.circular(8),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Text('来源：${resp.sourceTag.labelZh}',
                  style: TextStyle(color: colors.inkBlue, fontWeight: FontWeight.w700)),
              const SizedBox(width: 12),
              Text('置信度：${(resp.confidence * 100).toStringAsFixed(0)}%'),
            ],
          ),
          const SizedBox(height: 8),
          Text(resp.content),
          if (resp.piiBlocked)
            Padding(
              padding: const EdgeInsets.only(top: 8),
              child: Text('高敏信息已拦截，已转本地处理', style: TextStyle(color: colors.sealRed)),
            ),
        ],
      ),
    );
  }
}
