# Stučka WebView Editor (`Stučka/editor/`)

Tiptap + Yjs editor that runs **inside the Windows WebView2** host. Channel A only
(in-process `window.chrome.webview.postMessage`); the WebView never opens a network
connection (CSP `connect-src 'none'`, D1). All cross-process traffic goes through the
Flutter Dart shell, which talks to the Rust core over channel B (Bearer HTTP/WS).

## Build

```sh
cd "Stučka/editor"
npm install
npm run build   # → dist/stuchka-editor.umd.js  (single self-contained UMD bundle)
npm test        # vitest
```

The UMD bundle is copied to `assets/web/editor/` by the Flutter side
(`tools/copy_editor_to_assets.dart`) and loaded via `file:///`.

## What this package implements

- **Tiptap (not bare ProseMirror)** with **exactly 18 extensions** (StarterKit subset
  + 7 self-built). Excludes Strike/Code/CodeBlock/HorizontalRule/Mention. In collab mode
  the built-in History is disabled — undo/redo is handled by y-prosemirror via the
  Collaboration extension.
- **Channel A bridge** (`src/bridge.ts`) — `BridgeEnvelope` TS mirror, `postBack` /
  `onBridgeMessage`. No `fetch`/`XHR`/`WebSocket`.
- **Yjs collab** (`src/extensions/collab_yjs.ts`) — `BridgeYjsProvider` relays Yjs
  updates over channel A; LAN multi-peer sync is the Dart side's job.
- **Three-segment cards** (`src/cards.ts`) — `.conclusion-card` / `.evidence-list` /
  `.expand-drawer` reusing the Prototype `tokens.css` (visual authority).
- **Four source tags** (`src/source_tags.ts`) — `rule|kb|online|inferred` wire values
  (matching backend `SourceTag`) → `.src-rule/.src-kb/.src-net/.src-infer` +
  Lucide Sigma/Database/Globe/Sparkles SVGs.
- **AI mark** (`mountAiMark`) — fixed bottom-right `[AI]` preview-level marker.
- **GB45438 preview marker** (`src/extensions/inv02_watermark.ts`) — zero-width sequence
  in the editor + dossier JSON ONLY; never the final PDF watermark (D5). Stripped from
  Markdown export.
- **6 hard templates** (`src/templates/`) — `tpl_settlement.json` carries the
  non-deletable INV-10 disclaimer slot.
- **Exports** (`src/export/`) — Markdown (`[fact:ID]` / `[law:URN]` nodes), dossier JSON
  (`ai_generated_segments[]`, one entry per AI mark), draft PDF preview descriptor.
- **Zero Emoji** (`src/extensions/forbid_emoji.ts`) — block at input, strip on paste.
  Lucide line icons only (`src/icons/lucide.ts`).

## Boundaries (do not cross)

- Frontend writes NO final PDF / final watermark (D5: `crates/document` is authority).
- Frontend builds NO audit table (D3: `crates/audit` / `audit.sqlite`).
- Frontend creates NO LawRef short codes (D8: URN authority is data/02).
