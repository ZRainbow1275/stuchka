// E5: pin the inlined Lucide path data so it can never silently drift from the
// Dart side's `flutter_lucide ^0.474.0`.
//
// Two independent guards:
//  1. FROZEN SNAPSHOT — the inlined `ICON_PATHS` strings must equal a frozen,
//     hand-recorded expected map. Editing src/icons/lucide.ts without updating this
//     snapshot fails CI.
//  2. UPSTREAM CROSS-CHECK — the inlined strings must also equal the path markup
//     reconstructed from the pinned `lucide@0.474.0` npm package (a devDependency,
//     never bundled — CSP `connect-src 'none'` is untouched). Bumping the lucide pin
//     without re-vetting the glyphs fails CI.
//
// NOTE: this file contains ZERO literal Emoji (the no-Emoji lint stays green); the
// only non-ASCII below is inside SVG path numeric/letter data, which has no Emoji.

import { describe, it, expect } from 'vitest';
import {
  ICON_PATHS,
  LUCIDE_PINNED_VERSION,
  STUCHKA_ICON_NAMES,
  type StuchkaIconName,
} from '../src/icons/lucide';

// Pinned version expected by both the comment in package.json and flutter_lucide ^0.474.0.
const EXPECTED_VERSION = '0.474.0';

// FROZEN expected path data (recorded verbatim from lucide@0.474.0). If a glyph is
// intentionally changed, update BOTH src/icons/lucide.ts and this snapshot together.
const FROZEN_PATHS: Record<StuchkaIconName, string> = {
  sigma:
    '<path d="M18 7V5a1 1 0 0 0-1-1H6.5a.5.5 0 0 0-.4.8l4.5 6a2 2 0 0 1 0 2.4l-4.5 6a.5.5 0 0 0 .4.8H17a1 1 0 0 0 1-1v-2"/>',
  database:
    '<ellipse cx="12" cy="5" rx="9" ry="3"/><path d="M3 5V19A9 3 0 0 0 21 19V5"/><path d="M3 12A9 3 0 0 0 21 12"/>',
  globe:
    '<circle cx="12" cy="12" r="10"/><path d="M12 2a14.5 14.5 0 0 0 0 20 14.5 14.5 0 0 0 0-20"/><path d="M2 12h20"/>',
  sparkles:
    '<path d="M9.937 15.5A2 2 0 0 0 8.5 14.063l-6.135-1.582a.5.5 0 0 1 0-.962L8.5 9.936A2 2 0 0 0 9.937 8.5l1.582-6.135a.5.5 0 0 1 .963 0L14.063 8.5A2 2 0 0 0 15.5 9.937l6.135 1.581a.5.5 0 0 1 0 .964L15.5 14.063a2 2 0 0 0-1.437 1.437l-1.582 6.135a.5.5 0 0 1-.963 0z"/><path d="M20 3v4"/><path d="M22 5h-4"/><path d="M4 17v2"/><path d="M5 18H3"/>',
  // In 0.474.0 this icon is exported as `triangle-alert`; we keep the legacy key.
  'alert-triangle':
    '<path d="m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3"/><path d="M12 9v4"/><path d="M12 17h.01"/>',
};

// The lucide export node shape: ["svg", attrs, [ [tag, attrObj], ... ]].
type LucideChild = [string, Record<string, string | number>];
type LucideIcon = [string, Record<string, unknown>, LucideChild[]];

/** Reconstruct the inner element markup exactly as `ICON_PATHS` stores it. */
function innerMarkup(icon: LucideIcon): string {
  return icon[2]
    .map(([tag, attrs]) => {
      const a = Object.entries(attrs)
        .map(([k, v]) => `${k}="${v}"`)
        .join(' ');
      return `<${tag} ${a}/>`;
    })
    .join('');
}

describe('lucide pin (E5)', () => {
  it('records the exact pinned Lucide version (matches flutter_lucide ^0.474.0)', () => {
    expect(LUCIDE_PINNED_VERSION).toBe(EXPECTED_VERSION);
  });

  it('the package.json devDependency is pinned exact to that version (not bundled)', async () => {
    const pkg = (await import('../package.json', { with: { type: 'json' } })).default as {
      devDependencies?: Record<string, string>;
      dependencies?: Record<string, string>;
    };
    expect(pkg.devDependencies?.lucide).toBe(EXPECTED_VERSION);
    // lucide must NOT be a runtime dependency (would risk being bundled / CSP scope).
    expect(pkg.dependencies?.lucide).toBeUndefined();
  });

  it('every shipped icon has a frozen snapshot entry (no orphan glyphs)', () => {
    for (const name of STUCHKA_ICON_NAMES) {
      expect(FROZEN_PATHS[name], `missing frozen snapshot for: ${name}`).toBeDefined();
    }
    expect(Object.keys(FROZEN_PATHS).sort()).toEqual([...STUCHKA_ICON_NAMES].sort());
  });

  it('inlined path data matches the frozen snapshot exactly (drift guard)', () => {
    for (const name of STUCHKA_ICON_NAMES) {
      expect(ICON_PATHS[name], `path drift for: ${name}`).toBe(FROZEN_PATHS[name]);
    }
  });

  it('inlined path data matches the pinned lucide@0.474.0 package (upstream cross-check)', async () => {
    // Map our legacy key -> the 0.474.0 module name.
    const moduleFor: Record<StuchkaIconName, string> = {
      sigma: 'sigma',
      database: 'database',
      globe: 'globe',
      sparkles: 'sparkles',
      'alert-triangle': 'triangle-alert',
    };
    for (const name of STUCHKA_ICON_NAMES) {
      const mod = await import(`lucide/dist/esm/icons/${moduleFor[name]}.js`);
      const icon = (mod.default ?? Object.values(mod)[0]) as LucideIcon;
      expect(innerMarkup(icon), `upstream drift for: ${name}`).toBe(ICON_PATHS[name]);
    }
  });
});
