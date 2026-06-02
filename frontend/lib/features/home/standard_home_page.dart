import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../../theme/stuchka_icons.dart';
import '../../theme/stuchka_theme.dart';
import '../accessibility/simple_mode_controller.dart';
import '../accessibility/text_scaler_controller.dart';
import '../accessibility/theme_mode_controller.dart';
import '../case/case_providers.dart';

/// Standard home: case list (<= 3 active) + the accessibility toggles (text scale / theme /
/// simple mode entry) + the "开始诊断" entry.
class StandardHomePage extends ConsumerWidget {
  const StandardHomePage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final cases = ref.watch(caseListProvider);
    final colors = StuchkaSemanticColors.of(context);

    return Scaffold(
      appBar: AppBar(
        title: const Text('Stučka · 劳动纠纷工作台'),
        actions: [
          IconButton(
            tooltip: '农民工欠薪向导',
            icon: const Icon(StuchkaIcons.procedure),
            onPressed: () => context.go('/scenario/migrant-wage/identity'),
          ),
          IconButton(
            tooltip: '知识库管理',
            icon: const Icon(StuchkaIcons.sourceKb),
            onPressed: () => context.go('/kb'),
          ),
          IconButton(
            tooltip: '字号 +',
            icon: const Icon(StuchkaIcons.zoomIn),
            onPressed: () =>
                ref.read(textScalerProvider.notifier).setUserOverride(1.20),
          ),
          IconButton(
            tooltip: '深浅切换',
            icon: const Icon(StuchkaIcons.moon),
            onPressed: () {
              final cur = ref.read(themeModeProvider);
              ref.read(themeModeProvider.notifier).setMode(
                    cur == ThemeMode.dark ? ThemeMode.light : ThemeMode.dark,
                  );
            },
          ),
          IconButton(
            tooltip: '极简模式',
            icon: const Icon(StuchkaIcons.help),
            onPressed: () => ref.read(simpleModeProvider.notifier).enable(),
          ),
        ],
      ),
      floatingActionButton: FloatingActionButton.extended(
        backgroundColor: colors.sealRed,
        foregroundColor: colors.paper,
        icon: const Icon(StuchkaIcons.diagnose),
        label: const Text('开始诊断'),
        onPressed: () => context.go('/intake/identity'),
      ),
      body: cases.when(
        loading: () => const Center(child: CircularProgressIndicator()),
        error: (e, _) => _ErrorPanel(message: '$e', onRetry: () => ref.invalidate(caseListProvider)),
        data: (list) {
          if (list.isEmpty) {
            return Center(
              child: Column(
                mainAxisSize: MainAxisSize.min,
                children: [
                  const Text('尚无案件 · 点击右下角开始诊断'),
                  const SizedBox(height: 12),
                  OutlinedButton.icon(
                    icon: const Icon(StuchkaIcons.procedure, size: 16),
                    label: const Text('农民工欠薪向导（主用户路径）'),
                    onPressed: () => context.go('/scenario/migrant-wage/identity'),
                  ),
                ],
              ),
            );
          }
          return ListView.separated(
            padding: const EdgeInsets.all(16),
            itemCount: list.length,
            separatorBuilder: (context, index) => const Divider(height: 1),
            itemBuilder: (context, i) {
              final c = list[i];
              return ListTile(
                leading: Icon(StuchkaIcons.caseRoot, color: colors.inkBlue),
                title: Text(c.shortLabel),
                subtitle: Text('状态：${c.status.wire} · 创建于 ${c.createdAt ?? '-'}'),
                trailing: const Icon(StuchkaIcons.chevronRight),
                onTap: () => context.go('/case/${c.id}'),
              );
            },
          );
        },
      ),
    );
  }
}

class _ErrorPanel extends StatelessWidget {
  const _ErrorPanel({required this.message, required this.onRetry});
  final String message;
  final VoidCallback onRetry;

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          const Icon(StuchkaIcons.alert),
          const SizedBox(height: 8),
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 32),
            child: Text(message, textAlign: TextAlign.center),
          ),
          const SizedBox(height: 12),
          OutlinedButton.icon(
            icon: const Icon(StuchkaIcons.refresh),
            label: const Text('重试'),
            onPressed: onRetry,
          ),
        ],
      ),
    );
  }
}
