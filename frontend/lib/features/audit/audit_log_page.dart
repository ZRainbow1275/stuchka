import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../ipc/dto/dtos.dart';
import '../../ipc/rust_core_providers.dart';
import '../../theme/stuchka_icons.dart';
import '../../theme/stuchka_theme.dart';

final _auditProvider = FutureProvider.family<List<AuditEntryDto>, String>((ref, caseId) async {
  final client = ref.watch(rustCoreClientProvider);
  return client.queryAudit(caseId: caseId, limit: 100);
});

/// /case/:id/audit — the decrypted audit-chain projection (`GET /audit?case_id=&cursor=&limit=`).
class AuditLogPage extends ConsumerWidget {
  const AuditLogPage({super.key, required this.caseId});
  final String caseId;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final entries = ref.watch(_auditProvider(caseId));
    final colors = StuchkaSemanticColors.of(context);
    return Scaffold(
      appBar: AppBar(
        title: const Text('审计日志'),
        actions: [
          IconButton(
            icon: const Icon(StuchkaIcons.refresh),
            onPressed: () => ref.invalidate(_auditProvider(caseId)),
          ),
        ],
      ),
      body: entries.when(
        loading: () => const Center(child: CircularProgressIndicator()),
        error: (e, _) => Center(child: Text('查询失败：$e')),
        data: (list) {
          if (list.isEmpty) return const Center(child: Text('暂无审计记录'));
          return ListView.separated(
            padding: const EdgeInsets.all(12),
            itemCount: list.length,
            separatorBuilder: (context, index) => const Divider(height: 1),
            itemBuilder: (context, i) {
              final e = list[i];
              return ListTile(
                leading: CircleAvatar(
                  backgroundColor: colors.inkBlue,
                  foregroundColor: colors.paper,
                  child: Text('${e.seq}', style: const TextStyle(fontSize: 12)),
                ),
                title: Text('${e.why} · ${e.category.wire}'),
                subtitle: Text('${e.who} · ${e.when ?? '-'}\n${e.what}'),
                isThreeLine: true,
                trailing: Icon(StuchkaIcons.lock, size: 14, color: colors.ash),
              );
            },
          );
        },
      ),
    );
  }
}
