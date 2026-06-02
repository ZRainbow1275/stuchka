import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../theme/stuchka_icons.dart';
import '../../../theme/stuchka_theme.dart';
import '../hotlines.dart';
import 'cooldown_providers.dart';

/// The verbatim INV-07 Level-3 (severe) body copy (compliance/05 §4.2 Level-3). Language-neutral:
/// NO "诊断 / 判定" (compliance/05 §4 / §9.2 — `regex_count(诊断|判定) == 0`). Hard-coded, not editable.
const String kCrisisLevel3Body =
    '我们检测到您可能正处于难以承受的状态，您的安全是最重要的。\n\n'
    '请考虑现在做以下一件事：\n'
    '- 拨打全国心理援助热线 12320（24 小时）；\n'
    '- 拨打全国心理援助热线 010-82951332（北京）或 021-12320（上海）'
    '或您所在地区的专业危机干预电话；\n'
    '- 联系您信任的家人或朋友陪伴您。\n\n'
    '为保护您的状态，本系统接下来 24 小时内将暂停以下操作：\n'
    '- 任何“导出文书”、“放弃请求项”、“签和解”、“主动离职” 类高风险按钮；\n'
    '- 本暂停仅作用于您当前账户，不影响您所在群体案件的其他成员。\n\n'
    '如果您认为本次暂停是误判，您可以随时通过下方的 “申诉误判” 按钮申请解除。';

/// INV-07 Level-3 strong-reminder dialog + 24h cooldown (compliance/05 §4.2 / §5.2 / spec 05 §5.6.4).
///
/// On accept (我已拨打热线) the dialog begins a 24h cooldown on [kHighRiskFeature] for the local
/// user, scoped to this account only (C-B-9: never the rest of a group). The 误判申诉 entry is
/// reachable in-dialog (R1 必死, compliance/05 §5.3). Returns true iff accepted.
class CrisisLevel3Dialog {
  static Future<bool> show(BuildContext ctx, WidgetRef ref) async {
    final accept = await showDialog<bool>(
      context: ctx,
      barrierDismissible: false,
      builder: (dctx) => _Level3DialogBody(rootRef: ref),
    );
    if (accept == true) {
      // Begin the account-scoped 24h cooldown (compliance/05 §5.2). The cooldown notifier is the
      // real lock consumed by HighRiskGate.
      ref.read(cooldownProvider(kHighRiskFeature).notifier).begin24h();
    }
    return accept ?? false;
  }
}

class _Level3DialogBody extends StatelessWidget {
  const _Level3DialogBody({required this.rootRef});
  final WidgetRef rootRef;

  @override
  Widget build(BuildContext context) {
    final colors = StuchkaSemanticColors.of(context);
    return AlertDialog(
      title: Row(
        children: [
          Icon(StuchkaIcons.heart, color: colors.sealRed),
          const SizedBox(width: 8),
          const Expanded(child: Text('您的安全是最重要的')),
        ],
      ),
      content: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 520),
        child: SingleChildScrollView(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              const Text(kCrisisLevel3Body),
              const SizedBox(height: 12),
              // Bottom hotline disclaimer (compliance/05 §4.5 安全 6 — required on every Level-3 dialog).
              Text(
                '本系统不是心理医疗机构，无法替代专业心理援助。\n$kHotlineDisclaimer',
                style: Theme.of(context).textTheme.bodySmall,
              ),
            ],
          ),
        ),
      ),
      actions: [
        // 误判申诉 入口 — R1 必死 (compliance/05 §5.3). Submits a local appeal and releases immediately.
        TextButton(
          onPressed: () {
            rootRef.read(appealProvider.notifier).submit(reason: 'misdetection');
            Navigator.pop(context, false);
          },
          child: const Text('申诉误判'),
        ),
        FilledButton(
          autofocus: true,
          onPressed: () => Navigator.pop(context, true),
          child: const Text('我已拨打热线'),
        ),
      ],
    );
  }
}

/// 24h-cooldown banner shown when entering an INV-10 high-risk surface while a Level-3 cooldown is
/// active (compliance/05 §5.2 / spec 05 §5.6.4). Renders nothing when the cooldown is inactive.
/// The misdetection appeal here releases the cooldown locally with NO project approval
/// (compliance/05 §5.3 "用户即审计员，本机立即解除").
class Level3CooldownGate extends ConsumerWidget {
  const Level3CooldownGate({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final cooldown = ref.watch(cooldownProvider(kHighRiskFeature));
    if (!cooldown.active) return const SizedBox.shrink();
    return CooldownAppealCard(
      remaining: cooldown.remaining,
      onAppeal: () => ref.read(appealProvider.notifier).submit(reason: 'misdetection'),
    );
  }
}

/// The card body shared by [Level3CooldownGate] and the in-gate Level-3 lock (high_risk_gate.dart):
/// remaining time + the 误判申诉 button (< 3 steps reachable, compliance/05 §9.2).
class CooldownAppealCard extends StatelessWidget {
  const CooldownAppealCard({
    super.key,
    required this.remaining,
    required this.onAppeal,
  });

  final Duration remaining;
  final VoidCallback onAppeal;

  @override
  Widget build(BuildContext context) {
    final colors = StuchkaSemanticColors.of(context);
    final h = remaining.inHours;
    final m = remaining.inMinutes % 60;
    return Container(
      width: double.infinity,
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: colors.sealRed.withValues(alpha: 0.08),
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: colors.sealRed, width: 0.8),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Icon(StuchkaIcons.freeze, size: 16, color: colors.sealRed),
              const SizedBox(width: 6),
              Expanded(
                child: Text(
                  '高风险操作冷静期生效中 · 剩余约 $h 小时 $m 分钟',
                  style: TextStyle(color: colors.sealRed, fontWeight: FontWeight.w700),
                ),
              ),
            ],
          ),
          const SizedBox(height: 6),
          const Text('为保护您的状态，导出文书 / 放弃请求项 / 签和解 / 主动离职 等高风险按钮已暂停 24 小时，'
              '仅作用于您本人，不影响群体案件其他成员。'),
          const SizedBox(height: 8),
          Align(
            alignment: Alignment.centerRight,
            child: OutlinedButton.icon(
              icon: Icon(StuchkaIcons.info, size: 16, color: colors.sealRed),
              label: const Text('申诉误判（本机立即解除）'),
              onPressed: onAppeal,
            ),
          ),
        ],
      ),
    );
  }
}
