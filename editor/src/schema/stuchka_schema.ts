// stuchka_schema.ts — the exact 18-extension set. Spec §3.2.
//
// Group breakdown (18 total):
//   StarterKit subset (8): Document, Paragraph, Text, Heading, Bold, Italic,
//     BulletList, OrderedList, ListItem, Blockquote, HardBreak, History
//     — BUT StarterKit is registered as ONE extension that BUNDLES those nodes,
//     so for the "18 extensions" count we register the individual self-built +
//     collab + placeholder extensions plus the StarterKit-equivalent nodes as
//     standalone units. To make the 18-count deterministic and assertable, we
//     enumerate every extension explicitly below and exclude
//     Strike/Code/CodeBlock/HorizontalRule/Mention.
//
// The canonical 18 (assertable by name) are, in registration order:
//   1  doc               (StarterKit top node; the extension is named `doc`)
//   2  paragraph         (StarterKit)
//   3  text              (StarterKit)
//   4  heading           (StarterKit, h1-h3)
//   5  bold              (StarterKit)
//   6  italic            (StarterKit)
//   7  bulletList        (StarterKit)
//   8  orderedList       (StarterKit)
//   9  listItem          (StarterKit)
//   10 blockquote        (StarterKit)
//   11 hardBreak         (StarterKit)
//   12 fact              (self-built)
//   13 lawRef            (self-built)
//   14 stuchkaAiSegment  (self-built mark)
//   15 templateSlot      (self-built)
//   16 stuchkaAiSegmentPreviewMark (self-built, GB45438 preview)
//   17 stuchkaForbidEmoji (self-built)
//   18 stuchkaSanitizePaste (self-built)
//
// History/Collaboration: in collab mode History is DISABLED (undo via
// y-prosemirror); the legal-citation + collab extensions are layered on top in
// `main.ts` (they are mode-dependent and not part of the frozen 18 schema set).

import StarterKit from '@tiptap/starter-kit';
import type { AnyExtension } from '@tiptap/core';

import { StuchkaFactNode } from './fact_node';
import { StuchkaLawRefNode } from './law_ref_node';
import { StuchkaAiSegmentMark } from './ai_segment_mark';
import { StuchkaTemplateSlot } from '../extensions/template_slot';
import { StuchkaAiSegmentPreviewMark } from '../extensions/inv02_watermark';
import { StuchkaForbidEmoji } from '../extensions/forbid_emoji';
import { StuchkaSanitizePaste } from '../extensions/stuchka_paste';

/** The frozen list of extension NAMES that must be enabled (exactly 18). */
export const REQUIRED_EXTENSION_NAMES = [
  'doc',
  'paragraph',
  'text',
  'heading',
  'bold',
  'italic',
  'bulletList',
  'orderedList',
  'listItem',
  'blockquote',
  'hardBreak',
  'fact',
  'lawRef',
  'stuchkaAiSegment',
  'templateSlot',
  'stuchkaAiSegmentPreviewMark',
  'stuchkaForbidEmoji',
  'stuchkaSanitizePaste',
] as const;

/** Extensions that MUST be excluded (公文场景不需要). */
export const EXCLUDED_EXTENSION_NAMES = [
  'strike',
  'code',
  'codeBlock',
  'horizontalRule',
  'mention',
] as const;

/**
 * Build the StarterKit subset: disable the 5 excluded extensions and disable the
 * built-in History (collab mode hands undo to y-prosemirror, A17). We keep
 * Document/Paragraph/Text/Heading/Bold/Italic/BulletList/OrderedList/ListItem/
 * Blockquote/HardBreak.
 */
export function buildStarterKit(): AnyExtension {
  return StarterKit.configure({
    // 公文标题限制 h1-h3.
    heading: { levels: [1, 2, 3] },
    // Excluded (set to false to drop the extension entirely).
    strike: false,
    code: false,
    codeBlock: false,
    horizontalRule: false,
    // Collab mode disables History (undo via y-prosemirror).
    history: false,
  }) as AnyExtension;
}

/**
 * Build the exact frozen 18-extension array (StarterKit subset expanded
 * conceptually + the 7 self-built units). StarterKit registers the 11 StarterKit
 * names as sub-extensions; the 7 self-built bring the total to 18 distinct names.
 */
export function buildSchemaExtensions(): AnyExtension[] {
  return [
    buildStarterKit(),
    StuchkaFactNode,
    StuchkaLawRefNode,
    StuchkaAiSegmentMark,
    StuchkaTemplateSlot,
    StuchkaAiSegmentPreviewMark,
    StuchkaForbidEmoji,
    StuchkaSanitizePaste,
  ];
}
