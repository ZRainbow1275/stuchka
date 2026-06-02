import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../ipc/dto/enums.dart';
import '../../../theme/stuchka_icons.dart';
import '../../../theme/stuchka_theme.dart';
import '../case_providers.dart';

/// One recommended procedure column.
class ProcedureColumn {
  ProcedureColumn({
    required this.role,
    required this.title,
    required this.steps,
    this.coverageTag = CoverageTag.exact,
  });
  final String role; // main / parallel / fallback
  final String title;
  final List<String> steps;
  final CoverageTag coverageTag;
}

/// /case/:id/procedure-compare — three-way side-by-side (main / parallel / fallback). A column whose
/// coverage_tag == unknown degrades the WHOLE column to "转法援 + 法条引用" (spec 04 §4.8 / brief §3.7).
class ProcedureComparePage extends ConsumerWidget {
  const ProcedureComparePage({super.key, required this.caseId});
  final String caseId;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final agg = ref.watch(caseAggregateProvider(caseId));
    return Scaffold(
      appBar: AppBar(title: const Text('程序路径对比 · 三路并列')),
      body: agg.when(
        loading: () => const Center(child: CircularProgressIndicator()),
        error: (e, _) => Center(child: Text('加载失败：$e')),
        data: (_) {
          // R1a placeholder columns; the real recommended_procedures come from GET /case/:id
          // (later subtask wires the backend field).
          final columns = <ProcedureColumn>[
            ProcedureColumn(
              role: 'main',
              title: '主路径 · 劳动仲裁',
              steps: ['提交仲裁申请', '受理与排期', '开庭审理', '仲裁裁决'],
            ),
            ProcedureColumn(
              role: 'parallel',
              title: '并行 · 劳动监察投诉',
              steps: ['向劳动监察大队投诉', '责令限期改正', '行政处理'],
              coverageTag: CoverageTag.approximate,
            ),
            ProcedureColumn(
              role: 'fallback',
              title: '兜底 · 协商调解',
              steps: ['申请调解', '达成调解协议'],
              coverageTag: CoverageTag.unknown,
            ),
          ];
          return Padding(
            padding: const EdgeInsets.all(16),
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: columns
                  .map((c) => Expanded(child: Padding(
                        padding: const EdgeInsets.symmetric(horizontal: 6),
                        child: _ProcedureCard(column: c),
                      )))
                  .toList(),
            ),
          );
        },
      ),
    );
  }
}

class _ProcedureCard extends StatelessWidget {
  const _ProcedureCard({required this.column});
  final ProcedureColumn column;

  @override
  Widget build(BuildContext context) {
    final colors = StuchkaSemanticColors.of(context);
    final degraded = column.coverageTag == CoverageTag.unknown;
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(12),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(column.title, style: Theme.of(context).textTheme.titleMedium),
            const SizedBox(height: 8),
            if (degraded)
              _LegalAidFallback(colors: colors)
            else
              ...column.steps.asMap().entries.map((e) => Padding(
                    padding: const EdgeInsets.symmetric(vertical: 3),
                    child: Row(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text('${e.key + 1}.', style: TextStyle(color: colors.inkBlue)),
                        const SizedBox(width: 6),
                        Expanded(child: Text(e.value)),
                      ],
                    ),
                  )),
          ],
        ),
      ),
    );
  }
}

class _LegalAidFallback extends StatelessWidget {
  const _LegalAidFallback({required this.colors});
  final StuchkaSemanticColors colors;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(10),
      decoration: BoxDecoration(
        color: colors.sealRed.withValues(alpha: 0.08),
        borderRadius: BorderRadius.circular(6),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Icon(StuchkaIcons.alert, size: 16, color: colors.sealRed),
              const SizedBox(width: 6),
              Text('覆盖度未知 · 转法律援助',
                  style: TextStyle(color: colors.sealRed, fontWeight: FontWeight.w700)),
            ],
          ),
          const SizedBox(height: 6),
          const Text('该程序无足够法源支撑，建议咨询法律援助（12348）并提供法条引用。'),
        ],
      ),
    );
  }
}
