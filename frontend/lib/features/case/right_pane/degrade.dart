import 'package:flutter/material.dart';

import '../../../theme/stuchka_icons.dart';
import '../../../theme/stuchka_theme.dart';

/// AI dispatcher degrade levels 0-4 (ai/01 §4.5.3 / brief §5).
enum DegradeLevel {
  level0, // 全联网
  level1, // 主云挂，切备用
  level2, // 切本地 Qwen
  level3, // 纯规则 M5/M9/M16
  level4, // KB > 30 天，拒绝赔偿计算
}

extension DegradeLevelInfo on DegradeLevel {
  int get index0to4 => index;

  String get labelZh => 'Level $index';

  String get reasonZh => switch (this) {
        DegradeLevel.level0 => '全部联网通道可用',
        DegradeLevel.level1 => '主云不可用，已切换至备用云',
        DegradeLevel.level2 => '云端不可用，已切换至本地模型',
        DegradeLevel.level3 => '本地模型不可用，仅规则引擎可计算',
        DegradeLevel.level4 => '知识库已超过 30 天，赔偿计算已停用',
      };

  String get recoveryHintZh => switch (this) {
        DegradeLevel.level0 => '无需操作',
        DegradeLevel.level1 => '稍后将自动重试主云',
        DegradeLevel.level2 => '恢复网络后将自动回切云端',
        DegradeLevel.level3 => '下载本地模型或恢复网络以启用 AI',
        DegradeLevel.level4 => '请先更新知识库（GET /kb/version）',
      };

  /// Level4 → compensation engine abstains; the UI must NOT fall back to AI inference.
  bool get blocksCompensation => this == DegradeLevel.level4;

  static DegradeLevel fromIndex(int i) =>
      DegradeLevel.values[i.clamp(0, DegradeLevel.values.length - 1)];
}

/// One degrade transition event pushed over `WS /ws/status`.
class DegradeEvent {
  DegradeEvent({
    required this.ts,
    required this.from,
    required this.to,
    required this.reasonCode,
    required this.reasonTextZh,
    this.affectedModules = const [],
    required this.recoveryHint,
  });

  final DateTime ts;
  final DegradeLevel from;
  final DegradeLevel to;
  final String reasonCode;
  final String reasonTextZh;
  final List<String> affectedModules;
  final String recoveryHint;
}

/// Permanent degrade banner: `Level X · 原因 · 影响 N 模块 · 如何恢复` (spec / brief §5). Any
/// degrade must be可见可解释可恢复.
class AiDegradeBanner extends StatelessWidget {
  const AiDegradeBanner({super.key, required this.level, this.affectedModuleCount = 0});

  final DegradeLevel level;
  final int affectedModuleCount;

  @override
  Widget build(BuildContext context) {
    final colors = StuchkaSemanticColors.of(context);
    final color = switch (level) {
      DegradeLevel.level0 => colors.riskLow,
      DegradeLevel.level1 || DegradeLevel.level2 => colors.riskMid,
      DegradeLevel.level3 => colors.riskMid,
      DegradeLevel.level4 => colors.sealRed,
    };
    return Container(
      width: double.infinity,
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
      color: color.withValues(alpha: 0.12),
      child: Row(
        children: [
          Icon(level.blocksCompensation ? StuchkaIcons.alert : StuchkaIcons.info,
              size: 16, color: color),
          const SizedBox(width: 8),
          Expanded(
            child: Text(
              '${level.labelZh} · ${level.reasonZh} · 影响 $affectedModuleCount 模块 · ${level.recoveryHintZh}',
              style: Theme.of(context).textTheme.bodySmall?.copyWith(color: colors.char),
            ),
          ),
        ],
      ),
    );
  }
}

/// Compact degrade indicator for the status bar.
class AiDegradeIndicator extends StatelessWidget {
  const AiDegradeIndicator({super.key, required this.level});
  final DegradeLevel level;

  @override
  Widget build(BuildContext context) {
    final colors = StuchkaSemanticColors.of(context);
    final color = level.blocksCompensation ? colors.sealRed : colors.inkBlue;
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Icon(StuchkaIcons.info, size: 12, color: color),
        const SizedBox(width: 4),
        Text(level.labelZh, style: TextStyle(fontSize: 12, color: color)),
      ],
    );
  }
}
