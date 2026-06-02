// Lucide line icons as static inline SVG strings (NO network, NO Emoji).
//
// PINNED to Lucide v0.474.0 (E5) — the EXACT version mirrored by the Dart side's
// `flutter_lucide ^0.474.0`. The path data below was taken verbatim from
// `lucide@0.474.0` (npm), pinned as an exact devDependency in package.json
// (`"lucide": "0.474.0"`, dev-only — NOT bundled, so CSP `connect-src 'none'` is
// untouched). `test/lucide_pin.test.ts` asserts these strings match a frozen
// snapshot extracted from that version, so any drift fails CI.
// NOTE: in 0.474.0 the icon formerly named `alert-triangle` is exported as
// `triangle-alert`; the glyph (and its path data) is identical. We keep the
// `alert-triangle` key for backward compatibility with existing call sites.
//
// These are the icons mapped in `03-editor-workbench.md` §3.6 (four source tags)
// plus AlertTriangle (the Prototype Emoji replacement, brief I6). Kept as raw
// strings so the bundle stays self-contained under CSP `connect-src 'none'` —
// never fetched.
//
// Icon set is intentionally tiny and frozen; adding new glyphs means adding the
// official Lucide path here, never a Material/Cupertino/Emoji fallback (A15).

export type StuchkaIconName =
  | 'sigma'
  | 'database'
  | 'globe'
  | 'sparkles'
  | 'alert-triangle';

/**
 * The EXACT Lucide version the inlined `PATHS` below were taken from (E5). Pinned as
 * an exact devDependency (`"lucide": "0.474.0"`) and mirrored by the Dart side's
 * `flutter_lucide ^0.474.0`. `test/lucide_pin.test.ts` cross-checks both.
 */
export const LUCIDE_PINNED_VERSION = '0.474.0';

/** The inlined path data, exposed (read-only) so the pin test can snapshot it (E5). */
export const ICON_PATHS: Readonly<Record<StuchkaIconName, string>> = {
  // Sigma — rule engine (deterministic) source tag.
  sigma: '<path d="M18 7V5a1 1 0 0 0-1-1H6.5a.5.5 0 0 0-.4.8l4.5 6a2 2 0 0 1 0 2.4l-4.5 6a.5.5 0 0 0 .4.8H17a1 1 0 0 0 1-1v-2"/>',
  // Database — knowledge base / similar cases source tag.
  database: '<ellipse cx="12" cy="5" rx="9" ry="3"/><path d="M3 5V19A9 3 0 0 0 21 19V5"/><path d="M3 12A9 3 0 0 0 21 12"/>',
  // Globe — online retrieval source tag.
  globe: '<circle cx="12" cy="12" r="10"/><path d="M12 2a14.5 14.5 0 0 0 0 20 14.5 14.5 0 0 0 0-20"/><path d="M2 12h20"/>',
  // Sparkles — model inference (needs human confirmation) source tag.
  sparkles: '<path d="M9.937 15.5A2 2 0 0 0 8.5 14.063l-6.135-1.582a.5.5 0 0 1 0-.962L8.5 9.936A2 2 0 0 0 9.937 8.5l1.582-6.135a.5.5 0 0 1 .963 0L14.063 8.5A2 2 0 0 0 15.5 9.937l6.135 1.581a.5.5 0 0 1 0 .964L15.5 14.063a2 2 0 0 0-1.437 1.437l-1.582 6.135a.5.5 0 0 1-.963 0z"/><path d="M20 3v4"/><path d="M22 5h-4"/><path d="M4 17v2"/><path d="M5 18H3"/>',
  // AlertTriangle — replaces Prototype warning Emoji (brief I6).
  'alert-triangle': '<path d="m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3"/><path d="M12 9v4"/><path d="M12 17h.01"/>',
};

/** Render a Lucide icon as an inline SVG string (24x24 line icon, currentColor stroke). */
export function lucideSvg(name: StuchkaIconName, size = 16): string {
  return (
    `<svg xmlns="http://www.w3.org/2000/svg" width="${size}" height="${size}" ` +
    'viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" ' +
    `stroke-linecap="round" stroke-linejoin="round" class="lucide lucide-${name}" aria-hidden="true">` +
    ICON_PATHS[name] +
    '</svg>'
  );
}

/** All icon names this build ships (frozen set; used by lint / assertions). */
export const STUCHKA_ICON_NAMES: StuchkaIconName[] = [
  'sigma',
  'database',
  'globe',
  'sparkles',
  'alert-triangle',
];
