// block_diff.ts — block-level diff (ProseMirror Slice granularity). Spec §3.5.
//
// We diff at block granularity (not character) so the three-way view only shows
// blocks whose ancestor/mine/theirs text differs; auto-merged blocks are hidden.

import type { PMNodeLike } from '../export/to_json_dossier';

function blockText(node: PMNodeLike): string {
  if (node.text !== undefined) return node.text;
  return (node.content ?? []).map(blockText).join('');
}

/** A block is conflicting when both sides diverge from the ancestor differently. */
export function isConflicting(
  ancestor: PMNodeLike,
  mine: PMNodeLike,
  theirs: PMNodeLike,
): boolean {
  const a = blockText(ancestor);
  const m = blockText(mine);
  const t = blockText(theirs);
  const mineChanged = m !== a;
  const theirsChanged = t !== a;
  // Conflict only when both sides changed AND they disagree (true 3-way conflict).
  return mineChanged && theirsChanged && m !== t;
}

/** Filter to only the conflicting (pending) blocks for the three-way view. */
export function pendingConflicts<
  B extends { ancestor: PMNodeLike; mine: PMNodeLike; theirs: PMNodeLike; status: string },
>(blocks: B[]): B[] {
  return blocks.filter(
    (b) => b.status === 'pending' && isConflicting(b.ancestor, b.mine, b.theirs),
  );
}
