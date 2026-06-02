// StuchkaAiSegmentMark — marks AI-generated spans. Spec §3.2.2 / §3.8.
//
// Every span carrying this mark contributes exactly one entry to
// `DossierJson.ai_generated_segments[]` on export (GB45438 Layer 4, the
// frontend's only substantive contribution). The mark name is `stuchkaAiSegment`
// (the export + preview-marker extensions key off this exact name).

import { Mark, mergeAttributes } from '@tiptap/core';

/** The canonical mark name keyed by export + preview marker. */
export const AI_SEGMENT_MARK_NAME = 'stuchkaAiSegment';

export const StuchkaAiSegmentMark = Mark.create({
  name: AI_SEGMENT_MARK_NAME,

  addAttributes() {
    return {
      // Model identifier, e.g. "deepseek-v3.1-2026Q1".
      modelTag: { default: '' },
      // Model confidence in [0, 1].
      confidence: { default: 0 },
    };
  },

  parseHTML() {
    return [{ tag: 'span[data-ai-segment]' }];
  },

  renderHTML({ HTMLAttributes }) {
    return [
      'span',
      mergeAttributes(HTMLAttributes, {
        'data-ai-segment': 'true',
        'data-model-tag': HTMLAttributes.modelTag,
        class: 'stuchka-ai-segment',
      }),
      0,
    ];
  },
});
