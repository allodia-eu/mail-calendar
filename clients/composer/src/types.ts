// The compose document, typed to mirror `crates/mailcal-composer`'s serde shapes.
//
// This file is the wire contract between the editor and Rust, so it is written to fail loudly when
// the two drift: Rust's enums are externally tagged (`{"Paragraph": {…}}`, `{"Text": {…}}`), its
// unit variants serialize as bare strings (`"Small"`, `"Bullet"`, `"Attachment"`), and a field that
// is `Option<T>` + `#[serde(default)]` may simply be absent. Every optional below is `?`-optional
// for that reason and is OMITTED rather than sent as null; a null would deserialize fine but would
// bloat the document the discard probe diffs on every close.

/// `mailcal_composer::FontSize`. The `<option>` values in the toolbar are these tokens verbatim.
export type FontSize = "Small" | "Normal" | "Large" | "Huge";

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
  color?: HexColor;
  highlight?: HexColor;
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

/// `mailcal_composer::Block`: externally tagged.
export type Block =
  | { Paragraph: { content: InlineContent[] } }
  | { List: ListValue }
  | { Table: TableValue }
  | { Quote: QuoteValue }
  | { Signature: SignatureValue };

/// `mailcal_composer::AttachmentDisposition`. `Attachment` is a bare string (a unit variant);
/// `Inline` is externally tagged like every other data-carrying variant.
export type AttachmentDisposition = "Attachment" | { Inline: { cid: string } };

/// `mailcal_composer::DraftAttachment`.
export interface DraftAttachment {
  id: string;
  blob: string;
  file_name: string;
  media_type: string;
  size: number | null;
  disposition: AttachmentDisposition;
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
  color: HexColor | null;
  highlight: HexColor | null;
}

export function emptyMarks(): Marks {
  return {
    bold: false,
    italic: false,
    underline: false,
    size: null,
    color: null,
    highlight: null,
  };
}

/// The pixel size each `FontSize` renders at, in the editor and in the outgoing HTML.
/// Mirrors `FontSize::css_px`; the two must agree or the composer lies about what it will send.
export const SIZE_PX: Record<FontSize, number> = {
  Small: 13,
  Normal: 15,
  Large: 18,
  Huge: 24,
};

export function isFontSize(value: string | null | undefined): value is FontSize {
  return value === "Small" || value === "Normal" || value === "Large" || value === "Huge";
}
