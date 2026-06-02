// A9: each StuchkaAiSegmentMark mark == exactly one ai_generated_segments entry.

import { describe, it, expect } from 'vitest';
import { exportDossier, type EditorStateLike, type PMNodeLike } from '../src/export/to_json_dossier';
import { AI_SEGMENT_MARK_NAME } from '../src/schema/ai_segment_mark';

function aiMark(modelTag: string, confidence: number) {
  return { type: { name: AI_SEGMENT_MARK_NAME }, attrs: { modelTag, confidence } };
}

function text(t: string, marks: PMNodeLike['marks'] = []): PMNodeLike {
  return { type: { name: 'text' }, marks, text: t };
}

function paragraph(children: PMNodeLike[]): PMNodeLike {
  return { type: { name: 'paragraph' }, marks: [], content: children };
}

function makeState(content: PMNodeLike[]): EditorStateLike {
  const doc = { type: { name: 'doc' }, marks: [], content, toJSON: () => ({ content }) };
  return { doc };
}

describe('dossier ai_generated_segments', () => {
  it('emits exactly one entry per AI-marked span', () => {
    const state = makeState([
      paragraph([
        text('普通文本 '),
        text('AI 草拟段一', [aiMark('deepseek-v3.1', 0.82)]),
      ]),
      paragraph([text('AI 草拟段二', [aiMark('deepseek-v3.1', 0.7)])]),
      paragraph([text('又是普通文本')]),
    ]);

    const dossier = exportDossier(state, { id: 'case-1', docId: 'doc-1', kbHash: 'kb-1' });

    expect(dossier.ai_generated_segments).toHaveLength(2);
    expect(dossier.ai_generated_segments[0]).toMatchObject({
      text: 'AI 草拟段一',
      modelTag: 'deepseek-v3.1',
      confidence: 0.82,
    });
    expect(dossier.ai_generated_segments[1]).toMatchObject({ text: 'AI 草拟段二' });
  });

  it('produces no entries when there are no AI marks (no false positive)', () => {
    const state = makeState([paragraph([text('全为人工撰写')])]);
    const dossier = exportDossier(state, { id: 'c', docId: 'd', kbHash: 'k' });
    expect(dossier.ai_generated_segments).toHaveLength(0);
  });

  it('carries the dossier schema + case metadata', () => {
    const state = makeState([paragraph([text('x', [aiMark('m', 0.5)])])]);
    const dossier = exportDossier(state, { id: 'case-7', docId: 'doc-9', kbHash: 'hash-z' });
    expect(dossier.schemaVersion).toBe(1);
    expect(dossier.caseId).toBe('case-7');
    expect(dossier.docId).toBe('doc-9');
    expect(dossier.knowledgeBaseHash).toBe('hash-z');
    expect(dossier.body).toBeDefined();
  });
});
