import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:intl/intl.dart';

import '../../ipc/dto/dtos.dart';
import '../../ipc/rust_core_client.dart';
import '../../ipc/rust_core_providers.dart';
import '../../theme/stuchka_icons.dart';
import '../../theme/stuchka_theme.dart';
import '../case/case_providers.dart';

/// M6 知识库管理 (R1.5; prd/02 §2.1). Shows the live KB version (hash / label / generated time /
/// whole-day age) from `GET /kb/version` and lets the user re-validate it via `POST /kb/refresh`,
/// surfacing the REAL freshness level (fresh / stale / expired). The Level-4 (>= 30 天) expired
/// state is the INV-04 staleness gate that blocks compensation calculation — it is shown
/// prominently, never hidden. The actual network pull (GitHub Pages -> jsDelivr -> mirror) is the
/// honest R1b seam: refresh re-validates locally and reports `changed: false` truthfully.
class KbManagementPage extends ConsumerStatefulWidget {
  const KbManagementPage({super.key});

  @override
  ConsumerState<KbManagementPage> createState() => _KbManagementPageState();
}

class _KbManagementPageState extends ConsumerState<KbManagementPage> {
  bool _busy = false;
  String? _error;
  KbRefreshDto? _refresh;

  Future<void> _doRefresh() async {
    setState(() {
      _busy = true;
      _error = null;
    });
    try {
      final res = await ref.read(rustCoreClientProvider).kbRefresh();
      setState(() => _refresh = res);
      ref.invalidate(kbVersionProvider);
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
    final version = ref.watch(kbVersionProvider);
    return Scaffold(
      appBar: AppBar(title: const Text('知识库管理')),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          version.when(
            loading: () => const Center(
                child: Padding(padding: EdgeInsets.all(24), child: CircularProgressIndicator())),
            error: (e, _) => _card(
              context,
              icon: StuchkaIcons.alert,
              color: colors.sealRed,
              title: '知识库版本加载失败',
              body: '$e',
            ),
            data: (v) => _versionCard(context, colors, v),
          ),
          const SizedBox(height: 12),
          FilledButton.icon(
            onPressed: _busy ? null : _doRefresh,
            icon: _busy
                ? const SizedBox(width: 16, height: 16, child: CircularProgressIndicator(strokeWidth: 2))
                : const Icon(StuchkaIcons.refresh),
            label: const Text('检查并校验知识库'),
          ),
          const SizedBox(height: 8),
          Text(
            '联网增量同步（GitHub Pages → jsDelivr → 镜像）为后续版本（R1b）能力；当前校验为本地清单'
            '完整性复核，不联网拉取。',
            style: Theme.of(context).textTheme.bodySmall?.copyWith(color: colors.inkBlue),
          ),
          if (_error != null) ...[
            const SizedBox(height: 12),
            Text(_error!, style: TextStyle(color: colors.riskHigh)),
          ],
          if (_refresh != null) ...[
            const SizedBox(height: 16),
            _refreshResultCard(context, colors, _refresh!),
          ],
        ],
      ),
    );
  }

  Widget _versionCard(BuildContext context, StuchkaSemanticColors colors, KbVersionDto v) {
    final age = v.ageDays;
    final (stateColor, stateLabel) = _freshnessOfAge(colors, age);
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Icon(StuchkaIcons.sourceKb, color: colors.inkBlue, size: 20),
                const SizedBox(width: 8),
                Text('当前知识库', style: Theme.of(context).textTheme.titleMedium),
                const Spacer(),
                _stateChip(stateColor, stateLabel),
              ],
            ),
            const SizedBox(height: 8),
            _row(context, '版本', v.versionLabel.isEmpty ? '（未命名）' : v.versionLabel),
            _row(context, '内容哈希', v.versionHash, mono: true),
            if (v.updatedAt != null)
              _row(context, '生成时间', DateFormat('yyyy-MM-dd HH:mm').format(v.updatedAt!.toLocal())),
            _row(context, '已使用天数', '$age 天'),
            if (age >= 30) ...[
              const SizedBox(height: 8),
              Text(
                '知识库已超过 30 天（Level4）：赔偿计算已停用（INV-04 时效门），时效与流程提示仍可使用。'
                '请更新知识库后再进行金额计算。',
                style: Theme.of(context).textTheme.bodySmall?.copyWith(color: colors.sealRed),
              ),
            ],
          ],
        ),
      ),
    );
  }

  Widget _refreshResultCard(BuildContext context, StuchkaSemanticColors colors, KbRefreshDto r) {
    final (stateColor, stateLabel) = switch (r.freshness) {
      'expired' => (colors.sealRed, '已过期（Level4）'),
      'stale' => (colors.riskMid, '偏旧'),
      _ => (colors.riskLow, '新鲜'),
    };
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Icon(StuchkaIcons.check, color: stateColor, size: 20),
                const SizedBox(width: 8),
                Text('校验结果', style: Theme.of(context).textTheme.titleMedium),
                const Spacer(),
                _stateChip(stateColor, stateLabel),
              ],
            ),
            const SizedBox(height: 8),
            _row(context, '清单完整性', '已通过本地校验（KBC-02/03）'),
            _row(context, '已使用天数', '${r.ageDays} 天'),
            _row(context, '是否更新到新版本', r.changed ? '是' : '否（本地校验，未联网拉取）'),
          ],
        ),
      ),
    );
  }

  (Color, String) _freshnessOfAge(StuchkaSemanticColors colors, int age) {
    if (age >= 30) return (colors.sealRed, '已过期（Level4）');
    if (age >= 14) return (colors.riskMid, '偏旧');
    return (colors.riskLow, '新鲜');
  }

  Widget _stateChip(Color color, String label) => Container(
        padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 4),
        decoration: BoxDecoration(
          color: color.withValues(alpha: 0.12),
          borderRadius: BorderRadius.circular(12),
        ),
        child: Text(label, style: TextStyle(color: color, fontWeight: FontWeight.w600)),
      );

  Widget _row(BuildContext context, String label, String value, {bool mono = false}) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 3),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SizedBox(width: 110, child: Text(label, style: Theme.of(context).textTheme.bodySmall)),
          Expanded(
            child: Text(
              value,
              style: mono
                  ? const TextStyle(fontFamily: 'JetBrainsMonoSC', fontSize: 11)
                  : Theme.of(context).textTheme.bodyMedium,
            ),
          ),
        ],
      ),
    );
  }

  Widget _card(
    BuildContext context, {
    required IconData icon,
    required Color color,
    required String title,
    required String body,
  }) {
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Icon(icon, color: color, size: 20),
                const SizedBox(width: 8),
                Expanded(child: Text(title, style: Theme.of(context).textTheme.titleMedium)),
              ],
            ),
            const SizedBox(height: 8),
            Text(body),
          ],
        ),
      ),
    );
  }
}
