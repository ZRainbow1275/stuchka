// A13: forbid_emoji — detect at input, strip on paste.
//
// NOTE: Emoji fixtures are constructed from code points via String.fromCodePoint
// so this file contains ZERO literal Emoji glyphs (keeps the CI no-Emoji lint
// green while still exercising the stripper, brief §1.8 / A14).

import { describe, it, expect } from 'vitest';
import { EMOJI_RE, stripEmoji } from '../src/extensions/forbid_emoji';

// Code-point fixtures (no literal Emoji bytes in this file).
const BRIEFCASE = String.fromCodePoint(0x1f4bc); // 1F300-1FAFF block
const TELEPHONE = String.fromCodePoint(0x260e); // 2600-27BF block
const MAHJONG = String.fromCodePoint(0x1f004); // 1F000-1F2FF block
const MONEYBAG = String.fromCodePoint(0x1f4b0);
const THUMBSUP = String.fromCodePoint(0x1f44d);

describe('forbid emoji', () => {
  it('detects emoji code points across the named Unicode blocks', () => {
    expect(EMOJI_RE.test(BRIEFCASE)).toBe(true);
    expect(EMOJI_RE.test(TELEPHONE)).toBe(true);
    expect(EMOJI_RE.test(MAHJONG)).toBe(true);
  });

  it('does not flag normal Chinese legal text or full-width punctuation', () => {
    expect(EMOJI_RE.test('劳动合同法第八十七条')).toBe(false);
    expect(EMOJI_RE.test('请求事项：，。；§¶')).toBe(false);
  });

  it('strips emoji while preserving surrounding text', () => {
    expect(stripEmoji(`赔偿${MONEYBAG}金额8200元`)).toBe('赔偿金额8200元');
    expect(stripEmoji(`好的${THUMBSUP}${THUMBSUP} 继续`)).toBe('好的 继续');
  });

  it('leaves clean text unchanged', () => {
    const clean = '裁决被申请人支付欠付工资 8,200 元；';
    expect(stripEmoji(clean)).toBe(clean);
  });
});
