// main.ts — Stučka WebView editor entry. Channel A only (D1).
//
// Boots a Tiptap editor with the frozen 18-extension schema set + Yjs collab
// (BridgeYjsProvider over channel A) + legal-citation trigger + IME relay. The
// ProseMirror Step stream is relayed to the Dart shell via `editor.step`; the
// WebView NEVER posts to the Rust core directly (CSP `connect-src 'none'`).
//
// Built as a single UMD bundle `dist/stuchka-editor.umd.js` consumed by the
// Flutter WebView2 host via `file:///`.

import { Editor } from '@tiptap/core';
import * as Y from 'yjs';

import { buildSchemaExtensions } from './schema/stuchka_schema';
import { AI_SEGMENT_MARK_NAME } from './schema/ai_segment_mark';
import { buildCollabExtensions, BridgeYjsProvider } from './extensions/collab_yjs';
import { StuchkaLegalCitation } from './extensions/legal_citation';
import { onBridgeMessage, postBack } from './bridge';
import { installImeRelay } from './ime_relay';
import { TabRouter } from './tab_router';
import { mountAiMark } from './cards';
import { exportDossier, type CaseMeta, type EditorStateLike } from './export/to_json_dossier';
import { toMarkdown } from './export/to_markdown';
import { buildDraftPreview } from './export/draft_pdf_preview';

export interface BootOptions {
  mount: HTMLElement;
  caseId: string;
  docId: string;
  kbHash: string;
}

export interface StuchkaEditorHandle {
  editor: Editor;
  provider: BridgeYjsProvider;
  exportDossierJson: () => ReturnType<typeof exportDossier>;
  exportMarkdown: () => string;
  draftPreview: () => ReturnType<typeof buildDraftPreview>;
  destroy: () => void;
}

/**
 * The INV-06 audit four-tuple `why` for an `editor.step` (E1). Mirrors the backend
 * `EditorStep.why` triad in `backend/crates/api/src/routes/document.rs` (`why_to_category`):
 *   - `edit`          ordinary typing / structural edits (the default);
 *   - `ai_accept`     the editor applied AI-accepted content (inbound `editor.cmd`);
 *   - `merge_resolve` the editor applied a three-way merge result (inbound `merge.resolve`).
 * The originating intent is carried on the transaction meta key `STUCHKA_WHY_META` so
 * the single transaction hook reads it instead of relying on a dead constant.
 */
export type StuchkaWhy = 'edit' | 'ai_accept' | 'merge_resolve';

/** Transaction meta key the inbound handlers set so the hook can read the originating intent. */
export const STUCHKA_WHY_META = 'stuchkaWhy';

/**
 * Apply AI-accepted content into the doc (REAL path for inbound `editor.cmd` with
 * `action:"ai_accept"`, E1). Inserts the model text at the current selection,
 * wrapped in the `stuchkaAiSegment` mark (so it later contributes a real
 * `ai_generated_segments[]` entry on export, A9), and stamps the transaction meta
 * key so the transaction hook emits the step with `why="ai_accept"`.
 *
 * Returns true when a real edit was dispatched (non-empty text), false otherwise —
 * never a no-op pretend success.
 */
export function applyAiAccept(editor: Editor, payload: Record<string, unknown>): boolean {
  const text = typeof payload.text === 'string' ? payload.text : '';
  if (text.length === 0) return false;
  const modelTag = typeof payload.modelTag === 'string' ? payload.modelTag : '';
  const confidence = typeof payload.confidence === 'number' ? payload.confidence : 0;
  return editor
    .chain()
    .command(({ tr }) => {
      tr.setMeta(STUCHKA_WHY_META, 'ai_accept');
      return true;
    })
    .insertContent({
      type: 'text',
      text,
      marks: [{ type: AI_SEGMENT_MARK_NAME, attrs: { modelTag, confidence } }],
    })
    .run();
}

/**
 * Apply a three-way merge result into the doc (REAL path for inbound `merge.resolve`,
 * E1). Replaces the document body with the resolved text and stamps the transaction
 * meta key so the transaction hook emits the step with `why="merge_resolve"`. The
 * resolved text is the operator-chosen winner the Dart shell forwards (the same
 * `resolvedText` it sends to the backend `POST /document/:id/merge`).
 *
 * Returns true when a real edit was dispatched, false when there is nothing to apply.
 */
export function applyMergeResolve(editor: Editor, payload: Record<string, unknown>): boolean {
  const resolvedText =
    typeof payload.resolvedText === 'string'
      ? payload.resolvedText
      : typeof payload.text === 'string'
        ? payload.text
        : '';
  if (resolvedText.length === 0) return false;
  return editor
    .chain()
    .command(({ tr }) => {
      tr.setMeta(STUCHKA_WHY_META, 'merge_resolve');
      return true;
    })
    .setContent(resolvedText)
    .run();
}

/** Boot the editor and wire the channel-A bridge. */
export function boot(opts: BootOptions): StuchkaEditorHandle {
  const ydoc = new Y.Doc();
  const provider = new BridgeYjsProvider(ydoc);

  const editor = new Editor({
    element: opts.mount,
    extensions: [
      ...buildSchemaExtensions(),
      ...buildCollabExtensions(provider),
      StuchkaLegalCitation,
    ],
  });

  // Monotonic per-document applied-step version (E2). This counts the ProseMirror
  // Steps THIS client has applied across ALL transactions, so it advances by
  // `transaction.steps.length` each transaction and never resets (unlike the old
  // `transaction.docs.length`, which is the intra-transaction snapshot count and
  // resets every transaction). It is directly comparable to the backend persisted
  // step count (`step_no` / `serverVersion`), which likewise increments once per
  // step in `push_step`, so the catch-up contract `ack.serverVersion > batch.version
  // => editor.applyRemote` (A7) holds. Closure-scoped to this doc's lifetime.
  let appliedStepVersion = 0;

  // Relay ProseMirror Steps over channel A (spec §3.3). The shell forwards them
  // to the Rust core via channel B; the WebView never posts directly.
  editor.on('transaction', ({ transaction }) => {
    if (transaction.steps.length === 0) return;
    // Advance the monotonic version by the steps applied in this transaction (E2).
    appliedStepVersion += transaction.steps.length;
    // Originating intent for the INV-06 four-tuple (E1). Inbound ai-accept / merge
    // handlers stamp the meta key; ordinary typing has no meta and defaults to "edit".
    const why = (transaction.getMeta(STUCHKA_WHY_META) as StuchkaWhy | undefined) ?? 'edit';
    postBack({
      type: 'editor.step',
      payload: {
        stepJson: transaction.steps.map((s) => s.toJSON()),
        clientId: ydoc.clientID,
        // Monotonic across transactions for this doc (E2), comparable to serverVersion.
        version: appliedStepVersion,
        // INV-06 audit `why` (edit | ai_accept | merge_resolve), never omitted (E1).
        why,
        caseId: opts.caseId,
        docId: opts.docId,
      },
    });
  });

  // Inbound channel-A messages from the Dart shell. Three REAL paths (做深 / 冗余开发):
  //  - editor.applyRemote : downstream-only — apply a remote Yjs update (A7 catch-up).
  //  - editor.cmd (ai_accept) : the shell asks the editor to insert AI-accepted content;
  //                             the resulting Step carries why="ai_accept".
  //  - merge.resolve : the shell delivers a three-way merge result; the resulting Step
  //                    carries why="merge_resolve".
  onBridgeMessage((env) => {
    if (env.type === 'editor.applyRemote') {
      // Downstream-only (E3): local upstream pushes now use `editor.localUpdate`, so
      // there is no `direction` flag to inspect here anymore.
      const bytes = env.payload.updateBytes as string;
      if (bytes) provider.applyRemote(bytes);
      return;
    }
    if (env.type === 'editor.cmd') {
      const action = env.payload.action as string | undefined;
      if (action === 'ai_accept') {
        applyAiAccept(editor, env.payload);
      }
      return;
    }
    if (env.type === 'merge.resolve') {
      applyMergeResolve(editor, env.payload);
      return;
    }
  });

  const detachIme = installImeRelay(opts.mount);
  const tabRouter = new TabRouter();
  tabRouter.start();

  // Editor-internal preview-level [AI] mark (A26).
  mountAiMark(opts.mount.ownerDocument?.body ?? opts.mount);

  // Announce readiness with the bootstrap Yjs state (channel A).
  postBack({
    type: 'editor.ready',
    payload: { caseId: opts.caseId, docId: opts.docId, yjsBootstrapBytes: provider.encodeState() },
  });

  const caseMeta: CaseMeta = { id: opts.caseId, docId: opts.docId, kbHash: opts.kbHash };
  const stateLike = (): EditorStateLike => editor.state as unknown as EditorStateLike;

  return {
    editor,
    provider,
    exportDossierJson: () => exportDossier(stateLike(), caseMeta),
    exportMarkdown: () => toMarkdown(editor.state.doc as unknown as EditorStateLike['doc']),
    draftPreview: () => buildDraftPreview(),
    destroy: () => {
      detachIme();
      provider.destroy();
      editor.destroy();
      ydoc.destroy();
    },
  };
}

// Re-export the public surface so the UMD global `StuchkaEditor` exposes them.
export { exportDossier } from './export/to_json_dossier';
export { toMarkdown } from './export/to_markdown';
export { buildDraftPreview, DRAFT_MARK } from './export/draft_pdf_preview';
export { REQUIRED_EXTENSION_NAMES, EXCLUDED_EXTENSION_NAMES, buildSchemaExtensions } from './schema/stuchka_schema';
export { renderSourceTag, SOURCE_TAGS } from './source_tags';
export { renderThreeSegmentCard, mountAiMark } from './cards';
export { ALL_TEMPLATES, validateSlots, INV10_SETTLEMENT_DISCLAIMER } from './templates';
export { EMOJI_RE, stripEmoji } from './extensions/forbid_emoji';
export { ZW_MARK, stripZeroWidth } from './extensions/inv02_watermark';
