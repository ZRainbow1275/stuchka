import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../ipc/dto/dtos.dart';
import '../../ipc/rust_core_providers.dart';

/// `GET /case` — the active case list (<= 3 in the standard home; used by simple-mode "继续案件").
final caseListProvider = FutureProvider<List<CaseDto>>((ref) async {
  final client = ref.watch(rustCoreClientProvider);
  return client.listCases();
});

/// `GET /case/:id` aggregate view for the workbench.
final caseAggregateProvider =
    FutureProvider.family<CaseAggregateDto, String>((ref, id) async {
  final client = ref.watch(rustCoreClientProvider);
  return client.getCase(id);
});

/// `GET /kb/version` for the status bar KbVersionChip (>30 days = Level4).
final kbVersionProvider = FutureProvider<KbVersionDto>((ref) async {
  final client = ref.watch(rustCoreClientProvider);
  return client.kbVersion();
});

/// The current "active" case (the most-recently-updated case, used by the simple home).
final activeCaseProvider = Provider<CaseDto?>((ref) {
  final list = ref.watch(caseListProvider).value;
  if (list == null || list.isEmpty) return null;
  final sorted = [...list]
    ..sort((a, b) => (b.updatedAt ?? DateTime(1970)).compareTo(a.updatedAt ?? DateTime(1970)));
  return sorted.first;
});
