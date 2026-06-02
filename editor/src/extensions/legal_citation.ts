// StuchkaLegalCitation — `§` triggers law-ref autocomplete. Spec §3.2.2.
//
// On typing the full-width section sign `§` the shell is notified (channel A
// `editor.cmd`) to open the law-ref picker. The actual picker UI / law list
// lives in the Dart shell + backend `/law-ref`; this extension only detects the
// trigger and relays it. No short codes are ever created here (D8).

import { Extension } from '@tiptap/core';
import { Plugin } from '@tiptap/pm/state';
import { postBack } from '../bridge';

/** The trigger character (full-width section sign, kept verbatim per §2.7.2). */
export const LAW_CITATION_TRIGGER = '§';

export const StuchkaLegalCitation = Extension.create({
  name: 'stuchkaLegalCitation',

  addProseMirrorPlugins() {
    return [
      new Plugin({
        props: {
          handleTextInput: (_view, from, _to, text) => {
            if (text !== LAW_CITATION_TRIGGER) return false;
            // Relay the trigger to the shell; do NOT consume the character so the
            // `§` is still inserted (full-width punctuation preserved, §2.7.2).
            postBack({
              type: 'editor.cmd',
              payload: { command: 'lawRefPicker.open', args: { at: from } },
            });
            return false;
          },
          // Surface the current selection range so the picker can anchor.
          handleClick: (view) => {
            const { from, to } = view.state.selection;
            postBack({
              type: 'editor.selection',
              payload: { from, to, anchorOnFact: false },
            });
            return false;
          },
        },
      }),
    ];
  },
});
