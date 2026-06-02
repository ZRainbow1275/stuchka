// to_json_dossier.ts — dossier JSON (editor-side final product). Spec §3.8.
//
// GB45438 Layer 4: walk every node carrying the `stuchkaAiSegment` mark and emit
// exactly one `ai_generated_segments[]` entry per mark span (no omission / no
// false positive, A9). This array is the frontend's ONLY substantive GB45438
// contribution; the backend `crates/document` consumes it to write the binary
// watermark. The frontend must NOT self-judge "four-layer complete" (A12).

import { AI_SEGMENT_MARK_NAME } from '../schema/ai_segment_mark';

/** Minimal structural view of a ProseMirror node (decoupled from PM types for testability). */
export interface PMNodeLike {
  type: { name: string };
  marks: { type: { name: string }; attrs: Record<string, unknown> }[];
  text?: string;
  content?: PMNodeLike[];
}

/** Minimal editor state view used by the exporter. */
export interface EditorStateLike {
  doc: PMNodeLike & { toJSON(): unknown };
}

/** Case metadata supplied by the shell at export time. */
export interface CaseMeta {
  id: string;
  docId: string;
  kbHash: string;
}

/** One AI-generated segment entry (spec §3.8 shape). */
export interface AiSegment {
  pos: number;
  text: string;
  modelTag: string;
  confidence: number;
}

/** The exported dossier JSON (editor-side final product). */
export interface DossierJson {
  schemaVersion: 1;
  caseId: string;
  docId: string;
  knowledgeBaseHash: string;
  ai_generated_segments: AiSegment[];
  body: unknown;
  exportedAt: string;
}

/**
 * Depth-first walk that mirrors ProseMirror's `descendants`, tracking position.
 * For each contiguous run carrying the AI-segment mark we emit one entry. We emit
 * per text node (the unit the mark applies to), which yields a 1:1 mapping
 * between mark spans and `ai_generated_segments[]` entries (A9).
 */
function collectAiSegments(doc: PMNodeLike): AiSegment[] {
  const segments: AiSegment[] = [];
  let pos = 0;

  const visit = (node: PMNodeLike): void => {
    const aiMark = node.marks.find((m) => m.type.name === AI_SEGMENT_MARK_NAME);
    if (aiMark) {
      segments.push({
        pos,
        text: node.text ?? '',
        modelTag: (aiMark.attrs.modelTag as string) ?? '',
        confidence: (aiMark.attrs.confidence as number) ?? 0,
      });
    }
    if (node.text !== undefined) {
      pos += node.text.length;
    } else {
      pos += 1; // entering a non-leaf node consumes one position
      for (const child of node.content ?? []) visit(child);
      pos += 1; // leaving the node
    }
  };

  for (const child of doc.content ?? []) visit(child);
  return segments;
}

/** Export the editor state to a dossier JSON (GB45438 Layer 4 input). */
export function exportDossier(state: EditorStateLike, caseMeta: CaseMeta): DossierJson {
  return {
    schemaVersion: 1,
    caseId: caseMeta.id,
    docId: caseMeta.docId,
    knowledgeBaseHash: caseMeta.kbHash,
    ai_generated_segments: collectAiSegments(state.doc),
    body: state.doc.toJSON(),
    exportedAt: new Date().toISOString(),
  };
}
