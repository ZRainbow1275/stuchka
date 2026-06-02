// tab_router.ts — single-WebView internal Tab router (C-C-11). Spec §2.6.
//
// All documents / merge views switch inside one WebView instance via Tab switch.
// R1 cap: ≤ 8 simultaneous tabs; over the cap the LRU tab is evicted and its
// Tiptap instance destroyed (Yjs Doc released). Channel A only.

import { onBridgeMessage, postBack } from './bridge';

export type TabId = string;

export interface TabState {
  id: TabId;
  docId: string;
  mode: 'editor' | 'merge';
  /** Last-activation timestamp for LRU eviction. */
  lastActive: number;
}

/** R1 simultaneous tab cap (§2.6). */
export const MAX_TABS = 8;

export class TabRouter {
  private readonly tabs = new Map<TabId, TabState>();
  private activeTab: TabId | null = null;

  /** Optional disposer invoked when a tab is evicted/closed (destroy Tiptap + Yjs). */
  constructor(private readonly onDispose?: (tab: TabState) => void) {}

  start(): void {
    onBridgeMessage((env) => {
      if (env.type === 'tab.switch') {
        this.activate(env.payload.tabId as TabId, (env.payload.role as string) ?? 'mine');
      } else if (env.type === 'tab.close') {
        this.close(env.payload.tabId as TabId);
      }
    });
  }

  open(id: TabId, docId: string, mode: 'editor' | 'merge'): void {
    this.tabs.set(id, { id, docId, mode, lastActive: Date.now() });
    this.evictIfNeeded();
    postBack({ type: 'tab.opened', payload: { tabId: id, docId } });
  }

  activate(tabId: TabId, role: string): void {
    const tab = this.tabs.get(tabId);
    if (tab) tab.lastActive = Date.now();
    this.activeTab = tabId;
    // Toggle visibility of `.tab-panel` elements when running in a real DOM.
    const docAny = (globalThis as unknown as { document?: Document }).document;
    if (docAny) {
      docAny.querySelectorAll('.tab-panel').forEach((el) => {
        (el as HTMLElement).style.display = el.id === `tab-${tabId}` ? 'block' : 'none';
      });
    }
    postBack({ type: 'tab.opened', payload: { tabId, role } });
  }

  close(tabId: TabId): void {
    const tab = this.tabs.get(tabId);
    if (!tab) return;
    this.tabs.delete(tabId);
    this.onDispose?.(tab);
    if (this.activeTab === tabId) this.activeTab = null;
  }

  /** LRU eviction once the cap is exceeded. */
  private evictIfNeeded(): void {
    while (this.tabs.size > MAX_TABS) {
      let oldest: TabState | null = null;
      for (const t of this.tabs.values()) {
        if (t.id === this.activeTab) continue;
        if (!oldest || t.lastActive < oldest.lastActive) oldest = t;
      }
      if (!oldest) break;
      this.close(oldest.id);
    }
  }

  get size(): number {
    return this.tabs.size;
  }

  get active(): TabId | null {
    return this.activeTab;
  }
}
