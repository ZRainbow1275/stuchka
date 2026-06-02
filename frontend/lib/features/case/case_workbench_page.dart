import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:multi_split_view/multi_split_view.dart';

import '../../app/prefs.dart';
import '../../theme/stuchka_icons.dart';
import 'case_providers.dart';
import 'case_status_bar.dart';
import 'center_pane/fact_card.dart';
import 'left_pane/document_tree.dart';
import 'right_pane/ai_assistant_panel.dart';

/// Three-column workbench (spec 04 §4.2 / brief §3). editor|evidence|AI columns with persisted
/// split ratios and F2/F3/F4 focus switching.
class CaseWorkbenchPage extends ConsumerStatefulWidget {
  const CaseWorkbenchPage({super.key, required this.caseId});
  final String caseId;

  @override
  ConsumerState<CaseWorkbenchPage> createState() => _CaseWorkbenchPageState();
}

class _CaseWorkbenchPageState extends ConsumerState<CaseWorkbenchPage> {
  late final MultiSplitViewController _split;
  final _leftFocus = FocusNode(debugLabel: 'pane-left');
  final _centerFocus = FocusNode(debugLabel: 'pane-center');
  final _rightFocus = FocusNode(debugLabel: 'pane-right');

  @override
  void initState() {
    super.initState();
    final saved = ref.read(prefsProvider).readDoubleList(StuchkaPrefs.kCasePaneRatio);
    final flexes = (saved != null && saved.length == 3) ? saved : const [0.30, 0.38, 0.32];
    _split = MultiSplitViewController(
      areas: [
        Area(flex: flexes[0], min: 0.18, data: 'left'),
        Area(flex: flexes[1], min: 0.25, data: 'center'),
        Area(flex: flexes[2], min: 0.22, data: 'right'),
      ],
    );
  }

  @override
  void dispose() {
    _leftFocus.dispose();
    _centerFocus.dispose();
    _rightFocus.dispose();
    _split.dispose();
    super.dispose();
  }

  void _persistRatios() {
    final flexes = _split.areas.map((a) => a.flex ?? 1.0).toList();
    ref.read(prefsProvider).writeDoubleList(StuchkaPrefs.kCasePaneRatio, flexes);
  }

  KeyEventResult _onKey(FocusNode node, KeyEvent event) {
    if (event is! KeyDownEvent) return KeyEventResult.ignored;
    switch (event.logicalKey) {
      case LogicalKeyboardKey.f2:
        _leftFocus.requestFocus();
        return KeyEventResult.handled;
      case LogicalKeyboardKey.f3:
        _centerFocus.requestFocus();
        return KeyEventResult.handled;
      case LogicalKeyboardKey.f4:
        _rightFocus.requestFocus();
        return KeyEventResult.handled;
    }
    return KeyEventResult.ignored;
  }

  @override
  Widget build(BuildContext context) {
    final agg = ref.watch(caseAggregateProvider(widget.caseId));
    final kb = ref.watch(kbVersionProvider).value;

    return Scaffold(
      appBar: AppBar(
        title: const Text('案件工作台'),
        actions: [
          IconButton(
            tooltip: '证据',
            icon: const Icon(StuchkaIcons.evidence),
            onPressed: () => context.go('/case/${widget.caseId}/evidence'),
          ),
          IconButton(
            tooltip: '计算器',
            icon: const Icon(StuchkaIcons.calc),
            onPressed: () => context.go('/case/${widget.caseId}/calc'),
          ),
          IconButton(
            tooltip: '程序对比',
            icon: const Icon(StuchkaIcons.procedure),
            onPressed: () => context.go('/case/${widget.caseId}/procedure-compare'),
          ),
          IconButton(
            tooltip: '文书',
            icon: const Icon(StuchkaIcons.document),
            onPressed: () => context.go('/case/${widget.caseId}/documents'),
          ),
          IconButton(
            tooltip: '风险预警',
            icon: const Icon(StuchkaIcons.alert),
            onPressed: () => context.go('/case/${widget.caseId}/warnings'),
          ),
          IconButton(
            tooltip: '履行 / 执行监控',
            icon: const Icon(StuchkaIcons.deadline),
            onPressed: () => context.go('/case/${widget.caseId}/performance'),
          ),
          IconButton(
            tooltip: '审计',
            icon: const Icon(StuchkaIcons.audit),
            onPressed: () => context.go('/case/${widget.caseId}/audit'),
          ),
        ],
      ),
      body: Focus(
        autofocus: true,
        onKeyEvent: _onKey,
        child: agg.when(
          loading: () => const Center(child: CircularProgressIndicator()),
          error: (e, _) => Center(child: Text('加载失败：$e')),
          data: (aggregate) => Column(
            children: [
              Expanded(
                child: MultiSplitView(
                  controller: _split,
                  onDividerDragEnd: (index) => _persistRatios(),
                  dividerBuilder: (axis, index, resizable, dragging, highlighted, themeData) =>
                      const VerticalDivider(width: 6, thickness: 1),
                  builder: (context, area) {
                    switch (area.data as String) {
                      case 'left':
                        return Focus(
                          focusNode: _leftFocus,
                          child: DocumentTree(roots: buildCaseTree(aggregate)),
                        );
                      case 'center':
                        return Focus(
                          focusNode: _centerFocus,
                          child: FactCardsView(
                            facts: aggregate.facts,
                            onCollectEvidence: (_) =>
                                context.go('/case/${widget.caseId}/evidence'),
                          ),
                        );
                      case 'right':
                      default:
                        return Focus(
                          focusNode: _rightFocus,
                          child: AiAssistantPanel(caseId: widget.caseId),
                        );
                    }
                  },
                ),
              ),
              StuchkaCaseStatusBar(aggregate: aggregate, kbVersion: kb),
            ],
          ),
        ),
      ),
    );
  }
}
