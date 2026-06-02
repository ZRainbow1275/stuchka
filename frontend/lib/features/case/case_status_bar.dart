import 'package:flutter/material.dart';

import '../../ipc/dto/dtos.dart';
import '../../ipc/dto/enums.dart';
import '../../theme/stuchka_icons.dart';
import '../../theme/stuchka_theme.dart';
import 'right_pane/degrade.dart';

/// Case workbench status bar (spec 04 §4.10 / brief §3.7). Height 28dp. Shows the diagnose dot,
/// evidence/gap counts, chain closure %, sync indicator, AI degrade indicator, and the KB version
/// chip (> 30 days → 印章红 strong warning = Level4).
class StuchkaCaseStatusBar extends StatelessWidget {
  const StuchkaCaseStatusBar({
    super.key,
    required this.aggregate,
    this.kbVersion,
    this.degradeLevel = DegradeLevel.level0,
  });

  final CaseAggregateDto aggregate;
  final KbVersionDto? kbVersion;
  final DegradeLevel degradeLevel;

  @override
  Widget build(BuildContext context) {
    final colors = StuchkaSemanticColors.of(context);
    final evidenceCount = aggregate.evidences.length;
    final factCount = aggregate.facts.length;
    final confirmed = aggregate.facts.where((f) => f.status == FactStatus.confirmed).length;
    final closurePct = factCount == 0 ? 0 : (confirmed * 100 ~/ factCount);

    return Container(
      height: 28,
      color: colors.rice,
      padding: const EdgeInsets.symmetric(horizontal: 12),
      child: Row(
        children: [
          _Dot(color: aggregate.caseDto.status == CaseStatus.frozen ? colors.frozen : colors.riskLow),
          const SizedBox(width: 6),
          Text('诊断：${aggregate.caseDto.status.wire}', style: const TextStyle(fontSize: 12)),
          const _Sep(),
          Text('证据 $evidenceCount / 缺口 ${factCount - evidenceCount < 0 ? 0 : factCount - evidenceCount}',
              style: const TextStyle(fontSize: 12)),
          const _Sep(),
          Text('链闭合 $closurePct%', style: const TextStyle(fontSize: 12)),
          const _Sep(),
          Icon(StuchkaIcons.refresh, size: 12, color: colors.ash),
          const _Sep(),
          AiDegradeIndicator(level: degradeLevel),
          const Spacer(),
          _KbVersionChip(kb: kbVersion),
        ],
      ),
    );
  }
}

class _KbVersionChip extends StatelessWidget {
  const _KbVersionChip({required this.kb});
  final KbVersionDto? kb;

  @override
  Widget build(BuildContext context) {
    final colors = StuchkaSemanticColors.of(context);
    if (kb == null) {
      return Text('KB —', style: TextStyle(fontSize: 12, color: colors.ash));
    }
    final days = kb!.ageDays;
    final expired = days >= 30; // > 30 天 → 印章红 = Level4
    final color = expired ? colors.sealRed : colors.inkBlue;
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Icon(expired ? StuchkaIcons.alert : StuchkaIcons.sourceKb, size: 12, color: color),
        const SizedBox(width: 4),
        Text(
          'KB ${kb!.versionLabel} · ${days}d${expired ? ' · 已过期' : ''}',
          style: TextStyle(fontSize: 12, color: color, fontWeight: expired ? FontWeight.w700 : FontWeight.w400),
        ),
      ],
    );
  }
}

class _Dot extends StatelessWidget {
  const _Dot({required this.color});
  final Color color;
  @override
  Widget build(BuildContext context) =>
      Container(width: 8, height: 8, decoration: BoxDecoration(color: color, shape: BoxShape.circle));
}

class _Sep extends StatelessWidget {
  const _Sep();
  @override
  Widget build(BuildContext context) => const Padding(
        padding: EdgeInsets.symmetric(horizontal: 8),
        child: Text('·', style: TextStyle(fontSize: 12)),
      );
}
