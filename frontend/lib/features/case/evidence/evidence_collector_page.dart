import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../ipc/dto/dtos.dart';
import '../../../ipc/dto/enums.dart';
import '../../../ipc/rust_core_client.dart';
import '../../../ipc/rust_core_providers.dart';
import '../../../theme/stuchka_icons.dart';
import '../../../theme/stuchka_theme.dart';
import '../case_providers.dart';
import 'effective_score_widget.dart';
import 'evidence_cards.dart';

/// /case/:id/evidence — the seven-class evidence collector. A card per category + the
/// EffectiveScore four-band readout after upload (`POST /case/:id/evidence` multipart).
class EvidenceCollectorPage extends ConsumerStatefulWidget {
  const EvidenceCollectorPage({super.key, required this.caseId});
  final String caseId;

  @override
  ConsumerState<EvidenceCollectorPage> createState() => _EvidenceCollectorPageState();
}

class _EvidenceCollectorPageState extends ConsumerState<EvidenceCollectorPage> {
  EvidenceCategory _selected = EvidenceCategory.documentaryContract;
  Map<String, dynamic> _metadata = {};
  bool _busy = false;
  String? _error;
  EvidenceDto? _result;

  EvidenceCardSpec get _spec =>
      EvidenceCardSpec.all.firstWhere((s) => s.category == _selected);

  Future<void> _upload() async {
    setState(() {
      _busy = true;
      _error = null;
    });
    final client = ref.read(rustCoreClientProvider);
    try {
      // R1a desktop: a placeholder "import" file (real picker / camera is R1b). The metadata text
      // is what the HSD scan + scoring runs on.
      final stub = utf8.encode(jsonEncode({'note': '导入占位文件', ..._metadata}));
      final ev = await client.uploadEvidence(
        widget.caseId,
        fileBytes: stub,
        fileName: 'evidence-${_selected.wire}.json',
        metadata: _metadata,
      );
      setState(() => _result = ev);
      ref.invalidate(caseAggregateProvider(widget.caseId));
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
    return Scaffold(
      appBar: AppBar(title: const Text('证据收集 · 七大类')),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          Wrap(
            spacing: 8,
            runSpacing: 8,
            children: EvidenceCategory.values.map((c) {
              return ChoiceChip(
                selected: _selected == c,
                label: Text(c.labelZh),
                onSelected: (_) => setState(() {
                  _selected = c;
                  _metadata = {};
                  _result = null;
                }),
              );
            }).toList(),
          ),
          const SizedBox(height: 12),
          EvidenceCardForm(
            key: ValueKey(_selected),
            spec: _spec,
            onChanged: (m) => _metadata = m,
          ),
          const SizedBox(height: 12),
          FilledButton.icon(
            onPressed: _busy ? null : _upload,
            icon: _busy
                ? const SizedBox(width: 16, height: 16, child: CircularProgressIndicator(strokeWidth: 2))
                : const Icon(StuchkaIcons.add),
            label: const Text('上传并评分'),
          ),
          if (_error != null) ...[
            const SizedBox(height: 12),
            Text(
              _error!,
              style: TextStyle(color: StuchkaSemanticColors.of(context).riskHigh),
            ),
          ],
          if (_result != null) ...[
            const SizedBox(height: 16),
            Card(
              child: Padding(
                padding: const EdgeInsets.all(12),
                child: EffectiveScoreWidget(
                  score: _result!.effectiveScore,
                  breakdown: _result!.scoreBreakdown,
                ),
              ),
            ),
          ],
        ],
      ),
    );
  }
}
