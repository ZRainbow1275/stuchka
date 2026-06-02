// conflict_resolver.ts — conflict block types + resolution relay. Spec §3.5.

import type { PMNodeLike } from '../export/to_json_dossier';

/** Conflict resolution choice (matches backend `MergeWinner`: ancestor|mine|theirs|manual). */
export type MergeChoice = 'ancestor' | 'mine' | 'theirs' | 'manual';

/** A single block in three-way merge. Status mirrors the resolved-* states. */
export interface ConflictBlock {
  blockId: string;
  ancestor: PMNodeLike;
  mine: PMNodeLike;
  theirs: PMNodeLike;
  status: 'pending' | 'resolved-mine' | 'resolved-theirs' | 'resolved-manual';
}

/** Map a user choice to the resolved status. */
export function resolvedStatus(choice: MergeChoice): ConflictBlock['status'] {
  switch (choice) {
    case 'mine':
      return 'resolved-mine';
    case 'theirs':
      return 'resolved-theirs';
    case 'manual':
      return 'resolved-manual';
    case 'ancestor':
    default:
      return 'resolved-mine';
  }
}
