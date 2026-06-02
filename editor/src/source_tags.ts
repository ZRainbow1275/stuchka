// Four source tags (规则/库/网/推断) → CSS class + Lucide icon + label.
//
// Wire values MUST match the backend `SourceTag` enum (snake_case) in
// `crates/data-model/src/enums.rs`: rule | kb | online | inferred.
// CSS classes are the Prototype `tokens.css` visual authority: .src-rule /
// .src-kb / .src-net / .src-infer. Note the CSS class for `online` is `.src-net`
// (the Prototype names the金/net tag `.src-net`), so the wire→class map is explicit.

import { lucideSvg, type StuchkaIconName } from './icons/lucide';

/** Backend `SourceTag` wire values (snake_case, INV-08 disclosure). */
export type SourceTag = 'rule' | 'kb' | 'online' | 'inferred';

interface SourceTagSpec {
  /** tokens.css class (visual authority). */
  cssClass: 'src-rule' | 'src-kb' | 'src-net' | 'src-infer';
  /** Lucide icon (zero Emoji). */
  icon: StuchkaIconName;
  /** Chinese label. */
  label: string;
}

const SPEC: Record<SourceTag, SourceTagSpec> = {
  rule: { cssClass: 'src-rule', icon: 'sigma', label: '规则' },
  kb: { cssClass: 'src-kb', icon: 'database', label: '库' },
  online: { cssClass: 'src-net', icon: 'globe', label: '网' },
  inferred: { cssClass: 'src-infer', icon: 'sparkles', label: '推断' },
};

export const SOURCE_TAGS: SourceTag[] = ['rule', 'kb', 'online', 'inferred'];

/** The tokens.css class for a source tag. */
export function sourceTagClass(tag: SourceTag): string {
  return SPEC[tag].cssClass;
}

/** The Lucide icon name for a source tag. */
export function sourceTagIcon(tag: SourceTag): StuchkaIconName {
  return SPEC[tag].icon;
}

/** Render a `.src-*` chip with its Lucide icon + label (no Emoji). */
export function renderSourceTag(tag: SourceTag): string {
  const s = SPEC[tag];
  return (
    `<span class="src-tag ${s.cssClass}" data-source="${tag}">` +
    lucideSvg(s.icon, 12) +
    `<span class="src-label">${s.label}</span>` +
    '</span>'
  );
}
