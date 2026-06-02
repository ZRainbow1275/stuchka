// StuchkaFactNode — embedded fact-id card (inline atom). Spec §3.2.3.
//
// `state` enum MUST match the backend `FactStatus` enum
// (`crates/data-model/src/enums.rs`): pending | confirmed | disputed | deprecated.

import { Node, mergeAttributes } from '@tiptap/core';

/** Fact lifecycle states (must equal backend FactStatus snake_case). */
export const FACT_STATES = ['pending', 'confirmed', 'disputed', 'deprecated'] as const;
export type FactState = (typeof FACT_STATES)[number];

export const StuchkaFactNode = Node.create({
  name: 'fact',
  group: 'inline',
  inline: true,
  atom: true,
  selectable: true,

  addAttributes() {
    return {
      factId: { default: null },
      // pending / confirmed / disputed / deprecated
      state: { default: 'pending' as FactState },
      confidence: { default: 0 },
    };
  },

  parseHTML() {
    return [{ tag: 'span[data-fact-id]' }];
  },

  renderHTML({ HTMLAttributes }) {
    return [
      'span',
      mergeAttributes(HTMLAttributes, {
        'data-fact-id': HTMLAttributes.factId,
        'data-state': HTMLAttributes.state,
        class: 'stuchka-fact',
      }),
    ];
  },
});
