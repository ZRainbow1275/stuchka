import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../theme/stuchka_icons.dart';
import '../../../theme/stuchka_theme.dart';
import '../../accessibility/crisis/crisis_level3_dialog.dart';
import '../../accessibility/high_risk/disclaimer_payload.dart';
import '../../accessibility/high_risk/high_risk_ack.dart';
import '../../accessibility/high_risk/high_risk_gate.dart';

/// /case/:id/documents — the WebView2 host for the Tiptap editor (channel A) + the document-side
/// INV-10 high-risk trigger sites (compliance/05 §1 S-03 放弃请求项 / S-05 刑事报案材料导出).
///
/// R1a boundary (brief §6.1 / §7): the `webview_windows` package is P0-blocked on Dart 3 and the
/// editor内核 belongs to the 03-editor-workbench subtask. The shell ships the host SCAFFOLD + a
/// document-route placeholder that names the bundle it will load
/// (assets/web/editor/stuchka-editor.umd.js) and the channel-A CSP invariant. The live WebView load
/// test is gated until a Dart-3-compatible webview package is wired.
///
/// The query params steer the migrant-wage scenario (spec §4.9): `templates=` preselects the
/// arbitration / inspection templates, `action=export` opens the case-bundle export with its INV-10
/// gates.
class DocumentHostPage extends ConsumerWidget {
  const DocumentHostPage({
    super.key,
    required this.caseId,
    this.templates = const [],
    this.exportRequested = false,
  });

  final String caseId;

  /// Preselected document template ids (from `?templates=tpl_inspection,tpl_arbitration`).
  final List<String> templates;

  /// Whether the page was opened with `?action=export` (migrant-wage 导出卷宗 step).
  final bool exportRequested;

  /// The editor bundle asset path (loaded into the WebView host once a webview package is wired).
  static const String editorBundleAsset = 'assets/web/editor/stuchka-editor.umd.js';
  static const String editorIndexAsset = 'assets/web/editor/index.html';

  /// S-03 放弃任一仲裁请求项 (INV-10 real trigger). Gates the verbatim S-03 disclaimer carrying the
  /// {requested_item_name}/{amount}/{confidence} slots before any claim item is dropped, then
  /// AUDITS the acknowledgement (scene + timings) on confirm (INV-06 / compliance/05 §2.3).
  Future<void> _abandonClaim(BuildContext context, WidgetRef ref) async {
    final timings = await HighRiskGate.show(
      context,
      HighRiskScenario.abandonClaim,
      payload: const DisclaimerPayload(
        requestedItemName: '未签劳动合同二倍工资差额',
        amount: '48000',
        confidence: '较高',
      ),
    );
    if (timings == null) return; // user cancelled the gate
    await postHighRiskAck(
      ref,
      caseId: caseId,
      scenario: HighRiskScenario.abandonClaim,
      timings: timings,
    );
  }

  /// S-05 刑事报案材料导出（拒不支付劳动报酬罪）(INV-10 real trigger). Gates the verbatim S-05
  /// disclaimer + 8s+3s cooldown, then AUDITS the acknowledgement on confirm before the
  /// criminal-report bundle export proceeds (INV-06 / compliance/05 §2.3).
  Future<void> _exportCriminalReport(BuildContext context, WidgetRef ref) async {
    final timings = await HighRiskGate.show(
      context,
      HighRiskScenario.criminalReportExport,
    );
    if (timings == null) return;
    await postHighRiskAck(
      ref,
      caseId: caseId,
      scenario: HighRiskScenario.criminalReportExport,
      timings: timings,
    );
    if (!context.mounted) return;
    ScaffoldMessenger.of(context).showSnackBar(
      const SnackBar(content: Text('已确认导出刑事报案材料（前置程序由您自行核实）。')),
    );
  }

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final colors = StuchkaSemanticColors.of(context);
    return Scaffold(
      appBar: AppBar(title: const Text('文书工作台')),
      body: ListView(
        padding: const EdgeInsets.all(24),
        children: [
          // INV-07 Level-3 24h cooldown banner (compliance/05 §5.2): if active, the high-risk
          // document actions below are hard-blocked by the gate and this banner offers 误判申诉.
          const Level3CooldownGate(),
          const SizedBox(height: 12),
          Center(
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 520),
              child: Card(
                child: Padding(
                  padding: const EdgeInsets.all(24),
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      Icon(StuchkaIcons.document, size: 40, color: colors.inkBlue),
                      const SizedBox(height: 12),
                      Text('文书编辑器加载中', style: Theme.of(context).textTheme.titleLarge),
                      const SizedBox(height: 8),
                      const Text(
                        '编辑器内核（Tiptap / ProseMirror）将在 WebView2 宿主中加载 '
                        'assets/web/editor/stuchka-editor.umd.js。\n\n'
                        '通道 A（WebView↔编辑器）的 CSP 为 connect-src \'none\'，永不直接触达 Rust core；'
                        'ProseMirror Step 由外壳经通道 B 转交持久化。\n\n'
                        '编辑器内核装配属 03-editor-workbench 子任务。',
                        textAlign: TextAlign.center,
                      ),
                      const SizedBox(height: 16),
                      Container(
                        padding: const EdgeInsets.all(8),
                        decoration: BoxDecoration(
                          color: colors.rice,
                          borderRadius: BorderRadius.circular(4),
                        ),
                        child: Text('案件：$caseId',
                            style: const TextStyle(fontFamily: 'JetBrainsMonoSC')),
                      ),
                      if (templates.isNotEmpty) ...[
                        const SizedBox(height: 8),
                        Text('预选模板：${templates.join(' / ')}',
                            style: Theme.of(context).textTheme.bodySmall),
                      ],
                    ],
                  ),
                ),
              ),
            ),
          ),
          const SizedBox(height: 24),
          // Document-side INV-10 high-risk actions (real trigger sites).
          Text('高风险文书操作', style: Theme.of(context).textTheme.titleMedium),
          const SizedBox(height: 8),
          Wrap(
            spacing: 12,
            runSpacing: 8,
            children: [
              OutlinedButton.icon(
                icon: Icon(StuchkaIcons.close, size: 16, color: colors.riskHigh),
                label: const Text('放弃某项仲裁请求'),
                onPressed: () => _abandonClaim(context, ref),
              ),
              FilledButton.icon(
                icon: const Icon(StuchkaIcons.download, size: 16),
                label: Text(exportRequested ? '导出刑事报案材料（待确认）' : '导出刑事报案材料'),
                style: FilledButton.styleFrom(backgroundColor: colors.sealRed),
                onPressed: () => _exportCriminalReport(context, ref),
              ),
            ],
          ),
        ],
      ),
    );
  }
}
