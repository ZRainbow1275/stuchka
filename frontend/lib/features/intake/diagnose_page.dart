import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../../ipc/dto/dtos.dart';
import '../../ipc/dto/enums.dart';
import '../../ipc/rust_core_client.dart';
import '../../ipc/rust_core_providers.dart';
import '../../theme/stuchka_icons.dart';
import '../../theme/stuchka_theme.dart';
import '../accessibility/crisis/crisis_banners.dart';
import '../accessibility/crisis/crisis_detector.dart';

/// /intake/diagnose — first facts + an LLM diagnose query. Demonstrates the INV-07 crisis detector
/// running inline on user text before submission (Level1 banner / Level2 dialog).
class DiagnosePage extends ConsumerStatefulWidget {
  const DiagnosePage({super.key, required this.caseId});
  final String caseId;

  @override
  ConsumerState<DiagnosePage> createState() => _DiagnosePageState();
}

class _DiagnosePageState extends ConsumerState<DiagnosePage> {
  final _input = TextEditingController();
  final _detector = const CrisisDetector();
  CrisisLevel _crisis = CrisisLevel.none;
  bool _busy = false;
  String? _error;
  LlmQueryResp? _resp;

  @override
  void dispose() {
    _input.dispose();
    super.dispose();
  }

  void _onChanged(String t) {
    final lvl = _detector.scan(t);
    if (lvl != _crisis) setState(() => _crisis = lvl);
    if (lvl == CrisisLevel.mid || lvl == CrisisLevel.severe) {
      // Soft reminder; default action = continue (non-blocking).
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) CrisisLevel2Dialog.show(context);
      });
    }
  }

  Future<void> _diagnose() async {
    if (_input.text.trim().isEmpty) return;
    setState(() {
      _busy = true;
      _error = null;
    });
    final client = ref.read(rustCoreClientProvider);
    try {
      // 1. record the statement as a fact;
      await client.createFact(
        widget.caseId,
        CreateFactReq(
          category: FactCategory.other,
          statement: _input.text.trim(),
          source: FactSource.userInput,
        ),
      );
      // 2. ask the dispatcher.
      final resp = await client.llmQuery(
        LlmQueryReq(caseId: widget.caseId, prompt: _input.text.trim()),
      );
      setState(() => _resp = resp);
    } on RustCoreApiException catch (e) {
      setState(() => _error = '${e.code}: ${e.message}');
    } catch (e) {
      setState(() => _error = '$e');
    } finally {
      if (mounted) setState(() => _busy = false);
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
      body: Column(
        children: [
          if (_crisis == CrisisLevel.light)
            CrisisLevel1Banner(onDismiss: () => setState(() => _crisis = CrisisLevel.none)),
          Expanded(
            child: ListView(
              padding: const EdgeInsets.all(16),
              children: [
                TextField(
                  controller: _input,
                  maxLines: 4,
                  onChanged: _onChanged,
                  decoration: const InputDecoration(
                    labelText: '请描述您遇到的情况',
                    border: OutlineInputBorder(),
                  ),
                ),
                const SizedBox(height: 12),
                FilledButton.icon(
                  onPressed: _busy ? null : _diagnose,
                  icon: _busy
                      ? const SizedBox(width: 16, height: 16, child: CircularProgressIndicator(strokeWidth: 2))
                      : const Icon(StuchkaIcons.diagnose),
                  label: const Text('提交诊断'),
                ),
                if (_error != null) ...[
                  const SizedBox(height: 12),
                  Text(_error!, style: TextStyle(color: colors.riskHigh)),
                ],
                if (_resp != null) ...[
                  const SizedBox(height: 16),
                  _DiagnoseResult(resp: _resp!),
                ],
              ],
            ),
          ),
        ],
      ),
    );
  }
}

class _DiagnoseResult extends StatelessWidget {
  const _DiagnoseResult({required this.resp});
  final LlmQueryResp resp;

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
      ),
    );
  }
}
