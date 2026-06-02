// templates/index.ts — the 6 hard templates + slot validation. Spec §3.7.
//
// `tpl_settlement` MUST embed the non-deletable INV-10 disclaimer slot (GB-10);
// the loader asserts it on access. The `template_id` field matches the backend
// `TemplateId` serde wire values (arb_application | mediation | inspection |
// settlement | lawsuit | enforcement).

import arbitration from './tpl_arbitration.json';
import mediation from './tpl_mediation.json';
import inspection from './tpl_inspection.json';
import settlement from './tpl_settlement.json';
import lawsuit from './tpl_lawsuit.json';
import enforcement from './tpl_enforcement.json';

export interface TemplateSlot {
  key: string;
  required: boolean;
  validator: string;
  deletable?: boolean;
  locked?: boolean;
  text?: string;
}

export interface TemplateDoc {
  id: string;
  template_id: string;
  title: string;
  version: string;
  applicable_subtypes: string[];
  applicable_regions: string[];
  slots: TemplateSlot[];
  body_blocks: { type: string; level?: number; text?: string; slot?: string; locked?: boolean }[];
  gb45438_request: {
    header: string;
    request_backend_watermark: boolean;
    ai_segment_tracking: boolean;
  };
}

/** The backend `TemplateId` wire value → frontend template id. */
export const TEMPLATE_BACKEND_IDS = [
  'arb_application',
  'mediation',
  'inspection',
  'settlement',
  'lawsuit',
  'enforcement',
] as const;

export const TEMPLATES: Record<string, TemplateDoc> = {
  arb_application: arbitration as TemplateDoc,
  mediation: mediation as TemplateDoc,
  inspection: inspection as TemplateDoc,
  settlement: settlement as TemplateDoc,
  lawsuit: lawsuit as TemplateDoc,
  enforcement: enforcement as TemplateDoc,
};

/** All six templates in canonical order. */
export const ALL_TEMPLATES: TemplateDoc[] = TEMPLATE_BACKEND_IDS.map((id) => TEMPLATES[id]);

/** The canonical (non-deletable) INV-10 settlement disclaimer text — mirrors the backend const. */
export const INV10_SETTLEMENT_DISCLAIMER =
  '【重要风险提示（不可删除）】本和解协议一经签署即对双方产生法律约束力。' +
  '若本协议约定的金额低于法定应得数额的 80%，您可能因此放弃部分法定权益；' +
  '签署前请务必确认：本协议系您本人在充分知悉法律后果后自愿作出的决定。' +
  '如有疑问，请咨询执业律师或当地法律援助机构（全国法援热线：12348）。';

/** Slot validation result. */
export interface SlotCheck {
  ok: boolean;
  missing: string[];
  inv10Ok: boolean;
}

/** Validate a template's slots against provided values; settlement also checks INV-10 (GB-10). */
export function validateSlots(template: TemplateDoc, values: Record<string, unknown>): SlotCheck {
  const missing: string[] = [];
  for (const slot of template.slots) {
    if (!slot.required) continue;
    if (slot.validator === 'inv10_disclaimer') continue; // checked separately
    const v = values[slot.key];
    const empty = v == null || v === '' || (Array.isArray(v) && v.length === 0);
    if (empty) missing.push(slot.key);
  }
  // INV-10: settlement must carry the locked, non-deletable disclaimer slot.
  const inv10Slot = template.slots.find((s) => s.validator === 'inv10_disclaimer');
  const inv10Ok =
    template.template_id !== 'settlement' ||
    (inv10Slot != null &&
      inv10Slot.locked === true &&
      inv10Slot.deletable === false &&
      (inv10Slot.text ?? '').includes('不可删除') &&
      inv10Slot.text === INV10_SETTLEMENT_DISCLAIMER);
  return { ok: missing.length === 0 && inv10Ok, missing, inv10Ok };
}
