// draft_pdf_preview.ts — editor-internal PDF DRAFT preview only. Spec §3.9.
//
// D5 BOUNDARY: this renders a "草稿 · 非最终件" preview using the WebView2
// `PrintToPdfAsync` seam. It MUST NOT write the GB45438 four-layer watermark and
// MUST carry the explicit draft mark (A11). The final PDF is produced solely by
// the Rust backend `crates/document`. The frontend never claims four-layer
// completeness here (A12).

/** The explicit, mandatory draft marker stamped on every preview. */
export const DRAFT_MARK = '草稿 · 非最终件';

export interface DraftPreviewResult {
  /** Always true: this is a draft preview, never an export. */
  isDraft: true;
  /** The mandatory on-document draft mark. */
  draftMark: string;
  /** Whether the four-layer watermark was written (ALWAYS false here, D5). */
  watermarkWritten: false;
}

/**
 * Build the draft-preview descriptor. The actual `PrintToPdfAsync` invocation is
 * driven by the Dart shell over channel A (`editor.cmd`); this function only
 * asserts the draft contract and returns the descriptor the shell stamps.
 */
export function buildDraftPreview(): DraftPreviewResult {
  return {
    isDraft: true,
    draftMark: DRAFT_MARK,
    watermarkWritten: false,
  };
}
