// Markdown export: fact node → [fact:ID], law node → [law:URN] (spec §3.9).
// Also A10: zero-width AI preview marker is stripped from Markdown output.

import { describe, it, expect } from 'vitest';
import { toMarkdown } from '../src/export/to_markdown';
import { ZW_MARK } from '../src/extensions/inv02_watermark';
import type { PMNodeLike } from '../src/export/to_json_dossier';

function node(name: string, attrs: Record<string, unknown>, content: PMNodeLike[] = [], text?: string): PMNodeLike {
  return { type: { name }, marks: [], content, text, ...(Object.keys(attrs).length ? { attrs } : {}) } as PMNodeLike;
}

function text(t: string, marks: PMNodeLike['marks'] = []): PMNodeLike {
  return { type: { name: 'text' }, marks, text: t };
}

function doc(content: PMNodeLike[]): PMNodeLike {
  return { type: { name: 'doc' }, marks: [], content };
}

describe('markdown fact/law node forms', () => {
  it('renders a fact node as [fact:ID]', () => {
    const d = doc([node('paragraph', {}, [text('依据 '), node('fact', { factId: 'F-019' })])]);
    expect(toMarkdown(d)).toContain('[fact:F-019]');
  });

  it('renders a law-ref node as [law:URN] with the full D8 URN (no short code)', () => {
    const urn = 'law:中华人民共和国劳动合同法/v2012-12-28/§87/¶1';
    const d = doc([node('paragraph', {}, [text('适用 '), node('lawRef', { urn, title: '《劳动合同法》§ 87' })])]);
    const md = toMarkdown(d);
    expect(md).toContain(`[law:${urn}]`);
    expect(md).not.toMatch(/LCL-\d{4}-\d+/); // never a short code
  });

  it('renders bold and ordered lists', () => {
    const d = doc([
      node('heading', { level: 1 }, [text('仲裁申请书')]),
      node('orderedList', {}, [
        node('listItem', {}, [node('paragraph', {}, [text('裁决支付欠薪 ')])]),
        node('listItem', {}, [node('paragraph', {}, [text('裁决支付补偿', [{ type: { name: 'bold' }, attrs: {} }])])]),
      ]),
    ]);
    const md = toMarkdown(d);
    expect(md).toContain('# 仲裁申请书');
    expect(md).toContain('1. 裁决支付欠薪');
    expect(md).toContain('**裁决支付补偿**');
  });
});

describe('zero-width preview marker boundary (A10)', () => {
  it('is NOT present in to_markdown output even when the editor text carries it', () => {
    const d = doc([node('paragraph', {}, [text(`AI 草拟内容${ZW_MARK}`)])]);
    const md = toMarkdown(d);
    expect(md).not.toContain('​');
    expect(md).not.toContain('‌');
    expect(md).not.toContain('‍');
    expect(md).toContain('AI 草拟内容');
  });
});
