// Three-segment cards (先结论 / 再依据 / 最后展开) + AI mark.
//
// Reuses the Prototype tokens.css visual authority classes (.conclusion-card /
// .evidence-list / .evidence-item / .expand-drawer / .ai-mark). These are the
// visual authority — we render the structure, never reinvent the colors (A24).
// Source tags use the four .src-* chips with Lucide icons (A25). The AI mark is
// the fixed bottom-right [AI] preview-level marker (A26).

import { renderSourceTag, type SourceTag } from './source_tags';
import { lucideSvg } from './icons/lucide';

export interface EvidenceRow {
  source: SourceTag;
  title: string;
  meta?: string;
}

export interface ThreeSegmentCardModel {
  /** 先结论. */
  conclusion: { label: string; headline: string; body: string };
  /** 再依据. */
  evidence: { heading: string; rows: EvidenceRow[] };
  /** 最后展开. */
  expand?: { heading: string; body: string };
}

/** Render the three-segment conclusion/evidence/expand structure into a container. */
export function renderThreeSegmentCard(container: HTMLElement, model: ThreeSegmentCardModel): void {
  container.innerHTML = '';

  // 先结论.
  const card = document.createElement('section');
  card.className = 'conclusion-card';
  card.innerHTML =
    `<div class="label">${escapeHtml(model.conclusion.label)}</div>` +
    `<div class="headline">${escapeHtml(model.conclusion.headline)}</div>` +
    `<p>${escapeHtml(model.conclusion.body)}</p>`;
  container.appendChild(card);

  // 再依据.
  const list = document.createElement('section');
  list.className = 'evidence-list';
  const h = document.createElement('h3');
  h.textContent = model.evidence.heading;
  list.appendChild(h);
  model.evidence.rows.forEach((row) => {
    const item = document.createElement('div');
    item.className = 'evidence-item';
    item.innerHTML =
      renderSourceTag(row.source) +
      `<span class="ev-title">${escapeHtml(row.title)}</span>` +
      `<span class="ev-meta">${escapeHtml(row.meta ?? '')}</span>`;
    list.appendChild(item);
  });
  container.appendChild(list);

  // 最后展开.
  if (model.expand) {
    const drawer = document.createElement('section');
    drawer.className = 'expand-drawer';
    drawer.innerHTML =
      `<h3>${escapeHtml(model.expand.heading)}</h3>` +
      `<p>${escapeHtml(model.expand.body)}</p>`;
    container.appendChild(drawer);
  }
}

/**
 * Mount the fixed bottom-right [AI] preview-level mark (A26). The `[AI]` prefix
 * is supplied by the `.ai-mark::before` CSS rule; this function only mounts the
 * element with its body text. Editor-internal preview level — NOT the final
 * watermark (D5).
 */
export function mountAiMark(root: HTMLElement, text = '本页含 AI 辅助生成内容'): HTMLElement {
  const el = document.createElement('div');
  el.className = 'ai-mark';
  el.textContent = text;
  root.appendChild(el);
  return el;
}

/** A small warning banner using Lucide AlertTriangle (Prototype Emoji replacement, I6). */
export function renderWarning(text: string): string {
  return `<span class="stuchka-warn">${lucideSvg('alert-triangle', 14)}<span>${escapeHtml(text)}</span></span>`;
}

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}
