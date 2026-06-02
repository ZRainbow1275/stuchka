// A16: the Tiptap editor enables exactly 18 extensions (StarterKit subset + 7
// self-built), excluding Strike/Code/CodeBlock/HorizontalRule/Mention.
// A17: in collab mode the built-in History is disabled (undo via y-prosemirror).

import { describe, it, expect } from 'vitest';
import { Editor } from '@tiptap/core';
import * as Y from 'yjs';
import { buildSchemaExtensions, REQUIRED_EXTENSION_NAMES, EXCLUDED_EXTENSION_NAMES } from '../src/schema/stuchka_schema';
import { buildCollabExtensions, BridgeYjsProvider } from '../src/extensions/collab_yjs';

function resolvedNames(): Set<string> {
  const ydoc = new Y.Doc();
  const provider = new BridgeYjsProvider(ydoc);
  const el = document.createElement('div');
  const editor = new Editor({
    element: el,
    extensions: [...buildSchemaExtensions(), ...buildCollabExtensions(provider)],
  });
  // The extension manager flattens StarterKit into its child extensions.
  const names = new Set(editor.extensionManager.extensions.map((e) => e.name));
  editor.destroy();
  provider.destroy();
  ydoc.destroy();
  return names;
}

describe('eighteen extensions', () => {
  it('REQUIRED_EXTENSION_NAMES has exactly 18 entries', () => {
    expect(REQUIRED_EXTENSION_NAMES).toHaveLength(18);
    expect(new Set(REQUIRED_EXTENSION_NAMES).size).toBe(18);
  });

  it('enables every one of the 18 required schema extensions', () => {
    const names = resolvedNames();
    for (const required of REQUIRED_EXTENSION_NAMES) {
      expect(names.has(required), `missing extension: ${required}`).toBe(true);
    }
  });

  it('excludes Strike/Code/CodeBlock/HorizontalRule/Mention', () => {
    const names = resolvedNames();
    for (const excluded of EXCLUDED_EXTENSION_NAMES) {
      expect(names.has(excluded), `must NOT enable: ${excluded}`).toBe(false);
    }
  });

  it('disables built-in History in collab mode (undo via y-prosemirror, A17)', () => {
    const names = resolvedNames();
    expect(names.has('history')).toBe(false);
    // Collaboration is what supplies undo/redo.
    expect(names.has('collaboration')).toBe(true);
  });
});
