// three_way_view.ts — three-column merge UI (祖先 / 我的 / 对方). Spec §3.5.
//
// Renders only the pending conflict blocks; on a choice it relays `merge.resolve`
// over channel A. The buttons use Lucide icons + Chinese labels (zero Emoji).

import { postBack } from '../bridge';
import { lucideSvg } from '../icons/lucide';
import type { ConflictBlock, MergeChoice } from './conflict_resolver';
import type { PMNodeLike } from '../export/to_json_dossier';

function nodeText(node: PMNodeLike): string {
  if (node.text !== undefined) return node.text;
  return (node.content ?? []).map(nodeText).join('');
}

export class ThreeWayView {
  constructor(private readonly container: HTMLElement) {}

  render(blocks: ConflictBlock[]): void {
    this.container.innerHTML = '';
    blocks
      .filter((b) => b.status === 'pending')
      .forEach((b) => {
        const row = document.createElement('div');
        row.className = 'merge-row';
        row.setAttribute('data-block-id', b.blockId);
        row.appendChild(this.col('ancestor', '共同祖先', b.ancestor));
        row.appendChild(this.col('mine', '我的版本', b.mine));
        row.appendChild(this.col('theirs', '对方版本', b.theirs));
        row.appendChild(this.actions(b));
        this.container.appendChild(row);
      });
  }

  private col(kind: string, label: string, node: PMNodeLike): HTMLElement {
    const el = document.createElement('div');
    el.className = `col ${kind}`;
    const head = document.createElement('div');
    head.className = 'merge-col-head';
    head.textContent = label;
    const body = document.createElement('div');
    body.className = 'merge-body';
    body.textContent = nodeText(node);
    el.appendChild(head);
    el.appendChild(body);
    return el;
  }

  private actions(b: ConflictBlock): HTMLElement {
    const el = document.createElement('div');
    el.className = 'col actions';
    el.appendChild(this.button('采我', () => this.resolve(b, 'mine')));
    el.appendChild(this.button('采对方', () => this.resolve(b, 'theirs')));
    el.appendChild(this.button('手动合', () => this.resolve(b, 'manual'), 'alert-triangle'));
    return el;
  }

  private button(label: string, onClick: () => void, icon?: 'alert-triangle'): HTMLButtonElement {
    const btn = document.createElement('button');
    if (icon) btn.innerHTML = lucideSvg(icon, 14);
    btn.appendChild(document.createTextNode(label));
    btn.addEventListener('click', onClick);
    return btn;
  }

  private resolve(b: ConflictBlock, choice: MergeChoice): void {
    postBack({ type: 'merge.resolve', payload: { blockId: b.blockId, choice } });
  }
}
