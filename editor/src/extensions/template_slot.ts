// StuchkaTemplateSlot — placeholder node `{{case.respondent_name}}`. Spec §3.2.2 / §3.7.
//
// A slot is an inline atom carrying a `key` (e.g. `applicant.name`) and an
// `locked` flag. The INV-10 settlement disclaimer slot is rendered `locked` so
// it cannot be deleted by the user (GB-10); the editor enforces deletion guards
// off this flag and the template JSON marks it non-deletable.

import { Node, mergeAttributes } from '@tiptap/core';

export const StuchkaTemplateSlot = Node.create({
  name: 'templateSlot',
  group: 'inline',
  inline: true,
  atom: true,
  selectable: true,

  addAttributes() {
    return {
      key: { default: '' },
      // Non-deletable slots (e.g. INV-10 disclaimer) set this true.
      locked: { default: false },
      // Static text for locked slots (e.g. the full INV-10 disclaimer).
      text: { default: '' },
    };
  },

  parseHTML() {
    return [{ tag: 'span[data-slot-key]' }];
  },

  renderHTML({ HTMLAttributes }) {
    const key = (HTMLAttributes.key as string) ?? '';
    const locked = HTMLAttributes.locked === true;
    const text = (HTMLAttributes.text as string) ?? '';
    return [
      'span',
      mergeAttributes(HTMLAttributes, {
        'data-slot-key': key,
        'data-locked': locked ? 'true' : 'false',
        class: locked ? 'stuchka-slot stuchka-slot-locked' : 'stuchka-slot',
      }),
      locked && text ? text : `{{${key}}}`,
    ];
  },
});
