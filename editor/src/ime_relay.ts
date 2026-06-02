// ime_relay.ts — IME composition event forwarding. Spec §2.7.
//
// Listens for compositionstart/update/end and relays the caret bounding rect over
// channel A so the Dart shell can position the candidate window. During
// composition NO side effects fire (no evaluateJavascript, no re-render); we only
// post coordinates. Full-width punctuation is preserved (§2.7.2): no half-width
// auto-conversion / smart-quote input rules are installed.

import { postBack } from './bridge';

export function installImeRelay(editorRoot: HTMLElement): () => void {
  const onStart = (): void => {
    const rect = caretRect();
    postBack({
      type: 'ime.compositionStart',
      payload: { x: rect.x, y: rect.y, height: rect.height },
    });
  };
  const onEnd = (): void => {
    postBack({ type: 'ime.compositionEnd', payload: {} });
  };

  editorRoot.addEventListener('compositionstart', onStart);
  editorRoot.addEventListener('compositionend', onEnd);

  return () => {
    editorRoot.removeEventListener('compositionstart', onStart);
    editorRoot.removeEventListener('compositionend', onEnd);
  };
}

function caretRect(): { x: number; y: number; height: number } {
  const sel = (globalThis as unknown as { getSelection?: () => Selection | null }).getSelection?.();
  if (sel && sel.rangeCount > 0) {
    const r = sel.getRangeAt(0).getBoundingClientRect();
    return { x: r.left, y: r.top, height: r.height || 24 };
  }
  return { x: 0, y: 0, height: 24 };
}
