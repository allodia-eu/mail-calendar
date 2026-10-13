// A line's own style: whether it is a heading, how it is aligned, how far it is indented.
//
// The editor writes these as a heading element and an inline `style` (`text-align`,
// `margin-left`), which is what every other client writes too. Reading is wider, because a reopened
// draft brings their other spellings: an `align` attribute, `<center>`, an indent in points or
// inches, and Thunderbird's `<blockquote>` used as an indent rather than as a quote.

import { pxOfLength } from "./typography";
import { type Align, type Heading, HEADINGS, MAX_INDENT_PX, type ParagraphValue } from "./types";

/// The style a line carries, as the document holds it.
export interface LineStyle {
  spaced?: boolean;
  heading?: Heading;
  align?: Align;
  indent_px?: number;
}

/// What a line inherits from the elements around it: alignment carries into a line, as CSS's
/// `text-align` does, and indents add up, as margins do. A heading belongs to its own element.
export interface LineContext {
  align?: Align;
  indent_px: number;
}

export const NO_CONTEXT: LineContext = { indent_px: 0 };

/// The context the children of `element` are laid out in.
export function contextWithin(element: Element, outer: LineContext): LineContext {
  return {
    align: alignOf(element) ?? outer.align,
    indent_px: outer.indent_px + ownIndent(element),
  };
}

/// The style of a line drawn by `element` in `context`.
export function lineStyleOf(element: Element | null, context: LineContext): LineStyle {
  const within = element ? contextWithin(element, context) : context;
  const style: LineStyle = {};
  const heading = element ? headingOf(element) : null;
  if (heading) style.heading = heading;
  else if (element && isParagraph(element)) style.spaced = true;
  if (within.align) style.align = within.align;
  const indent = Math.min(MAX_INDENT_PX, Math.round(within.indent_px));
  if (indent > 0) style.indent_px = indent;
  return style;
}

/// A paragraph holding `content` in `style`, its unset fields omitted as Rust omits them.
export function paragraph(content: ParagraphValue["content"], style: LineStyle): ParagraphValue {
  return { content, ...style };
}

/// Whether `element` is a paragraph, set apart from the next, rather than a line. A `<p>` is, as a
/// browser draws it, unless it says it has no spacing: Word's `MsoNormal` lines are `<p>`s whose
/// stylesheet (which the sanitiser drops) takes their margins away, and an inline `margin:0`
/// does the same.
function isParagraph(element: Element): boolean {
  if (element.tagName !== "P" || /(^|\s)Mso/.test(element.getAttribute("class") ?? "")) return false;
  const style = (element as HTMLElement).style;
  const top = pxOfLength(style?.marginTop);
  const bottom = pxOfLength(style?.marginBottom);
  return !(top === 0 && bottom === 0);
}

export function headingOf(element: Element): Heading | null {
  const tag = element.tagName;
  return (HEADINGS as readonly string[]).includes(tag) ? (tag as Heading) : null;
}

function alignOf(element: Element): Align | null {
  if (element.tagName === "CENTER") return "Center";
  const raw = ((element as HTMLElement).style?.textAlign || element.getAttribute("align") || "")
    .trim()
    .toLowerCase();
  if (raw === "center" || raw === "middle") return "Center";
  if (raw === "right" || raw === "end") return "Right";
  if (raw === "justify") return "Justify";
  return null;
}

/// The indent `element` itself adds: its left margin, or for a `<blockquote>` that is an indent
/// rather than a quote (`read_html.ts` has already taken the quotes), the 40px a browser gives it.
function ownIndent(element: Element): number {
  const margin = pxOfLength((element as HTMLElement).style?.marginLeft);
  if (margin !== null && margin > 0) return margin;
  return element.tagName === "BLOCKQUOTE" ? 40 : 0;
}

/// Writes `style` onto the editor's own line element, replacing any it had.
export function applyLineStyle(element: HTMLElement, style: LineStyle): void {
  element.style.textAlign = style.align ? style.align.toLowerCase() : "";
  element.style.marginLeft = style.indent_px ? `${style.indent_px}px` : "";
  if (!element.getAttribute("style")) element.removeAttribute("style");
}

/// The element a line in `style` is drawn as.
export function lineTag(style: LineStyle): string {
  if (style.heading) return style.heading.toLowerCase();
  return style.spaced ? "p" : "div";
}
