// StuchkaForbidEmoji — permanently block Emoji at input + paste. Spec §3.4 / §1.8.
//
// Two ProseMirror plugin props:
//   handleTextInput → returns true (swallow) when the typed text contains Emoji.
//   handlePaste     → strips Emoji from the pasted plain text, then inserts the
//                     cleaned text (returns true so the default paste is cancelled).
//
// EMOJI_RE matches the Unicode blocks named in spec §1.8 / §3.4.

import { Extension } from '@tiptap/core';
import { Plugin } from '@tiptap/pm/state';

/** The frozen Emoji detection regex (spec §3.4). */
export const EMOJI_RE = /[\u{1F300}-\u{1FAFF}\u{2600}-\u{27BF}\u{1F000}-\u{1F2FF}]/u;

/** Strip every Emoji code point from a string (used by paste handling + tests). */
export function stripEmoji(text: string): string {
  return text.replace(new RegExp(EMOJI_RE, 'gu'), '');
}

export const StuchkaForbidEmoji = Extension.create({
  name: 'stuchkaForbidEmoji',

  addProseMirrorPlugins() {
    return [
      new Plugin({
        props: {
          // Block Emoji being typed / IME-committed.
          handleTextInput: (_view, _from, _to, text) => EMOJI_RE.test(text),
          // Strip Emoji out of pasted plain text.
          handlePaste: (view, ev) => {
            const text = ev.clipboardData?.getData('text/plain') ?? '';
            if (!EMOJI_RE.test(text)) return false;
            const cleaned = stripEmoji(text);
            view.dispatch(view.state.tr.insertText(cleaned));
            return true;
          },
        },
      }),
    ];
  },
});
