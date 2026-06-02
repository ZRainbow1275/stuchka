// StuchkaSanitizePaste — strip external styles/classes on paste. Spec §3.2.2.
//
// Office/WPS paste injects inline `style`, `class`, `<font>`, `mso-*` noise. For
// legal documents we keep only a whitelist of structural tags and drop all
// presentational attributes, so the document model stays clean.

import { Extension } from '@tiptap/core';
import { Plugin } from '@tiptap/pm/state';

const ALLOWED_TAGS = new Set([
  'P', 'BR', 'H1', 'H2', 'H3', 'UL', 'OL', 'LI', 'BLOCKQUOTE', 'STRONG', 'B', 'EM', 'I', 'SPAN',
]);

/** Recursively strip presentational attributes + disallowed tags from an element subtree. */
export function sanitizeElement(el: Element): void {
  // Remove noisy attributes on every element.
  for (const attr of Array.from(el.attributes)) {
    const name = attr.name.toLowerCase();
    const keepData =
      name === 'data-fact-id' ||
      name === 'data-law-urn' ||
      name === 'data-ai-segment' ||
      name === 'data-model-tag' ||
      name === 'data-state';
    if (name === 'style' || name === 'class' || name.startsWith('mso-') || (!keepData && name.startsWith('on'))) {
      el.removeAttribute(attr.name);
    }
  }
  // Recurse, unwrapping disallowed tags (e.g. <font>).
  for (const child of Array.from(el.children)) {
    sanitizeElement(child);
    if (!ALLOWED_TAGS.has(child.tagName) && !child.hasAttribute('data-fact-id') && !child.hasAttribute('data-law-urn') && !child.hasAttribute('data-ai-segment')) {
      // Unwrap: replace the node with its children.
      const parent = child.parentNode;
      if (parent) {
        while (child.firstChild) parent.insertBefore(child.firstChild, child);
        parent.removeChild(child);
      }
    }
  }
}

export const StuchkaSanitizePaste = Extension.create({
  name: 'stuchkaSanitizePaste',

  addProseMirrorPlugins() {
    return [
      new Plugin({
        props: {
          transformPastedHTML: (html) => {
            // Best-effort DOM cleanup; only available where DOMParser exists.
            const DP = (globalThis as unknown as { DOMParser?: typeof DOMParser }).DOMParser;
            if (!DP) return html;
            const doc = new DP().parseFromString(html, 'text/html');
            sanitizeElement(doc.body);
            return doc.body.innerHTML;
          },
        },
      }),
    ];
  },
});
