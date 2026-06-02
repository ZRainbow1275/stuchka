import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:intl/intl.dart';

import '../../../ipc/rust_core_client.dart';
import '../../../ipc/rust_core_providers.dart';
import '../../../theme/stuchka_icons.dart';
import '../../../theme/stuchka_theme.dart';
import '../../accessibility/high_risk/disclaimer_payload.dart';
import '../../accessibility/high_risk/high_risk_ack.dart';
import '../../accessibility/high_risk/high_risk_gate.dart';
import '../case_providers.dart';
import 'rule_trace_view.dart';
import 'vs_cost_chart.dart';

/// /case/:id/calc — M9 rule-engine visualisation (`POST /compute/run`). Shows the RuleTraceView
/// per step + the win-cost horizontal bar chart. Level-4 KB blocks compensation (E_KB_OUTDATED).
class CompCalcPage extends ConsumerStatefulWidget {
  const CompCalcPage({super.key, required this.caseId});
  final String caseId;

  @override
  ConsumerState<CompCalcPage> createState() => _CompCalcPageState();
}

class _CompCalcPageState extends ConsumerState<CompCalcPage> {
  final _monthlyWage = TextEditingController(text: '8000');
  final _settlementAmount = TextEditingController();
  bool _busy = false;
  String? _error;
  List<RuleTrace> _traces = [];
  List<CostBar> _bars = [];

  /// The rule-engine computed 应得金额 (compliance/05 §3.2 {expected_amount}). 0 until a run completes.
  double _expectedAmount = 0;

  @override
  void dispose() {
    _monthlyWage.dispose();
    _settlementAmount.dispose();
    super.dispose();
  }

  Future<void> _run() async {
    setState(() {
      _busy = true;
      _error = null;
    });
    final client = ref.read(rustCoreClientProvider);
    final agg = ref.read(caseAggregateProvider(widget.caseId)).value;
    final province = agg?.caseDto.province ?? '广东省';
    final city = agg?.caseDto.city ?? '深圳市';
    final occurred = agg?.caseDto.caseOccurredAt ?? DateFormat('yyyy-MM-dd').format(DateTime.now());
    try {
      final resp = await client.computeRun({
        'caseId': widget.caseId,
        'scenarios': ['severance'],
        'province': province,
        'city': city,
        'wageData': {'monthlyWage': _monthlyWage.text.trim()},
        'period': {'from': '2022-01-01', 'to': occurred},
      });
      _ingest(resp);
    } on RustCoreApiException catch (e) {
      setState(() => _error = e.isKbOutdated
          ? '知识库已超过 30 天（Level4），赔偿计算已停用，请先更新知识库。'
          : '${e.code}: ${e.message}');
    } catch (e) {
      setState(() => _error = '$e');
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  void _ingest(Map<String, dynamic> resp) {
    final traces = <RuleTrace>[];
    final results = (resp['results'] as List?) ?? const [];
    for (final r in results) {
      final m = (r as Map).cast<String, dynamic>();
      final outcome = (m['outcome'] as Map?)?.cast<String, dynamic>() ?? {};
      final lawRefs = ((outcome['law_refs'] as List?) ?? const [])
          .map((e) => ((e as Map)['urn'] ?? '').toString())
          .toList();
      final derivation = (outcome['derivation'] as List?) ?? const [];
      for (final d in derivation) {
        final dm = (d as Map).cast<String, dynamic>();
        traces.add(RuleTrace(
          labelZh: (dm['label_zh'] ?? '') as String,
          expression: (dm['expression'] ?? '') as String,
          value: '${dm['value'] ?? ''}',
          lawRefs: lawRefs,
        ));
      }
      if (derivation.isEmpty && m['amount'] != null) {
        traces.add(RuleTrace(
          labelZh: (m['scenario'] ?? '结果') as String,
          expression: '规则引擎计算',
          value: '${m['amount']}',
          lawRefs: lawRefs,
        ));
      }
    }
    final preTax = double.tryParse('${resp['preTax'] ?? resp['pre_tax'] ?? 0}') ?? 0;
    setState(() {
      _traces = traces;
      _expectedAmount = preTax;
      _bars = [
        CostBar('预期收益', preTax),
        CostBar('仲裁费', 10),
        CostBar('律师费区间', preTax * 0.1),
        CostBar('时间成本', preTax * 0.05),
      ];
    });
  }

  /// S-02 settlement-below-80% confirm (compliance/05 §1 S-02 / §3.2). REAL trigger: the user enters
  /// a settlement amount and presses 签署和解协议. When the amount is below 80% of the rule-engine
  /// computed 应得金额, the INV-10 gate fires with the verbatim S-02 disclaimer (carrying
  /// {expected_amount}/{settlement_amount}/{ratio}). Above 80% it proceeds without the gate.
  Future<void> _signSettlement() async {
    final settlement = double.tryParse(_settlementAmount.text.trim()) ?? 0;
    if (_expectedAmount <= 0 || settlement <= 0) {
      setState(() => _error = '请先运行计算，并填写一个有效的和解金额。');
      return;
    }
    final ratio = settlement / _expectedAmount * 100;
    if (ratio < 80) {
      final timings = await HighRiskGate.show(
        context,
        HighRiskScenario.settlementBelow80,
        payload: DisclaimerPayload(
          expectedAmount: _expectedAmount.toStringAsFixed(0),
          settlementAmount: settlement.toStringAsFixed(0),
          ratio: ratio.toStringAsFixed(0),
        ),
      );
      if (timings == null) return; // user cancelled the gate
      // INV-10 §2.3 / INV-06: persist the acknowledgement (scene + timings) to the audit chain —
      // never drop the timings (the Wave-1 re-review gap).
      await postHighRiskAck(
        ref,
        caseId: widget.caseId,
        scenario: HighRiskScenario.settlementBelow80,
        timings: timings,
      );
    }
    if (!mounted) return;
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(content: Text('已记录和解金额 ¥ ${settlement.toStringAsFixed(0)}（占应得 ${ratio.toStringAsFixed(0)}%）')),
    );
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('赔偿计算器 · 规则可视化')),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          Row(
            children: [
              Expanded(
                child: TextField(
                  controller: _monthlyWage,
                  keyboardType: TextInputType.number,
                  decoration: const InputDecoration(
                    labelText: '月工资（元）',
                    border: OutlineInputBorder(),
                  ),
                ),
              ),
              const SizedBox(width: 12),
              FilledButton.icon(
                onPressed: _busy ? null : _run,
                icon: _busy
                    ? const SizedBox(width: 16, height: 16, child: CircularProgressIndicator(strokeWidth: 2))
                    : const Icon(StuchkaIcons.calc),
                label: const Text('计算'),
              ),
            ],
          ),
          if (_error != null) ...[
            const SizedBox(height: 12),
            Text(
              _error!,
              style: TextStyle(color: StuchkaSemanticColors.of(context).riskHigh),
            ),
          ],
          const SizedBox(height: 16),
          Text('胜诉成本对比（水平条形图）', style: Theme.of(context).textTheme.titleMedium),
          const SizedBox(height: 8),
          if (_bars.isNotEmpty) VsCostChart(bars: _bars),
          const SizedBox(height: 16),
          Text('规则引擎推导', style: Theme.of(context).textTheme.titleMedium),
          const SizedBox(height: 8),
          RuleTraceView(traces: _traces),
          const SizedBox(height: 24),
          // S-02 和解协议签署入口 (INV-10 real trigger site).
          Text('和解协议', style: Theme.of(context).textTheme.titleMedium),
          const SizedBox(height: 8),
          Text(
            '若对方提出和解金额低于系统计算应得金额的 80%，签署前将强制弹出完整免责与冷静期。',
            style: Theme.of(context).textTheme.bodySmall,
          ),
          const SizedBox(height: 8),
          Row(
            children: [
              Expanded(
                child: TextField(
                  controller: _settlementAmount,
                  keyboardType: TextInputType.number,
                  decoration: const InputDecoration(
                    labelText: '和解金额（元）',
                    border: OutlineInputBorder(),
                  ),
                ),
              ),
              const SizedBox(width: 12),
              FilledButton.icon(
                onPressed: _expectedAmount <= 0 ? null : _signSettlement,
                icon: const Icon(StuchkaIcons.signature),
                label: const Text('签署和解协议'),
              ),
            ],
          ),
        ],
      ),
    );
  }
}
