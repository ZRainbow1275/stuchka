// Channel A bridge (in-process WebMessage only). The WebView NEVER opens a
// network connection (CSP `connect-src 'none'`, D1). All cross-process traffic
// goes through the Dart shell via `window.chrome.webview.postMessage`.
//
// This file is the TS mirror of the Dart `BridgeEnvelope` (freezed) contract in
// `02-flutter-webview-bridge.md` §2.3.1.

/** Channel A message types (spec §2.3.2). */
export type BridgeMessageType =
  | 'editor.ready'
  | 'editor.step'
  // Downstream-only (shell -> webview): a remote Yjs update the WebView applies.
  | 'editor.applyRemote'
  // Upstream-only (webview -> shell): a LOCAL Yjs update the WebView produced and
  // surfaces so the shell can persist/forward it (E3). Distinct from
  // `editor.applyRemote` which is downstream-only; never reuse a direction flag.
  | 'editor.localUpdate'
  | 'editor.selection'
  | 'editor.cmd'
  | 'tab.switch'
  | 'tab.opened'
  | 'tab.close'
  | 'ime.compositionStart'
  | 'ime.compositionEnd'
  | 'merge.conflict'
  | 'merge.resolve'
  | 'audit.facetClick'
  | 'crisis.scan'
  | 'tabset.openExternal';

/** The channel A envelope (TS mirror of Dart `BridgeEnvelope`). */
export interface BridgeEnvelope {
  /** In-process correlation id (NOT a D9 persistence key). */
  id: string;
  type: BridgeMessageType;
  /** Monotonic; downgrade is incompatible. Always 1 in R1. */
  schemaVersion: 1;
  /** ISO 8601. */
  ts: string;
  payload: Record<string, unknown>;
}

/** Minimal typing of the WebView2 host bridge surface (channel A). */
interface ChromeWebView {
  postMessage(message: string): void;
  addEventListener(type: 'message', listener: (ev: { data: string }) => void): void;
}

interface ChromeWebViewWindow {
  chrome?: { webview?: ChromeWebView };
}

let correlationCounter = 0;

/** Process-local correlation id (idempotency key); explicitly NOT a UUIDv7 D9 key. */
export function nextCorrelationId(): string {
  correlationCounter += 1;
  return `wv-${Date.now().toString(36)}-${correlationCounter.toString(36)}`;
}

function host(): ChromeWebView | undefined {
  const w = globalThis as unknown as ChromeWebViewWindow;
  return w.chrome?.webview;
}

/**
 * Post a message back to the Dart shell over channel A.
 * Falls back to `globalThis.postMessage` only for test harnesses; production
 * always routes through `window.chrome.webview`.
 */
export function postBack(msg: { type: BridgeMessageType; payload: Record<string, unknown> }): void {
  const env: BridgeEnvelope = {
    id: nextCorrelationId(),
    type: msg.type,
    schemaVersion: 1,
    ts: new Date().toISOString(),
    payload: msg.payload,
  };
  const json = JSON.stringify(env);
  const h = host();
  if (h) {
    h.postMessage(json);
    return;
  }
  // Test / non-WebView environment: emit on globalThis.postMessage if present.
  const gp = (globalThis as unknown as { postMessage?: (m: string) => void }).postMessage;
  if (typeof gp === 'function') gp(json);
}

/** Subscribe to inbound channel A messages from the Dart shell. */
export function onBridgeMessage(handler: (env: BridgeEnvelope) => void): void {
  const h = host();
  if (!h) return;
  h.addEventListener('message', (ev) => {
    let env: BridgeEnvelope;
    try {
      env = JSON.parse(ev.data) as BridgeEnvelope;
    } catch {
      return;
    }
    if (env.schemaVersion !== 1) {
      // Downgrade incompatible (A4): drop silently rather than mis-handle.
      return;
    }
    handler(env);
  });
}
