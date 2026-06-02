// A19 + A25: editor enums/classes match the backend snake_case wire values.
//   FactState  == backend FactStatus   (pending|confirmed|disputed|deprecated)
//   SourceTag  == backend SourceTag     (rule|kb|online|inferred)
//   source tag CSS classes == Prototype tokens.css (.src-rule/.src-kb/.src-net/.src-infer)

import { describe, it, expect } from 'vitest';
import { FACT_STATES } from '../src/schema/fact_node';
import { SOURCE_TAGS, sourceTagClass, sourceTagIcon, renderSourceTag } from '../src/source_tags';

describe('fact state enum matches backend FactStatus', () => {
  it('is exactly pending|confirmed|disputed|deprecated', () => {
    expect([...FACT_STATES]).toEqual(['pending', 'confirmed', 'disputed', 'deprecated']);
  });
});

describe('source tags match backend SourceTag wire values + tokens.css classes', () => {
  it('wire values are rule|kb|online|inferred', () => {
    expect([...SOURCE_TAGS]).toEqual(['rule', 'kb', 'online', 'inferred']);
  });

  it('maps each tag to the Prototype .src-* class', () => {
    expect(sourceTagClass('rule')).toBe('src-rule');
    expect(sourceTagClass('kb')).toBe('src-kb');
    expect(sourceTagClass('online')).toBe('src-net'); // 金/网 chip is .src-net
    expect(sourceTagClass('inferred')).toBe('src-infer');
  });

  it('maps each tag to its Lucide icon (Sigma/Database/Globe/Sparkles)', () => {
    expect(sourceTagIcon('rule')).toBe('sigma');
    expect(sourceTagIcon('kb')).toBe('database');
    expect(sourceTagIcon('online')).toBe('globe');
    expect(sourceTagIcon('inferred')).toBe('sparkles');
  });

  it('renders an SVG chip (Lucide line icon, never an Emoji)', () => {
    const html = renderSourceTag('inferred');
    expect(html).toContain('class="src-tag src-infer"');
    expect(html).toContain('<svg');
    expect(html).toContain('lucide-sparkles');
    // No Emoji in the rendered chip.
    expect(/[\u{1F300}-\u{1FAFF}\u{2600}-\u{27BF}\u{1F000}-\u{1F2FF}]/u.test(html)).toBe(false);
  });
});
