import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:intl/intl.dart';

import '../../../ipc/dto/dtos.dart';
import '../../../ipc/rust_core_client.dart';
import '../../../ipc/rust_core_providers.dart';
import '../../../theme/stuchka_icons.dart';
import '../../../theme/stuchka_theme.dart';
import '../case_providers.dart';

/// M16 履行 / 执行监控时间线 (prd/04 §4.10). The user records the instrument's payment schedule
/// (按期付款时间表); the page calls `POST /performance/evaluate` and renders the REAL per-installment
/// state timeline (未到期 / 按期 / 逾期履行 / 违约) plus, on any breach, the §250 enforcement
/// countdown the engine reanchors at the first breach. Every state and every day-count is the
/// engine's own output — the page fabricates nothing. Lucide icons only; zero emoji.
class PerformanceTimelinePage extends ConsumerStatefulWidget {
  const PerformanceTimelinePage({super.key, required this.caseId});

  final String caseId;

  @override
  ConsumerState<PerformanceTimelinePage> createState() => _PerformanceTimelinePageState();
}

class _PerformanceTimelinePageState extends ConsumerState<PerformanceTimelinePage> {
  static final _fmt = DateFormat('yyyy-MM-dd');

  String _instrumentKind = 'award';
  final _effectiveDate = TextEditingController(text: _fmt.format(DateTime.now()));
  final List<_InstallmentRow> _rows = [_InstallmentRow()];

  bool _busy = false;
  String? _error;
  PerformanceStatusDto? _status;

  @override
  void dispose() {
    _effectiveDate.dispose();
    for (final r in _rows) {
      r.dispose();
    }
    super.dispose();
  }

  void _addRow() => setState(() => _rows.add(_InstallmentRow()));

  void _removeRow(int i) => setState(() {
        _rows.removeAt(i).dispose();
        if (_rows.isEmpty) _rows.add(_InstallmentRow());
      });

  Future<void> _pickDate(TextEditingController ctrl) async {
    final initial = DateTime.tryParse(ctrl.text.trim()) ?? DateTime.now();
    final picked = await showDatePicker(
      context: context,
      initialDate: initial,
      firstDate: DateTime(2000),
      lastDate: DateTime(2100),
    );
    if (picked != null) ctrl.text = _fmt.format(picked);
  }

  Future<void> _evaluate() async {
    // Build the schedule from rows that carry both a due date and an amount; skip blank rows.
    final installments = <PerformanceInstallmentReq>[];
    for (final r in _rows) {
      final due = r.dueOn.text.trim();
      final amount = r.amount.text.trim();
      if (due.isEmpty && amount.isEmpty) continue; // blank row — ignore
      if (DateTime.tryParse(due) == null) {
        setState(() => _error = '每期的应付款日必须是合法日期（yyyy-MM-dd）。');
        return;
      }
      if (_Amount.tryParse(amount) == null) {
        setState(() => _error = '每期的应付金额必须是合法数字。');
        return;
      }
      final paidOn = r.paidOn.text.trim();
      final paidAmount = r.paidAmount.text.trim();
      if (paidOn.isNotEmpty && DateTime.tryParse(paidOn) == null) {
        setState(() => _error = '实际付款日必须是合法日期（yyyy-MM-dd）。');
        return;
      }
      if (paidAmount.isNotEmpty && _Amount.tryParse(paidAmount) == null) {
        setState(() => _error = '实际付款金额必须是合法数字。');
        return;
      }
      installments.add(PerformanceInstallmentReq(
        dueOn: due,
        amount: amount,
        paidOn: paidOn.isEmpty ? null : paidOn,
        paidAmount: paidAmount.isEmpty ? null : paidAmount,
      ));
    }
    if (installments.isEmpty) {
      setState(() => _error = '请至少录入一期履行计划（应付款日 + 应付金额）。');
      return;
    }
    if (DateTime.tryParse(_effectiveDate.text.trim()) == null) {
      setState(() => _error = '文书生效日必须是合法日期（yyyy-MM-dd）。');
      return;
    }

    setState(() {
      _busy = true;
      _error = null;
    });
    final client = ref.read(rustCoreClientProvider);
    final agg = ref.read(caseAggregateProvider(widget.caseId)).value;
    final province = agg?.caseDto.province ?? '广东省';
    final city = agg?.caseDto.city ?? '深圳市';
    try {
      final status = await client.evaluatePerformance(PerformanceEvalReq(
        caseId: widget.caseId,
        province: province,
        city: city,
        instrumentKind: _instrumentKind,
        effectiveDate: _effectiveDate.text.trim(),
        installments: installments,
      ));
      setState(() => _status = status);
    } on RustCoreApiException catch (e) {
      setState(() {
        _status = null;
        _error = e.isKbOutdated
            ? '知识库已超过 30 天（Level4），履行 / 执行时效计算已停用，请先更新知识库。'
            : e.isRuleNoCoverage
                ? '当前地区暂未纳入执行时效规则覆盖。'
                : '${e.code}: ${e.message}';
      });
    } catch (e) {
      setState(() {
        _status = null;
        _error = '$e';
      });
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('履行 / 执行监控')),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          _instrumentForm(context),
          const SizedBox(height: 16),
          Text('按期付款时间表', style: Theme.of(context).textTheme.titleMedium),
          const SizedBox(height: 8),
          for (var i = 0; i < _rows.length; i++) _rowCard(context, i),
          const SizedBox(height: 8),
          Align(
            alignment: Alignment.centerLeft,
            child: TextButton.icon(
              onPressed: _addRow,
              icon: const Icon(StuchkaIcons.add),
              label: const Text('增加一期'),
            ),
          ),
          const SizedBox(height: 8),
          FilledButton.icon(
            onPressed: _busy ? null : _evaluate,
            icon: _busy
                ? const SizedBox(width: 16, height: 16, child: CircularProgressIndicator(strokeWidth: 2))
                : const Icon(StuchkaIcons.deadline),
            label: const Text('评估履行情况'),
          ),
          if (_error != null) ...[
            const SizedBox(height: 12),
            Text(_error!, style: TextStyle(color: StuchkaSemanticColors.of(context).riskHigh)),
          ],
          if (_status != null) ...[
            const SizedBox(height: 24),
            _results(context, _status!),
          ],
        ],
      ),
    );
  }

  Widget _instrumentForm(BuildContext context) {
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text('生效文书', style: Theme.of(context).textTheme.titleMedium),
            const SizedBox(height: 12),
            Row(
              children: [
                Expanded(
                  child: DropdownButtonFormField<String>(
                    initialValue: _instrumentKind,
                    decoration: const InputDecoration(labelText: '文书类型', border: OutlineInputBorder()),
                    items: const [
                      DropdownMenuItem(value: 'award', child: Text('仲裁裁决书')),
                      DropdownMenuItem(value: 'judgment', child: Text('法院判决')),
                      DropdownMenuItem(value: 'settlement', child: Text('和解协议')),
                    ],
                    onChanged: (v) => setState(() => _instrumentKind = v ?? 'award'),
                  ),
                ),
                const SizedBox(width: 12),
                Expanded(child: _dateField('文书生效日', _effectiveDate)),
              ],
            ),
          ],
        ),
      ),
    );
  }

  Widget _rowCard(BuildContext context, int i) {
    final row = _rows[i];
    return Card(
      margin: const EdgeInsets.only(bottom: 12),
      child: Padding(
        padding: const EdgeInsets.all(12),
        child: Column(
          children: [
            Row(
              children: [
                Text('第 ${i + 1} 期', style: Theme.of(context).textTheme.titleSmall),
                const Spacer(),
                IconButton(
                  tooltip: '删除本期',
                  onPressed: () => _removeRow(i),
                  icon: const Icon(StuchkaIcons.close),
                ),
              ],
            ),
            Row(
              children: [
                Expanded(child: _dateField('应付款日', row.dueOn)),
                const SizedBox(width: 12),
                Expanded(child: _amountField('应付金额（元）', row.amount)),
              ],
            ),
            const SizedBox(height: 8),
            Row(
              children: [
                Expanded(child: _dateField('实际付款日（未付留空）', row.paidOn)),
                const SizedBox(width: 12),
                Expanded(child: _amountField('实际付款金额（未付留空）', row.paidAmount)),
              ],
            ),
          ],
        ),
      ),
    );
  }

  Widget _dateField(String label, TextEditingController ctrl) {
    return TextField(
      controller: ctrl,
      decoration: InputDecoration(
        labelText: label,
        border: const OutlineInputBorder(),
        suffixIcon: IconButton(
          icon: const Icon(StuchkaIcons.deadline, size: 18),
          onPressed: () => _pickDate(ctrl),
        ),
      ),
    );
  }

  Widget _amountField(String label, TextEditingController ctrl) {
    return TextField(
      controller: ctrl,
      keyboardType: const TextInputType.numberWithOptions(decimal: true),
      inputFormatters: [FilteringTextInputFormatter.allow(RegExp(r'[0-9.]'))],
      decoration: InputDecoration(labelText: label, border: const OutlineInputBorder()),
    );
  }

  Widget _results(BuildContext context, PerformanceStatusDto s) {
    final colors = StuchkaSemanticColors.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text('履行结果', style: Theme.of(context).textTheme.titleMedium),
        const SizedBox(height: 8),
        _summaryCard(context, colors, s),
        const SizedBox(height: 12),
        for (final inst in s.installments) _installmentTile(context, colors, inst),
        if (s.enforcement != null) ...[
          const SizedBox(height: 12),
          _enforcementCard(context, colors, s),
        ],
      ],
    );
  }

  Widget _summaryCard(BuildContext context, StuchkaSemanticColors colors, PerformanceStatusDto s) {
    final breached = s.hasBreach;
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Icon(breached ? StuchkaIcons.alert : StuchkaIcons.check,
                    color: breached ? colors.sealRed : colors.riskLow, size: 20),
                const SizedBox(width: 8),
                Text(
                  breached ? '存在违约期，已触发执行时效倒计时' : '暂未发现违约',
                  style: Theme.of(context).textTheme.titleMedium,
                ),
              ],
            ),
            const SizedBox(height: 8),
            Text('应付总额：¥ ${s.totalAmount}'),
            Text('已付总额：¥ ${s.totalPaid}'),
            Text('累计违约缺口：¥ ${s.totalShortfall}',
                style: TextStyle(color: breached ? colors.sealRed : null)),
            if (s.firstBreachAt != null) Text('首个违约到期日：${s.firstBreachAt}'),
          ],
        ),
      ),
    );
  }

  Widget _installmentTile(
      BuildContext context, StuchkaSemanticColors colors, InstallmentAssessmentDto inst) {
    final (color, label) = _stateView(colors, inst);
    return Card(
      margin: const EdgeInsets.only(bottom: 8),
      child: Padding(
        padding: const EdgeInsets.all(12),
        child: Row(
          children: [
            Container(width: 6, height: 40, color: color),
            const SizedBox(width: 12),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text('应付款日 ${inst.dueOn} · ¥ ${inst.amount}',
                      style: Theme.of(context).textTheme.bodyMedium),
                  if (inst.paidOn != null)
                    Text('实付 ${inst.paidOn} · ¥ ${inst.paidAmount ?? '0'}',
                        style: Theme.of(context).textTheme.bodySmall),
                ],
              ),
            ),
            Container(
              padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 4),
              decoration: BoxDecoration(
                color: color.withValues(alpha: 0.12),
                borderRadius: BorderRadius.circular(12),
              ),
              child: Text(label, style: TextStyle(color: color, fontWeight: FontWeight.w600)),
            ),
          ],
        ),
      ),
    );
  }

  (Color, String) _stateView(StuchkaSemanticColors colors, InstallmentAssessmentDto inst) {
    switch (inst.state) {
      case 'not_yet_due':
        return (colors.inkBlue, '未到期');
      case 'paid_on_time':
        return (colors.riskLow, '按期履行');
      case 'paid_late':
        return (colors.riskMid, '逾期履行（迟 ${inst.daysLate ?? 0} 天）');
      case 'overdue':
        return (
          colors.sealRed,
          '违约（逾期 ${inst.overdueDays ?? 0} 天 · 缺口 ¥ ${inst.shortfall ?? '0'}）'
        );
      default:
        return (colors.riskMid, inst.state);
    }
  }

  Widget _enforcementCard(
      BuildContext context, StuchkaSemanticColors colors, PerformanceStatusDto s) {
    final ef = s.enforcement!;
    final body = StringBuffer();
    if (ef.isOk) {
      if (ef.isExpired) {
        body.writeln('以首个违约到期日（${s.firstBreachAt}）为锚，执行申请两年时效（民诉法 §250）已届满，'
            '逾期 ${ef.overdueDays ?? 0} 天。');
      } else {
        body.writeln('以首个违约到期日（${s.firstBreachAt}）为锚的执行申请时效（民诉法 §250，两年）：'
            '原始剩余 ${ef.rawRemainingDays} 天，建议在 ${ef.bufferedRemainingDays} 天内申请强制执行'
            '（已含 10% 安全余量）。');
      }
    } else {
      body.writeln('执行时效未能给出确定结论：${ef.reasons.join('；')}');
    }
    return Card(
      color: colors.rice,
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Icon(StuchkaIcons.deadline, color: colors.sealRed, size: 20),
                const SizedBox(width: 8),
                Text('执行申请时效倒计时', style: Theme.of(context).textTheme.titleMedium),
              ],
            ),
            const SizedBox(height: 8),
            Text(body.toString().trim()),
            const SizedBox(height: 6),
            Text(
              '本结果须经人工二次确认，系统不作时效承诺。',
              style: Theme.of(context).textTheme.bodySmall?.copyWith(color: colors.inkBlue),
            ),
            if (ef.lawRefs.isNotEmpty) ...[
              const SizedBox(height: 6),
              Text(
                '依据：${ef.lawRefs.join('；')}',
                style: const TextStyle(fontFamily: 'JetBrainsMonoSC', fontSize: 11),
              ),
            ],
          ],
        ),
      ),
    );
  }
}

/// Mutable controllers backing one installment input row.
class _InstallmentRow {
  final TextEditingController dueOn = TextEditingController();
  final TextEditingController amount = TextEditingController();
  final TextEditingController paidOn = TextEditingController();
  final TextEditingController paidAmount = TextEditingController();

  void dispose() {
    dueOn.dispose();
    amount.dispose();
    paidOn.dispose();
    paidAmount.dispose();
  }
}

/// Minimal decimal validator used only to reject obviously malformed amount input before sending it
/// to the engine (the engine remains the single source of truth for the monetary arithmetic).
class _Amount {
  static String? tryParse(String s) {
    final t = s.trim();
    if (t.isEmpty) return null;
    if (!RegExp(r'^\d+(\.\d+)?$').hasMatch(t)) return null;
    return t;
  }
}
