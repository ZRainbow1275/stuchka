// StuchkaLawRefNode — stable law reference (inline atom). Spec §3.2.2.
//
// The `urn` attribute is a D8 URN, e.g.
//   law:中华人民共和国劳动合同法/v2012-12-28/§87/¶1
// NEVER a short code (GB-05 / D8). The 条 `§` 款 `¶` separators are full-width
// per the URN authority. Rendered as "[《劳动合同法》§ 87]".

import { Node, mergeAttributes } from '@tiptap/core';

export const StuchkaLawRefNode = Node.create({
  name: 'lawRef',
  group: 'inline',
  inline: true,
  atom: true,
  selectable: true,

  addAttributes() {
    return {
      // D8 URN (authority: data/02). No short codes.
      urn: { default: null },
      // Human-facing inline title, e.g. "《劳动合同法》§ 87".
      title: { default: '' },
    };
  },

  parseHTML() {
    return [{ tag: 'span[data-law-urn]' }];
  },

  renderHTML({ HTMLAttributes }) {
    const title = (HTMLAttributes.title as string) ?? '';
    return [
      'span',
      mergeAttributes(HTMLAttributes, {
        'data-law-urn': HTMLAttributes.urn,
        class: 'stuchka-law-ref',
      }),
      `[${title}]`,
    ];
  },
});
