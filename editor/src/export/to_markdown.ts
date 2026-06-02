// to_markdown.ts — Markdown serializer (editor-side final product). Spec §3.9.
//
// Custom serializer:
//   fact node    → `[fact:ID]`
//   lawRef node  → `[law:URN]`   (D8 URN, no short codes)
// The zero-width AI preview marker (inv02_watermark) MUST be stripped from the
// Markdown output (A10) — it only lives in the editor / dossier JSON, never in
// plain-text exports.

import { stripZeroWidth } from '../extensions/inv02_watermark';
import type { PMNodeLike } from './to_json_dossier';

/** Serialize a ProseMirror doc-like tree to Markdown with [fact:ID]/[law:URN] nodes. */
export function toMarkdown(doc: PMNodeLike): string {
  const out: string[] = [];

  const renderInline = (node: PMNodeLike): string => {
    switch (node.type.name) {
      case 'fact': {
        const factId = factAttr(node, 'factId');
        return `[fact:${factId}]`;
      }
      case 'lawRef': {
        const urn = factAttr(node, 'urn');
        return `[law:${urn}]`;
      }
      case 'text': {
        let t = node.text ?? '';
        // Apply bold / italic marks.
        const names = node.marks.map((m) => m.type.name);
        if (names.includes('bold')) t = `**${t}**`;
        if (names.includes('italic')) t = `*${t}*`;
        return t;
      }
      case 'hardBreak':
        return '\n';
      case 'templateSlot': {
        const text = factAttr(node, 'text');
        const key = factAttr(node, 'key');
        return text || `{{${key}}}`;
      }
      default:
        return (node.content ?? []).map(renderInline).join('');
    }
  };

  const renderBlock = (node: PMNodeLike, depth = 0): void => {
    switch (node.type.name) {
      case 'heading': {
        const level = headingLevel(node);
        out.push(`${'#'.repeat(level)} ${(node.content ?? []).map(renderInline).join('')}`);
        out.push('');
        break;
      }
      case 'paragraph': {
        out.push((node.content ?? []).map(renderInline).join(''));
        out.push('');
        break;
      }
      case 'blockquote': {
        for (const child of node.content ?? []) {
          out.push(`> ${(child.content ?? []).map(renderInline).join('')}`);
        }
        out.push('');
        break;
      }
      case 'bulletList': {
        (node.content ?? []).forEach((li) => {
          out.push(`${'  '.repeat(depth)}- ${(li.content ?? []).map((p) => (p.content ?? []).map(renderInline).join('')).join('')}`);
        });
        out.push('');
        break;
      }
      case 'orderedList': {
        (node.content ?? []).forEach((li, i) => {
          out.push(`${'  '.repeat(depth)}${i + 1}. ${(li.content ?? []).map((p) => (p.content ?? []).map(renderInline).join('')).join('')}`);
        });
        out.push('');
        break;
      }
      default: {
        for (const child of node.content ?? []) renderBlock(child, depth);
      }
    }
  };

  for (const child of doc.content ?? []) renderBlock(child);
  // Strip the zero-width AI preview marker from plain-text export (A10).
  return stripZeroWidth(out.join('\n').replace(/\n{3,}/g, '\n\n').trimEnd() + '\n');
}

function factAttr(node: PMNodeLike, key: string): string {
  const attrs = (node as unknown as { attrs?: Record<string, unknown> }).attrs;
  return attrs && attrs[key] != null ? String(attrs[key]) : '';
}

function headingLevel(node: PMNodeLike): number {
  const attrs = (node as unknown as { attrs?: { level?: number } }).attrs;
  const level = attrs?.level ?? 1;
  return Math.min(Math.max(level, 1), 3);
}
