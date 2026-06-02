// StuchkaAiSegmentPreviewMark — GB45438 editor-side PREVIEW marker. Spec §3.2.4.
//
// D5 HARD BOUNDARY: the zero-width sequence produced here is ONLY an
// editor-internal "AI paragraph preview marker" + a boundary marker inside the
// exported dossier JSON. It is NEVER the final GB45438 watermark — the final
// four-layer标识 (incl. the PDF binary watermark) is produced solely by the Rust
// backend `crates/document` (pdfium-render + lopdf). The frontend must not claim
// to produce "compliant watermark", and `to_markdown.ts` MUST strip these
// zero-width chars (A10).

import { Extension } from '@tiptap/core';
import { Plugin } from '@tiptap/pm/state';
import { AI_SEGMENT_MARK_NAME } from '../schema/ai_segment_mark';

/** The zero-width combining sequence (ZWSP + ZWNJ + ZWJ). Editor/JSON only. */
export const ZW_MARK = '​‌‍';

/** Strip the preview zero-width marker from plain text (used by markdown export, A10). */
export function stripZeroWidth(text: string): string {
  // Remove the exact preview sequence as well as any stray ZW chars.
  return text.replace(/​|‌|‍/g, '');
}

export const StuchkaAiSegmentPreviewMark = Extension.create({
  name: 'stuchkaAiSegmentPreviewMark',

  addProseMirrorPlugins() {
    return [
      new Plugin({
        appendTransaction: (_trs, _oldState, newState) => {
          const tr = newState.tr;
          newState.doc.descendants((node, pos) => {
            if (node.type.name !== 'paragraph') return;
            // Only AI-marked paragraphs get the preview boundary.
            const hasAiMark = node.content.content.some((child) =>
              child.marks.some((m) => m.type.name === AI_SEGMENT_MARK_NAME),
            );
            if (!hasAiMark) return;
            if (node.textContent.endsWith(ZW_MARK)) return;
            tr.insertText(ZW_MARK, pos + node.nodeSize - 1);
          });
          return tr.docChanged ? tr : null;
        },
      }),
    ];
  },
});
