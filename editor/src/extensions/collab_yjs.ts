// Yjs collaboration binding (channel A only). Spec §3.6.
//
// The WebView holds a single `Y.Doc`; the `BridgeYjsProvider` relays Yjs updates
// to/from the Dart shell over channel A (`editor.step` / `editor.applyRemote`)
// and NEVER opens a socket itself (CSP `connect-src 'none'`, D1). LAN multi-peer
// sync is the Dart `YjsWsProvider`'s job (channel B), not the editor's.
//
// In collab mode Tiptap's built-in History is DISABLED — undo/redo is handled by
// y-prosemirror's undo plugin via the Collaboration extension (A17).

import Collaboration from '@tiptap/extension-collaboration';
import CollaborationCursor from '@tiptap/extension-collaboration-cursor';
import * as Y from 'yjs';
import { applyUpdate, encodeStateAsUpdate } from 'yjs';
import { Awareness } from 'y-protocols/awareness';
import { postBack } from '../bridge';

/** XML fragment key shared with the backend Yjs doc. */
export const YJS_FRAGMENT = 'stuchka-doc';

/**
 * Channel-A Yjs provider: bridges the in-process `Y.Doc` to the Dart shell.
 * - Local updates → surfaced UPSTREAM via the distinct `editor.localUpdate` type
 *   (E3) so the shell can persist them. `editor.applyRemote` is downstream-only and
 *   is NEVER reused for local pushes (no `direction` flag overload). The shell also
 *   forwards ProseMirror steps via the Collaboration extension's transaction hook in
 *   main.ts.
 * - Remote updates from the shell are applied via `applyRemote`.
 */
export class BridgeYjsProvider {
  readonly doc: Y.Doc;
  /** Local awareness state (cursors). LAN multi-peer awareness is the Dart side's job. */
  readonly awareness: Awareness;
  private readonly origin = Symbol('bridge-yjs');

  constructor(doc: Y.Doc) {
    this.doc = doc;
    this.awareness = new Awareness(doc);
    this.doc.on('update', this.onLocalUpdate);
  }

  private onLocalUpdate = (update: Uint8Array, origin: unknown): void => {
    if (origin === this.origin) return; // ignore updates we applied from remote
    // Upstream push uses the distinct `editor.localUpdate` type (E3). The previous
    // `editor.applyRemote` + `direction:"up"` overload conflated the downstream-only
    // apply path with the local push; the Dart side mirrors `editor.localUpdate`.
    postBack({
      type: 'editor.localUpdate',
      payload: { updateBytes: toBase64(update) },
    });
  };

  /** Apply a remote Yjs update (base64) received from the Dart shell. */
  applyRemote(updateBase64: string): void {
    Y.transact(this.doc, () => {
      applyUpdate(this.doc, fromBase64(updateBase64), this.origin);
    });
  }

  /** Encode the full state as a base64 update (bootstrap handshake). */
  encodeState(): string {
    return toBase64(encodeStateAsUpdate(this.doc));
  }

  destroy(): void {
    this.doc.off('update', this.onLocalUpdate);
    this.awareness.destroy();
  }
}

/**
 * Build the two collaboration extensions bound to the given provider.
 * Collaboration supplies undo/redo (replacing the disabled built-in History);
 * CollaborationCursor renders remote cursors off the bridge awareness.
 */
export function buildCollabExtensions(provider: BridgeYjsProvider) {
  return [
    Collaboration.configure({ document: provider.doc, field: YJS_FRAGMENT }),
    CollaborationCursor.configure({
      provider: { awareness: provider.awareness } as never,
    }),
  ];
}

interface NodeBuffer {
  from(input: Uint8Array | string, enc?: string): { toString(enc: string): string } & Uint8Array;
}

function nodeBuffer(): NodeBuffer | undefined {
  return (globalThis as unknown as { Buffer?: NodeBuffer }).Buffer;
}

function toBase64(bytes: Uint8Array): string {
  let binary = '';
  for (let i = 0; i < bytes.length; i += 1) binary += String.fromCharCode(bytes[i]);
  const b = (globalThis as unknown as { btoa?: (s: string) => string }).btoa;
  if (b) return b(binary);
  // Node fallback for tests.
  const buf = nodeBuffer();
  return buf ? buf.from(bytes).toString('base64') : binary;
}

function fromBase64(b64: string): Uint8Array {
  const a = (globalThis as unknown as { atob?: (s: string) => string }).atob;
  if (a) {
    const binary = a(b64);
    const out = new Uint8Array(binary.length);
    for (let i = 0; i < binary.length; i += 1) out[i] = binary.charCodeAt(i);
    return out;
  }
  const buf = nodeBuffer();
  return buf ? new Uint8Array(buf.from(b64, 'base64')) : new Uint8Array();
}
