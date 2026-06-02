// E1 + E2 + E3 wire contracts for the WebView editor's channel-A traffic.
//
// Drives the REAL boot() path through a stand-in WebView2 host injected onto
// globalThis.chrome.webview, so the genuine src/bridge.ts postBack / onBridgeMessage
// code runs (no mock of the bridge itself). Inbound messages are delivered exactly
// as the Dart shell would (a JSON BridgeEnvelope on the host 'message' listener).
//
// Asserts:
//   E1 — editor.step carries `why` ("edit" by default; "ai_accept" on an inbound
//        editor.cmd ai-accept; "merge_resolve" on an inbound merge.resolve).
//   E2 — the `version` field is monotonic across two separate transactions.
//   E3 — local Yjs upstream uses the distinct `editor.localUpdate` type, NEVER
//        `editor.applyRemote` with a direction flag.
//
// ZERO literal Emoji in this file.

import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { boot, type StuchkaEditorHandle } from '../src/main';

interface CapturedHost {
  outbound: Array<{ type: string; payload: Record<string, unknown> }>;
  /** Deliver an inbound envelope to the registered listener(s), as the shell would. */
  deliver(env: { type: string; payload: Record<string, unknown> }): void;
}

let host: CapturedHost;
let handle: StuchkaEditorHandle | undefined;
let mount: HTMLElement;

beforeEach(() => {
  const listeners: Array<(ev: { data: string }) => void> = [];
  const outbound: CapturedHost['outbound'] = [];

  // A genuine WebView2-shaped host surface (channel A). The bridge reads
  // globalThis.chrome.webview; this is the same object the real host exposes.
  (globalThis as unknown as { chrome?: unknown }).chrome = {
    webview: {
      postMessage(message: string) {
        const env = JSON.parse(message) as { type: string; payload: Record<string, unknown> };
        outbound.push({ type: env.type, payload: env.payload });
      },
      addEventListener(_type: 'message', listener: (ev: { data: string }) => void) {
        listeners.push(listener);
      },
    },
  };

  host = {
    outbound,
    deliver(env) {
      const wire = JSON.stringify({
        id: 'shell-1',
        type: env.type,
        schemaVersion: 1,
        ts: new Date().toISOString(),
        payload: env.payload,
      });
      for (const l of listeners) l({ data: wire });
    },
  };

  mount = document.createElement('div');
  document.body.appendChild(mount);
  handle = boot({ mount, caseId: 'case-1', docId: 'doc-1', kbHash: 'kb-1' });
});

afterEach(() => {
  handle?.destroy();
  handle = undefined;
  mount.remove();
  delete (globalThis as unknown as { chrome?: unknown }).chrome;
});

function steps() {
  return host.outbound.filter((m) => m.type === 'editor.step');
}

describe('editor.step why field (E1)', () => {
  it('defaults to "edit" for ordinary typing', () => {
    const ed = handle!.editor;
    ed.commands.insertContent('普通录入文本');
    const s = steps();
    expect(s.length).toBeGreaterThan(0);
    for (const m of s) {
      expect(m.payload.why).toBe('edit');
    }
  });

  it('is "ai_accept" when the shell sends an editor.cmd ai-accept', () => {
    host.outbound.length = 0;
    host.deliver({
      type: 'editor.cmd',
      payload: { action: 'ai_accept', text: 'AI 草拟段落', modelTag: 'deepseek-v3.1-2026Q1', confidence: 0.8 },
    });
    const s = steps();
    expect(s.length).toBeGreaterThan(0);
    // Every step produced by the ai-accept transaction is tagged ai_accept.
    expect(s.some((m) => m.payload.why === 'ai_accept')).toBe(true);
    expect(s.every((m) => m.payload.why === 'ai_accept')).toBe(true);
  });

  it('is "merge_resolve" when the shell sends a merge.resolve', () => {
    host.outbound.length = 0;
    host.deliver({
      type: 'merge.resolve',
      payload: { conflictId: 'c-1', winner: 'mine', resolvedText: '合并后的最终正文' },
    });
    const s = steps();
    expect(s.length).toBeGreaterThan(0);
    expect(s.some((m) => m.payload.why === 'merge_resolve')).toBe(true);
    expect(s.every((m) => m.payload.why === 'merge_resolve')).toBe(true);
  });

  it('only ever emits a why in the backend triad (edit|ai_accept|merge_resolve)', () => {
    handle!.editor.commands.insertContent('文本一');
    host.deliver({ type: 'editor.cmd', payload: { action: 'ai_accept', text: '段二' } });
    host.deliver({ type: 'merge.resolve', payload: { resolvedText: '段三' } });
    const allowed = new Set(['edit', 'ai_accept', 'merge_resolve']);
    for (const m of steps()) {
      expect(allowed.has(m.payload.why as string)).toBe(true);
    }
  });
});

describe('editor.step version field (E2)', () => {
  it('is monotonic across two separate transactions', () => {
    const ed = handle!.editor;
    ed.commands.insertContent('第一段');
    const afterFirst = steps();
    const v1 = afterFirst[afterFirst.length - 1].payload.version as number;

    ed.commands.insertContent('第二段');
    const afterSecond = steps();
    const v2 = afterSecond[afterSecond.length - 1].payload.version as number;

    expect(typeof v1).toBe('number');
    expect(typeof v2).toBe('number');
    // Strictly increasing across transactions (NOT reset per transaction like the
    // old transaction.docs.length). Comparable to the backend serverVersion.
    expect(v2).toBeGreaterThan(v1);
  });

  it('advances by the number of steps applied (never resets to a small count)', () => {
    const ed = handle!.editor;
    ed.commands.insertContent('A');
    ed.commands.insertContent('B');
    ed.commands.insertContent('C');
    const versions = steps().map((m) => m.payload.version as number);
    // Every successive emitted version is >= the previous (monotone non-decreasing,
    // strictly increasing across step-bearing transactions).
    for (let i = 1; i < versions.length; i += 1) {
      expect(versions[i]).toBeGreaterThan(versions[i - 1]);
    }
  });
});

describe('local Yjs upstream uses editor.localUpdate (E3)', () => {
  it('emits editor.localUpdate (never editor.applyRemote) for a local edit', () => {
    handle!.editor.commands.insertContent('触发本地 Yjs 更新');
    const localUpdates = host.outbound.filter((m) => m.type === 'editor.localUpdate');
    expect(localUpdates.length).toBeGreaterThan(0);
    // The upstream payload carries updateBytes and NO direction flag.
    for (const m of localUpdates) {
      expect(typeof m.payload.updateBytes).toBe('string');
      expect((m.payload.updateBytes as string).length).toBeGreaterThan(0);
      expect('direction' in m.payload).toBe(false);
    }
    // applyRemote is downstream-only; the WebView must never emit it upstream.
    const upstreamApplyRemote = host.outbound.filter((m) => m.type === 'editor.applyRemote');
    expect(upstreamApplyRemote.length).toBe(0);
  });
});
