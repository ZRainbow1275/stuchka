// A18: 6 hard templates exist + slot validators; settlement embeds the
// non-deletable INV-10 disclaimer (GB-10). A19 is covered in fact_state.test.ts.

import { describe, it, expect } from 'vitest';
import {
  ALL_TEMPLATES,
  TEMPLATES,
  TEMPLATE_BACKEND_IDS,
  validateSlots,
  INV10_SETTLEMENT_DISCLAIMER,
} from '../src/templates';

describe('six hard templates', () => {
  it('ships exactly the 6 backend template ids', () => {
    expect(ALL_TEMPLATES).toHaveLength(6);
    expect([...TEMPLATE_BACKEND_IDS]).toEqual([
      'arb_application',
      'mediation',
      'inspection',
      'settlement',
      'lawsuit',
      'enforcement',
    ]);
  });

  it('every template has slots with validators', () => {
    for (const tpl of ALL_TEMPLATES) {
      expect(tpl.slots.length).toBeGreaterThan(0);
      for (const slot of tpl.slots) {
        expect(slot.validator).toBeTruthy();
      }
    }
  });

  it('settlement carries the locked, non-deletable INV-10 disclaimer slot', () => {
    const settlement = TEMPLATES.settlement;
    const inv10 = settlement.slots.find((s) => s.validator === 'inv10_disclaimer');
    expect(inv10).toBeDefined();
    expect(inv10!.locked).toBe(true);
    expect(inv10!.deletable).toBe(false);
    expect(inv10!.text).toBe(INV10_SETTLEMENT_DISCLAIMER);
    expect(inv10!.text).toContain('不可删除');
  });

  it('settlement slot validation fails if INV-10 disclaimer text is altered', () => {
    const settlement = structuredClone(TEMPLATES.settlement);
    const inv10 = settlement.slots.find((s) => s.validator === 'inv10_disclaimer')!;
    inv10.text = '被篡改的免责声明';
    const result = validateSlots(settlement, {
      'applicant.name': '张三',
      'applicant.id_no': '440000199001011234',
      'respondent.name': '某厂',
      claims: ['c'],
      facts: ['f'],
      law_refs: ['law:x/v1/§1'],
    });
    expect(result.inv10Ok).toBe(false);
    expect(result.ok).toBe(false);
  });

  it('arbitration validation passes with all required slots filled', () => {
    const result = validateSlots(TEMPLATES.arb_application, {
      'applicant.name': '张三',
      'applicant.id_no': '440000199001011234',
      'respondent.name': '某厂',
      claims: ['c'],
      facts: ['f'],
      evidence_index: ['e'],
      law_refs: ['law:中华人民共和国劳动合同法/v2012-12-28/§87'],
    });
    expect(result.ok).toBe(true);
    expect(result.missing).toHaveLength(0);
  });
});
