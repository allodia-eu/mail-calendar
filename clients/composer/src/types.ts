// The compose document, typed to mirror `crates/mailcal-composer`'s serde shapes.
//
// This file is the wire contract between the editor and Rust, so it is written to fail loudly when
// the two drift: Rust's enums are externally tagged (`{"Paragraph": {…}}`, `{"Text": {…}}`), its
// unit variants serialize as bare strings (`"Small"`, `"Bullet"`, `"Attachment"`), and a field that
// is `Option<T>` + `#[serde(default)]` may simply be absent. Every optional below is `?`-optional
// for that reason and is OMITTED rather than sent as null; a null would deserialize fine but would
// bloat the document the discard probe diffs on every close.

/// `mailcal_composer::FontSize`: a size in CSS pixels. The toolbar offers `PRESET_SIZES`; a draft
/// another client wrote can carry any size in `MIN_SIZE_PX..=MAX_SIZE_PX`.
export type FontSize = number;

/// `mailcal_composer::Heading`.
export type Heading = "H1" | "H2" | "H3" | "H4" | "H5" | "H6";

/// `mailcal_composer::Align`. A line set at its start carries none.
export type Align = "Center" | "Right" | "Justify";

/// `mailcal_composer::ListKind`.
export type ListKind = "Bullet" | "Ordered";

/// `mailcal_composer::QuoteStyle`.
export type QuoteStyle = "Indented" | "LineAndHeader";

/// A `#rrggbb` colour, lowercase. Rust rejects anything else, so the editor normalises before it
/// emits (see `marks.ts`) rather than passing a browser's `rgb(…)` through and failing on submit.
export type HexColor = string;

/// `mailcal_composer::TextRun`.
export interface TextRun {
  text: string;
  bold: boolean;
  italic: boolean;
  underline: boolean;
  font_size?: FontSize;
  /// `mailcal_composer::FontFamily`, in its canonical spelling (`canonicalFamily`).
  font?: string;
  color?: HexColor;
  highlight?: HexColor;
  /// The run's link target, one of `mailcal_composer::LINK_SCHEMES`.
  link?: string;
}

/// `mailcal_composer::InlineImage`.
export interface InlineImage {
  attachment_id: string;
  alt_text: string;
  width_px?: number | null;
}

/// `mailcal_composer::InlineContent`: externally tagged.
export type InlineContent = { Text: TextRun } | { Image: InlineImage };

/// `mailcal_composer::ListItem`. `child` is a nested sub-list, to any depth.
export interface ListItem {
  content: InlineContent[];
  child: ListValue | null;
}

/// `mailcal_composer::List`.
export interface ListValue {
  kind: ListKind;
  items: ListItem[];
}

export interface TableCell {
  content: InlineContent[];
}

export interface TableRow {
  cells: TableCell[];
}

/// `mailcal_composer::Table`. Rust requires it to be rectangular and non-empty, so every table
/// mutation in `tables.ts` preserves both: a ragged table is a failed send, not a broken render.
export interface TableValue {
  rows: TableRow[];
}

export interface QuoteHeader {
  label: string;
  value: string;
}

export interface QuoteAttribution {
  line: string;
  headers: QuoteHeader[];
}

/// `mailcal_composer::Quote`.
export interface QuoteValue {
  style: QuoteStyle;
  attribution: QuoteAttribution;
  body_html: string;
  body_plain: string;
}

/// `mailcal_composer::Signature`.
export interface SignatureValue {
  body_html: string;
  body_plain: string;
}

/// `mailcal_composer::Paragraph`. The optional fields are omitted when unset, as Rust omits them.
export interface ParagraphValue {
  content: InlineContent[];
  /// A paragraph set apart from the next, as a `<p>` is; omitted for a line, as a `<div>` is.
  spaced?: boolean;
  heading?: Heading;
  align?: Align;
  /// The indent in CSS pixels, omitted at zero.
  indent_px?: number;
}

/// `mailcal_composer::Block`: externally tagged.
export type Block =
  | { Paragraph: ParagraphValue }
  | { Rule: Record<string, never> }
  | { List: ListValue }
  | { Table: TableValue }
  | { Quote: QuoteValue }
  | { Signature: SignatureValue };

/// `mailcal_composer::AttachmentDisposition`. `Attachment` is a bare string (a unit variant);
/// `Inline` is externally tagged like every other data-carrying variant.
export type AttachmentDisposition = "Attachment" | { Inline: { cid: string } };

/// `mailcal_composer::DraftAttachment`. Exactly one of `blob` and `data_url` is set: a file the
/// host staged is referenced by handle, a picture the editor captured carries its own bytes.
export interface DraftAttachment {
  id: string;
  blob?: string;
  file_name: string;
  media_type: string;
  size: number | null;
  disposition: AttachmentDisposition;
  data_url?: string;
}

/// `mailcal_composer::ComposerDocument`: what `composerDocument()` returns.
export interface ComposerDocument {
  blocks: Block[];
  attachments: DraftAttachment[];
}

/// The marks in force at a point in the tree, accumulated as `inlinesFrom` descends.
export interface Marks {
  bold: boolean;
  italic: boolean;
  underline: boolean;
  size: FontSize | null;
  font: string | null;
  color: HexColor | null;
  highlight: HexColor | null;
  link: string | null;
}

export function emptyMarks(): Marks {
  return {
    bold: false,
    italic: false,
    underline: false,
    size: null,
    font: null,
    color: null,
    highlight: null,
    link: null,
  };
}

/// The sizes the toolbar offers, by the name each is shown under. Mirrors `FontSize::SMALL` to
/// `FontSize::HUGE`.
export const PRESET_SIZES = { Small: 13, Normal: 15, Large: 18, Huge: 24 } as const;

/// `FontSize::MIN_PX` and `FontSize::MAX_PX`: the range a size is clamped to.
export const MIN_SIZE_PX = 6;
export const MAX_SIZE_PX = 96;

/// A size of `px` pixels as the document holds it: whole, and clamped to the range Rust keeps.
export function fontSizeOf(px: number): FontSize | null {
  if (!Number.isFinite(px) || px <= 0) return null;
  return Math.min(MAX_SIZE_PX, Math.max(MIN_SIZE_PX, Math.round(px)));
}

/// `Paragraph::MAX_INDENT_PX`.
export const MAX_INDENT_PX = 400;

/// The indent one press of the indent button adds or takes away, in CSS pixels.
export const INDENT_STEP_PX = 40;

export const HEADINGS: readonly Heading[] = ["H1", "H2", "H3", "H4", "H5", "H6"];
