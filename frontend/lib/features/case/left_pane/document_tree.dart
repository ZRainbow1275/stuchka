import 'package:flutter/material.dart';

import '../../../ipc/dto/dtos.dart';
import '../../../ipc/dto/enums.dart';
import '../../../theme/stuchka_icons.dart';
import '../../../theme/stuchka_theme.dart';

/// Six tree node kinds (spec 04 §4.2 / brief §3.2).
enum CaseTreeNodeKind {
  caseRoot,
  factGroup,
  evidenceGroup,
  documentGroup,
  calculationGroup,
  deadlineGroup,
}

extension CaseTreeNodeKindInfo on CaseTreeNodeKind {
  IconData get icon => switch (this) {
        CaseTreeNodeKind.caseRoot => StuchkaIcons.caseRoot,
        CaseTreeNodeKind.factGroup => StuchkaIcons.factGroup,
        CaseTreeNodeKind.evidenceGroup => StuchkaIcons.evidenceGroup,
        CaseTreeNodeKind.documentGroup => StuchkaIcons.documentGroup,
        CaseTreeNodeKind.calculationGroup => StuchkaIcons.calculationGroup,
        CaseTreeNodeKind.deadlineGroup => StuchkaIcons.deadlineGroup,
      };
}

/// Node lifecycle status (pending/confirmed/frozen/risk) — drives the left border tint.
enum NodeStatus { pending, confirmed, frozen, risk }

class CaseTreeNode {
  CaseTreeNode({
    required this.id,
    required this.kind,
    required this.label,
    this.status = NodeStatus.pending,
    this.children = const [],
  });

  final String id;
  final CaseTreeNodeKind kind;
  final String label;
  final NodeStatus status;
  final List<CaseTreeNode> children;
}

/// Build the six-group tree for a case aggregate.
List<CaseTreeNode> buildCaseTree(CaseAggregateDto agg) {
  final frozen = agg.caseDto.status == CaseStatus.frozen;
  NodeStatus s(NodeStatus base) => frozen ? NodeStatus.frozen : base;

  return [
    CaseTreeNode(
      id: agg.caseDto.id,
      kind: CaseTreeNodeKind.caseRoot,
      label: agg.caseDto.shortLabel,
      status: s(NodeStatus.confirmed),
      children: [
        CaseTreeNode(
          id: 'facts',
          kind: CaseTreeNodeKind.factGroup,
          label: '事实（${agg.facts.length}）',
          status: s(NodeStatus.pending),
        ),
        CaseTreeNode(
          id: 'evidence',
          kind: CaseTreeNodeKind.evidenceGroup,
          label: '证据（${agg.evidences.length}）',
          status: s(NodeStatus.pending),
        ),
        CaseTreeNode(
          id: 'documents',
          kind: CaseTreeNodeKind.documentGroup,
          label: '文书（${agg.documents.length}）',
          status: s(NodeStatus.pending),
        ),
        CaseTreeNode(
          id: 'calc',
          kind: CaseTreeNodeKind.calculationGroup,
          label: '计算',
          status: s(NodeStatus.pending),
        ),
        CaseTreeNode(
          id: 'deadline',
          kind: CaseTreeNodeKind.deadlineGroup,
          label: '时效',
          status: s(NodeStatus.risk),
        ),
      ],
    ),
  ];
}

/// Left pane document tree. A frozen case (INV-04) shows the墨蓝 frozen border around the whole
/// tree.
class DocumentTree extends StatelessWidget {
  const DocumentTree({super.key, required this.roots, this.onSelect});

  final List<CaseTreeNode> roots;
  final void Function(CaseTreeNode node)? onSelect;

  @override
  Widget build(BuildContext context) {
    final colors = StuchkaSemanticColors.of(context);
    final frozen = roots.any((r) => _anyFrozen(r));
    return Container(
      decoration: frozen
          ? BoxDecoration(border: Border.all(color: colors.frozen, width: 1.5))
          : null,
      child: ListView(
        padding: const EdgeInsets.all(8),
        children: roots.map((r) => _TreeNodeTile(node: r, depth: 0, onSelect: onSelect)).toList(),
      ),
    );
  }

  bool _anyFrozen(CaseTreeNode n) =>
      n.status == NodeStatus.frozen || n.children.any(_anyFrozen);
}

class _TreeNodeTile extends StatelessWidget {
  const _TreeNodeTile({required this.node, required this.depth, this.onSelect});
  final CaseTreeNode node;
  final int depth;
  final void Function(CaseTreeNode node)? onSelect;

  @override
  Widget build(BuildContext context) {
    final colors = StuchkaSemanticColors.of(context);
    final statusColor = switch (node.status) {
      NodeStatus.pending => colors.riskMid,
      NodeStatus.confirmed => colors.riskLow,
      NodeStatus.frozen => colors.frozen,
      NodeStatus.risk => colors.sealRed,
    };
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        InkWell(
          onTap: () => onSelect?.call(node),
          child: Padding(
            padding: EdgeInsets.only(left: 8.0 + depth * 16, top: 6, bottom: 6, right: 8),
            child: Row(
              children: [
                Icon(node.kind.icon, size: 16, color: statusColor),
                const SizedBox(width: 8),
                Expanded(child: Text(node.label, overflow: TextOverflow.ellipsis)),
              ],
            ),
          ),
        ),
        ...node.children.map((c) => _TreeNodeTile(node: c, depth: depth + 1, onSelect: onSelect)),
      ],
    );
  }
}
