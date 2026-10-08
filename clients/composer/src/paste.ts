// Pasting formatted text: the HTML a word processor, a browser or another mail client puts on the
// clipboard, mapped onto what the document can hold.
//
// That markup is hostile input (`docs/composer-security.md` Gate 7), so none of it is inserted. It
// is parsed into an inert document, and the walk below reads two things out of it, text and a
// closed set of marks, and builds new markup in the editor's own shapes from those alone. Whatever
// it does not name (a script, a style sheet, an event handler, a picture, a class, a font, a
// colour) has no way through. What it keeps is what the document schema holds: paragraphs, line
// breaks, bold, italic, underline, links, lists and tables.
//
// Colour, highlight and size survive only as the editor's own `data-*` stamps, so text copied
// within a draft keeps them. A foreign colour is dropped: Word stamps `color:#1A1A1A` on every run
// and a page in dark mode stamps a light one, and either would follow the text into the message.

import { safeLinkHref } from "./links";
import { normalizeColor } from "./marks";
import { type FontSize, type HexColor, isFontSize, SIZE_PX } from "./types";

interface PasteMarks {
  bold: boolean;
  italic: boolean;
  underline: boolean;
  link: string | null;
  color: HexColor | null;
  highlight: HexColor | null;
  size: FontSize | null;
}

const PLAIN: PasteMarks = {
  bold: false,
  italic: false,
  underline: false,
  link: null,
  color: null,
  highlight: null,
  size: null,
};

/// Dropped with everything inside them: code, metadata, embedded content, and form controls,
/// whose text is a value rather than a sentence. A picture is dropped too: pasted HTML names it by
/// URL, and the composer fetches nothing (Gate 3).
const DROPPED = new Set([
  "script", "style", "template", "head", "title", "meta", "link", "base", "iframe", "frame",
  "frameset", "object", "embed", "noscript", "svg", "math", "canvas", "video", "audio", "img",
  "picture", "input", "select", "textarea", "button", "xml",
]);

/// Elements that start and end a line. Their nesting is not kept: the document has no sections,
/// headings or quotes of its own, so each becomes the paragraphs it contains.
const BLOCKS = new Set([
  "p", "div", "h1", "h2", "h3", "h4", "h5", "h6", "blockquote", "pre", "address", "section",
  "article", "header", "footer", "main", "nav", "aside", "figure", "figcaption", "dl", "dt", "dd",
  "center", "form", "fieldset", "legend", "details", "summary", "hr", "li", "caption",
]);

/// A list's or a table's own elements, which end a line wherever the structure itself could not be
/// kept: inside a list item or a cell, which hold inline content only.
const STRUCTURES = new Set(["ul", "ol", "table", "tbody", "thead", "tfoot", "tr", "td", "th"]);

const HEADINGS = new Set(["h1", "h2", "h3", "h4", "h5", "h6"]);

/// Where the text of one line goes, and what ends a line there.
///
/// At the top level a line is a paragraph, and a list or a table sits between paragraphs. Inside a
/// list item or a table cell the document holds inline content only, so a line ends in a space.
class Flow {
  private line: HTMLElement | null = null;
  /// Whether the text so far ends in collapsible white space, so the next run's leading space is
  /// dropped as a browser would. True at the start of a line, which drops leading space too.
  private spaced = true;
  /// Inside a list item or a cell, a line ended since the last text: the next text starts with the
  /// space that separates the two. Deferred so the end of the item gains no trailing space.
  private separated = false;
  private lastRun: { key: string; text: Text; outer: Node } | null = null;

  constructor(
    private readonly doc: Document,
    private readonly root: Node,
    readonly inline: boolean,
  ) {}

  text(value: string, marks: PasteMarks): void {
    let text = value.replace(/[\t\n\f\r ]+/g, " ");
    if (this.spaced && text.startsWith(" ")) text = text.slice(1);
    if (!text) return;
    if (this.separated) {
      if (!text.startsWith(" ")) text = ` ${text}`;
      this.separated = false;
    }
    this.spaced = text.endsWith(" ");
    const target = this.target();
    const key = JSON.stringify(marks);
    if (this.lastRun?.key === key && this.lastRun.outer === target.lastChild) {
      this.lastRun.text.appendData(text);
      return;
    }
    const node = this.doc.createTextNode(text);
    const outer = wrapRun(this.doc, node, marks);
    target.appendChild(outer);
    this.lastRun = { key, text: node, outer };
  }

  lineBreak(): void {
    if (this.inline) {
      this.endLine();
      return;
    }
    this.trimEnd();
    this.target().appendChild(this.doc.createElement("br"));
    this.spaced = true;
  }

  endLine(): void {
    this.trimEnd();
    if (this.inline) this.separated ||= this.root.hasChildNodes();
    else this.line = null;
    this.spaced = true;
  }

  /// A list or a table, between paragraphs. Never called on an `inline` flow: a list item or a
  /// cell holds inline content only, so the caller flattens the structure into the line instead.
  block(element: HTMLElement): void {
    this.endLine();
    this.root.appendChild(element);
  }

  private target(): Node {
    if (this.inline) return this.root;
    if (!this.line) {
      this.line = this.doc.createElement("p");
      this.root.appendChild(this.line);
    }
    return this.line;
  }

  private trimEnd(): void {
    const run = this.lastRun;
    if (run?.text.data.endsWith(" ")) {
      run.text.data = run.text.data.slice(0, -1);
      // The run's wrappers go with it: an empty `<a>` or `<b>` is a mark on nothing.
      if (!run.text.data) run.outer.parentNode?.removeChild(run.outer);
    }
    this.lastRun = null;
  }
}

/// One run of text in the markup the editor itself writes, which `elementMarks` reads back.
function wrapRun(doc: Document, text: Text, marks: PasteMarks): Node {
  let node: Node = text;
  const wrap = (tag: string): HTMLElement => {
    const element = doc.createElement(tag);
    element.appendChild(node);
    node = element;
    return element;
  };
  // A link is drawn underlined already; the mark would send it out underlined twice.
  if (marks.underline && !marks.link) wrap("u");
  if (marks.italic) wrap("i");
  if (marks.bold) wrap("b");
  if (marks.color || marks.highlight || marks.size) {
    const span = wrap("span");
    if (marks.color) {
      span.dataset.color = marks.color;
      span.style.color = marks.color;
    }
    if (marks.highlight) {
      span.dataset.highlight = marks.highlight;
      span.style.backgroundColor = marks.highlight;
    }
    if (marks.size) {
      span.dataset.size = marks.size;
      span.style.fontSize = `${SIZE_PX[marks.size]}px`;
    }
  }
  if (marks.link) wrap("a").setAttribute("href", marks.link);
  return node;
}

/// The marks in force on `element`, from its tag and the few inline style properties that mean
/// one of them. A style can take a mark away as well as add one: Google Docs wraps a whole copy in
/// `<b style="font-weight:normal">`.
function marksOf(element: HTMLElement, tag: string, inherited: PasteMarks): PasteMarks {
  const style = element.style;
  const weight = style?.getPropertyValue("font-weight") ?? "";
  const fontStyle = style?.getPropertyValue("font-style") ?? "";
  const decoration = `${style?.getPropertyValue("text-decoration-line") ?? ""} ${
    style?.getPropertyValue("text-decoration") ?? ""
  }`;

  // Only a weight or a style that names one decides; `inherit`, `unset` and the like keep the tag's.
  let bold = inherited.bold || tag === "b" || tag === "strong" || tag === "th" || HEADINGS.has(tag);
  const numeric = Number.parseInt(weight, 10);
  if (weight === "bold" || weight === "bolder" || numeric >= 600) bold = true;
  else if (weight === "normal" || weight === "lighter" || numeric < 600) bold = false;
  let italic = inherited.italic || ["i", "em", "cite", "var", "dfn"].includes(tag);
  if (fontStyle === "italic" || fontStyle === "oblique") italic = true;
  else if (fontStyle === "normal") italic = false;
  const underline = inherited.underline || tag === "u" || tag === "ins" || decoration.includes("underline");

  const size = element.dataset?.size;
  return {
    bold,
    italic,
    underline,
    link: (tag === "a" ? safeLinkHref(element.getAttribute("href")) : null) ?? inherited.link,
    color: normalizeColor(element.dataset?.color) ?? inherited.color,
    highlight: normalizeColor(element.dataset?.highlight) ?? inherited.highlight,
    size: isFontSize(size) ? size : inherited.size,
  };
}

function hidden(element: HTMLElement): boolean {
  return element.hasAttribute("hidden") || element.style?.getPropertyValue("display") === "none";
}

/// `pre` is true inside preformatted text, whose newlines are line breaks: code copied from a page
/// keeps its lines.
function walk(node: Node, marks: PasteMarks, flow: Flow, doc: Document, pre = false): void {
  if (node.nodeType === 3) {
    const value = node.nodeValue ?? "";
    if (!pre) {
      flow.text(value, marks);
      return;
    }
    value.split(/\r\n|\r|\n/).forEach((line, index) => {
      if (index > 0) flow.lineBreak();
      flow.text(line, marks);
    });
    return;
  }
  if (node.nodeType !== 1) return;
  const element = node as HTMLElement;
  const tag = element.localName;
  if (DROPPED.has(tag) || hidden(element)) return;
  if (tag === "br") {
    flow.lineBreak();
    return;
  }
  if (!flow.inline && (tag === "ul" || tag === "ol")) {
    const list = listFrom(element, marks, doc);
    if (list) return flow.block(list);
  }
  if (!flow.inline && tag === "table") {
    const table = tableFrom(element, marks, doc);
    if (table) return flow.block(table);
  }
  const inner = marksOf(element, tag, marks);
  const inPre = pre || tag === "pre" || /^pre/.test(element.style?.getPropertyValue("white-space") ?? "");
  const block = BLOCKS.has(tag) || STRUCTURES.has(tag);
  if (block) flow.endLine();
  for (const child of Array.from(element.childNodes)) walk(child, inner, flow, doc, inPre);
  if (block) flow.endLine();
}

function listFrom(source: HTMLElement, marks: PasteMarks, doc: Document): HTMLElement | null {
  const list = doc.createElement(source.localName);
  for (const child of Array.from(source.children) as HTMLElement[]) {
    if (DROPPED.has(child.localName) || hidden(child)) continue;
    // A list straight inside a list (`<ul><li>a</li><ul>…</ul></ul>`, which some editors write)
    // is the previous item's sub-list, not an item of its own with no text.
    const previous = list.lastElementChild;
    const own =
      previous && !Array.from(previous.children).some((node) => /^(ul|ol)$/.test(node.localName));
    if (own && (child.localName === "ul" || child.localName === "ol")) {
      const sublist = listFrom(child, marks, doc);
      if (sublist) previous.appendChild(sublist);
      continue;
    }
    const item = doc.createElement("li");
    const flow = new Flow(doc, item, true);
    let sublist: HTMLElement | null = null;
    const nodes = child.localName === "li" ? Array.from(child.childNodes) : [child];
    for (const node of nodes) {
      const name = (node as Element).localName;
      if (!sublist && (name === "ul" || name === "ol")) sublist = listFrom(node as HTMLElement, marks, doc);
      else walk(node, marksOf(child, child.localName, marks), flow, doc);
    }
    flow.endLine();
    if (sublist) item.appendChild(sublist);
    if (item.hasChildNodes()) list.appendChild(item);
  }
  return list.hasChildNodes() ? list : null;
}

/// A table's own rows, each its cells' inline content, padded to the widest row: Rust refuses a
/// ragged table, and a merged cell (`colspan`) is what makes a pasted one ragged.
function tableFrom(source: HTMLElement, marks: PasteMarks, doc: Document): HTMLElement | null {
  const rows: HTMLElement[][] = [];
  const sections = [
    source,
    ...Array.from(source.children).filter((child) => /^(thead|tbody|tfoot)$/.test(child.localName)),
  ];
  for (const section of sections) {
    for (const row of Array.from(section.children) as HTMLElement[]) {
      if (row.localName !== "tr" || hidden(row)) continue;
      const cells: HTMLElement[] = [];
      for (const cell of Array.from(row.children) as HTMLElement[]) {
        if ((cell.localName !== "td" && cell.localName !== "th") || hidden(cell)) continue;
        const td = doc.createElement("td");
        const flow = new Flow(doc, td, true);
        const cellMarks = marksOf(cell, cell.localName, marks);
        for (const child of Array.from(cell.childNodes)) walk(child, cellMarks, flow, doc);
        flow.endLine();
        cells.push(td);
        // A merged cell keeps the columns after it in place: its content in the first, the rest
        // empty. 1000 is the most a browser honours.
        const span = Math.min(Number.parseInt(cell.getAttribute("colspan") ?? "", 10) || 1, 1000);
        for (let extra = 1; extra < span; extra += 1) cells.push(doc.createElement("td"));
      }
      if (cells.length > 0) rows.push(cells);
    }
  }
  if (rows.length === 0) return null;
  const width = Math.max(...rows.map((cells) => cells.length));
  const table = doc.createElement("table");
  const body = table.appendChild(doc.createElement("tbody"));
  for (const cells of rows) {
    const tr = body.appendChild(doc.createElement("tr"));
    while (cells.length < width) cells.push(doc.createElement("td"));
    for (const td of cells) {
      // An empty cell needs the placeholder to be clicked into (`emptyLine`).
      if (!td.hasChildNodes()) td.appendChild(doc.createElement("br"));
      tr.appendChild(td);
    }
  }
  return table;
}

/// The editor markup for a clipboard's `text/html`, or `null` when it holds no text worth keeping
/// (a copied picture, say), so the caller falls back to the plain-text flavour.
///
/// One paragraph comes back as its runs alone, so a phrase pasted mid-sentence joins that sentence
/// rather than splitting it into three lines. `inline` is a paste into a list item or a table cell,
/// which hold inline content only, so everything arrives as one line there.
export function pastedHtml(doc: Document, html: string, inline = false): string | null {
  const view = doc.defaultView as (Window & typeof globalThis) | null;
  if (!view?.DOMParser) return null;
  // A parsed document has no browsing context: its scripts never run and its pictures never load.
  const source = new view.DOMParser().parseFromString(html, "text/html");
  const out = doc.createElement("div");
  const flow = new Flow(doc, out, inline);
  for (const child of Array.from(source.body?.childNodes ?? [])) walk(child, PLAIN, flow, doc);
  flow.endLine();
  if (!out.textContent?.trim()) return null;
  const only = out.childNodes.length === 1 ? (out.firstChild as Element) : null;
  return only?.localName === "p" ? only.innerHTML : out.innerHTML;
}
