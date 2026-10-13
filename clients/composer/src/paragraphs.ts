// The commands that act on whole lines: heading level, alignment, indent, and the rule between two
// blocks.
//
// Written as DOM surgery for the reason `lists.ts` is: the legacy commands (`formatBlock`,
// `justifyCenter`, `indent`) emit different markup per engine, and `indent` wraps lines in a
// `<blockquote>` the reader would have to tell apart from a quote. Each command here writes the one
// shape `lines.ts` reads, so a line formatted on any WebView saves the same way.

import { caretInto, documentOf, emptyLine, focusEditor, rangeOverlaps, rangeWithin, restoreSelection, saveSelection } from "./dom";
import { applyLineStyle, type LineStyle, lineStyleOf, lineTag, NO_CONTEXT } from "./lines";
import { type Align, type Heading, INDENT_STEP_PX, MAX_INDENT_PX } from "./types";

const LINE_TAGS = new Set(["P", "DIV", "H1", "H2", "H3", "H4", "H5", "H6"]);

/// Where a line style does not apply: a list item, a table cell, or one of the regions that hold
/// another message's or the signature's markup verbatim.
const OUTSIDE_LINES = "li, td, th, .allodia-quote, .allodia-signature";

/// The lines the selection touches, in document order: elements that hold a line of the message
/// rather than other blocks. A collapsed caret yields the one line it sits in.
export function selectedLines(editor: HTMLElement): HTMLElement[] {
  const range = rangeWithin(editor);
  if (!range) return [];
  const doc = documentOf(editor);
  return Array.from(editor.querySelectorAll<HTMLElement>("p, div, h1, h2, h3, h4, h5, h6")).filter((line) => {
    if (line.closest(OUTSIDE_LINES)) return false;
    if (Array.from(line.children).some((child) => LINE_TAGS.has(child.tagName) || child.tagName === "UL" || child.tagName === "OL" || child.tagName === "TABLE")) {
      return false;
    }
    const own = doc.createRange();
    own.selectNodeContents(line);
    return rangeOverlaps(range, own);
  });
}

/// Applies `change` to every selected line, keeping the selection where it was.
function eachLine(editor: HTMLElement, change: (line: HTMLElement, style: LineStyle) => LineStyle): boolean {
  const lines = selectedLines(editor);
  if (lines.length === 0) return false;
  const caret = saveSelection(editor);
  for (const line of lines) {
    const current = lineStyleOf(line, NO_CONTEXT);
    const next = change(line, current);
    const element = lineTag(next) === lineTag(current) ? line : retag(line, lineTag(next));
    applyLineStyle(element, next);
  }
  restoreSelection(editor, caret);
  focusEditor(editor);
  return true;
}

/// `line` as a `tag` element holding the same children. The children move rather than being
/// copied, so the saved selection's text nodes survive and the caret stays put.
function retag(line: HTMLElement, tag: string): HTMLElement {
  const element = documentOf(line).createElement(tag);
  for (const attribute of Array.from(line.attributes)) element.setAttribute(attribute.name, attribute.value);
  while (line.firstChild) element.appendChild(line.firstChild);
  line.replaceWith(element);
  return element;
}

/// Makes the selected lines headings at `heading`, or body text again when it is `null`.
export function setHeading(editor: HTMLElement, heading: Heading | null): boolean {
  return eachLine(editor, (_, style) => {
    const next: LineStyle = { ...style };
    if (heading) next.heading = heading;
    else delete next.heading;
    return next;
  });
}

/// Aligns the selected lines, or sets them back at the start of the line when `align` is `null`.
export function setAlign(editor: HTMLElement, align: Align | null): boolean {
  return eachLine(editor, (_, style) => {
    const next: LineStyle = { ...style };
    if (align) next.align = align;
    else delete next.align;
    return next;
  });
}

/// Indents the selected lines one step, or takes one step away. Returns whether the selection was
/// on lines at all, so the caller can let the key it came from fall through when it was not.
export function indentLines(editor: HTMLElement, outdent: boolean): boolean {
  return eachLine(editor, (_, style) => {
    const current = style.indent_px ?? 0;
    const indent = outdent
      ? Math.max(0, current - INDENT_STEP_PX)
      : Math.min(MAX_INDENT_PX, current + INDENT_STEP_PX);
    const next: LineStyle = { ...style };
    if (indent > 0) next.indent_px = indent;
    else delete next.indent_px;
    return next;
  });
}

/// Inserts a horizontal rule below the block the caret is in, and puts the caret on the line
/// after it, opening one when the rule would otherwise end the message.
export function insertRule(editor: HTMLElement): void {
  const range = rangeWithin(editor);
  const doc = documentOf(editor);
  let block: Node | null = range?.startContainer ?? null;
  while (block && block.parentNode !== editor) block = block.parentNode;
  const rule = doc.createElement("hr");
  if (block) editor.insertBefore(rule, block.nextSibling);
  else editor.appendChild(rule);
  let next = rule.nextElementSibling as HTMLElement | null;
  if (!next || !LINE_TAGS.has(next.tagName)) {
    next = emptyLine(doc);
    editor.insertBefore(next, rule.nextSibling);
  }
  caretInto(next, true);
  focusEditor(editor);
}
