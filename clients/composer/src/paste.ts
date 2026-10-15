// Pasting formatted text: the HTML a word processor, a browser or another mail client puts on the
// clipboard, mapped onto what the document can hold.
//
// That markup is hostile input (`docs/composer-security.md` Gate 7), so none of it is inserted. It
// is parsed into an inert document, and the walk below reads two things out of it, text and a
// closed set of marks, and builds new markup in the editor's own shapes from those alone. Whatever
// it does not name (a script, an event handler, a picture, a font, a colour) has no way through.
// What it keeps is what the document schema holds: paragraphs, line breaks, bold, italic,
// underline, links, lists and tables. A style sheet is read for those three marks on a class and
// nothing else, because that is how Excel marks a cell (`classRules`).
//
// Colour, highlight and size survive only as the editor's own `data-*` stamps, so text copied
// within a draft keeps them. A foreign colour is dropped: Word stamps `color:#1A1A1A` on every run
// and a page in dark mode stamps a light one, and either would follow the text into the message.

import type { Attachments } from "./attachments";
import { listStart } from "./document";
import { imageFilesFrom, isShowableDataUrl, MAX_CAPTURED_BYTES } from "./images";
import { safeLinkHref } from "./links";
import { normalizeColor } from "./marks";
import { type ClassRule, classRules, declared, MARK_PROPERTIES } from "./paste_styles";
import { wordLists } from "./paste_word";
import { type FontSize, fontSizeOf, type HexColor } from "./types";

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

/// What the walk builds with: the editor's document, and the class rules of the markup it reads.
interface Paste {
  doc: Document;
  classes: ClassRule[];
  /// Where a picture the markup carries is registered, or `null` when the caller keeps none.
  attachments: Attachments | null;
}

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

  /// A picture, inline among the text of the line.
  image(node: HTMLElement): void {
    if (this.separated) {
      this.target().appendChild(this.doc.createTextNode(" "));
      this.separated = false;
    }
    this.target().appendChild(node);
    this.spaced = false;
    this.lastRun = null;
  }

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
      span.dataset.size = String(marks.size);
      span.style.fontSize = `${marks.size}px`;
    }
  }
  if (marks.link) wrap("a").setAttribute("href", marks.link);
  return node;
}

/// The marks in force on `element`, from its tag and the few style properties that mean one of
/// them, inline or from a class rule. A style can take a mark away as well as add one: Google Docs
/// wraps a whole copy in `<b style="font-weight:normal">`.
function marksOf(element: HTMLElement, tag: string, inherited: PasteMarks, classes: ClassRule[]): PasteMarks {
  const [weight, fontStyle, decorationLine, decoration] = MARK_PROPERTIES.map((property) =>
    declared(element, property, classes),
  ) as [string, string, string, string];

  // Only a weight or a style that names one decides; `inherit`, `unset` and the like keep the tag's.
  let bold = inherited.bold || tag === "b" || tag === "strong" || tag === "th" || HEADINGS.has(tag);
  const numeric = Number.parseInt(weight, 10);
  if (weight === "bold" || weight === "bolder" || numeric >= 600) bold = true;
  else if (weight === "normal" || weight === "lighter" || numeric < 600) bold = false;
  let italic = inherited.italic || ["i", "em", "cite", "var", "dfn"].includes(tag);
  if (fontStyle === "italic" || fontStyle === "oblique") italic = true;
  else if (fontStyle === "normal") italic = false;
  const underline =
    inherited.underline || tag === "u" || tag === "ins" || `${decorationLine} ${decoration}`.includes("underline");

  const size = Number(element.dataset?.size);
  return {
    bold,
    italic,
    underline,
    link: (tag === "a" ? safeLinkHref(element.getAttribute("href")) : null) ?? inherited.link,
    color: normalizeColor(element.dataset?.color) ?? inherited.color,
    highlight: normalizeColor(element.dataset?.highlight) ?? inherited.highlight,
    size: size > 0 ? fontSizeOf(size) : inherited.size,
  };
}

function hidden(element: HTMLElement): boolean {
  return element.hasAttribute("hidden") || element.style?.getPropertyValue("display") === "none";
}

/// `pre` is true inside preformatted text, whose newlines are line breaks: code copied from a page
/// keeps its lines.
function walk(node: Node, marks: PasteMarks, flow: Flow, paste: Paste, pre = false): void {
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
  if (tag === "img" && !hidden(element)) {
    const picture = inlinePicture(element, paste);
    if (picture) flow.image(picture);
    return;
  }
  if (DROPPED.has(tag) || hidden(element)) return;
  if (tag === "br") {
    flow.lineBreak();
    return;
  }
  if (!flow.inline && (tag === "ul" || tag === "ol")) {
    const list = listFrom(element, marks, paste);
    if (list) return flow.block(list);
  }
  if (!flow.inline && tag === "table") {
    const table = tableFrom(element, marks, paste);
    if (table) return flow.block(table);
  }
  const inner = marksOf(element, tag, marks, paste.classes);
  const inPre = pre || tag === "pre" || /^pre/.test(element.style?.getPropertyValue("white-space") ?? "");
  const block = BLOCKS.has(tag) || STRUCTURES.has(tag);
  if (block) flow.endLine();
  for (const child of Array.from(element.childNodes)) walk(child, inner, flow, paste, inPre);
  if (block) flow.endLine();
}

function listFrom(source: HTMLElement, marks: PasteMarks, paste: Paste): HTMLElement | null {
  const { doc } = paste;
  const list = doc.createElement(source.localName);
  const start = listStart(source);
  if (start !== null) list.setAttribute("start", String(start));
  for (const child of Array.from(source.children) as HTMLElement[]) {
    if (DROPPED.has(child.localName) || hidden(child)) continue;
    // A list straight inside a list (`<ul><li>a</li><ul>…</ul></ul>`, which some editors write)
    // is the previous item's sub-list, not an item of its own with no text.
    const previous = list.lastElementChild;
    const own =
      previous && !Array.from(previous.children).some((node) => /^(ul|ol)$/.test(node.localName));
    if (own && (child.localName === "ul" || child.localName === "ol")) {
      const sublist = listFrom(child, marks, paste);
      if (sublist) previous.appendChild(sublist);
      continue;
    }
    const item = doc.createElement("li");
    const flow = new Flow(doc, item, true);
    let sublist: HTMLElement | null = null;
    const nodes = child.localName === "li" ? Array.from(child.childNodes) : [child];
    for (const node of nodes) {
      const name = (node as Element).localName;
      if (!sublist && (name === "ul" || name === "ol")) sublist = listFrom(node as HTMLElement, marks, paste);
      else walk(node, marksOf(child, child.localName, marks, paste.classes), flow, paste);
    }
    flow.endLine();
    if (sublist) item.appendChild(sublist);
    if (item.hasChildNodes()) list.appendChild(item);
  }
  return list.hasChildNodes() ? list : null;
}

/// A table's own rows, each its cells' inline content, padded to the widest row: Rust refuses a
/// ragged table, and a merged cell (`colspan`) is what makes a pasted one ragged.
function tableFrom(source: HTMLElement, marks: PasteMarks, paste: Paste): HTMLElement | null {
  const { doc } = paste;
  const rows: HTMLElement[][] = [];
  const sections = [
    source,
    ...Array.from(source.children).filter((child) => /^(thead|tbody|tfoot)$/.test(child.localName)),
  ];
  // The columns a cell merged down from an earlier row (`rowspan`) holds in each later one, by row:
  // the rows it reaches get an empty cell there, so their own cells keep their columns. Counted
  // over every row of the markup, since a hidden one (Excel writes a hidden row as
  // `display:none`) is still a row the merge reaches.
  const held: Set<number>[] = [];
  let index = -1;
  for (const section of sections) {
    for (const row of Array.from(section.children) as HTMLElement[]) {
      if (row.localName !== "tr") continue;
      index += 1;
      if (hidden(row)) continue;
      const cells: HTMLElement[] = [];
      const taken = held[index] ?? new Set<number>();
      const skipTaken = () => {
        while (taken.has(cells.length)) cells.push(doc.createElement("td"));
      };
      for (const cell of Array.from(row.children) as HTMLElement[]) {
        if ((cell.localName !== "td" && cell.localName !== "th") || hidden(cell)) continue;
        skipTaken();
        const column = cells.length;
        const td = doc.createElement("td");
        const flow = new Flow(doc, td, true);
        const cellMarks = marksOf(cell, cell.localName, marks, paste.classes);
        for (const child of Array.from(cell.childNodes)) walk(child, cellMarks, flow, paste);
        flow.endLine();
        cells.push(td);
        // A merged cell keeps the columns after it in place: its content in the first, the rest
        // empty. 1000 is the most a browser honours.
        const span = Math.min(Number.parseInt(cell.getAttribute("colspan") ?? "", 10) || 1, 1000);
        for (let extra = 1; extra < span; extra += 1) cells.push(doc.createElement("td"));
        const down = Math.min(Number.parseInt(cell.getAttribute("rowspan") ?? "", 10) || 1, 1000);
        for (let below = 1; below < down; below += 1) {
          const later = (held[index + below] ??= new Set<number>());
          for (let at = column; at < column + span; at += 1) later.add(at);
        }
      }
      skipTaken();
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

/// The picture `element` carries inline, as a new inline attachment of this draft, or `null`.
///
/// Only a `data:` URI of a format a message body may carry, within the cap a pasted picture has
/// (images.ts): what a picture copied from inside a draft looks like on the clipboard. One the
/// markup names by URL, on the web or on disk, is dropped, because the composer fetches nothing and
/// reads no file the user did not pick (docs/composer-security.md, Gate 7). An attachment id the
/// markup carries is never reused: it names an attachment of another draft, or of none.
function inlinePicture(element: HTMLElement, paste: Paste): HTMLElement | null {
  const url = element.getAttribute("src") ?? "";
  if (!paste.attachments || !isShowableDataUrl(url)) return null;
  const comma = url.indexOf(",");
  const payload = url.length - comma - 1;
  const bytes = url.slice(0, comma).endsWith(";base64") ? Math.floor((payload * 3) / 4) : payload;
  if (bytes > MAX_CAPTURED_BYTES) return null;
  const id = paste.attachments.addCapturedImage({ data_url: url, file_name: "", media_type: "" });
  const picture = paste.doc.createElement("img");
  picture.dataset.attachmentId = id;
  picture.setAttribute("src", url);
  picture.setAttribute("alt", element.getAttribute("alt") ?? "");
  const width = Number.parseInt(element.getAttribute("width") ?? "", 10);
  if (width > 0) picture.setAttribute("width", String(width));
  return picture;
}

/// The editor markup for a clipboard's `text/html`, or `null` when it holds neither text nor a
/// picture worth keeping (a browser's copied picture, say), so the caller falls back to the
/// clipboard's picture or its plain-text flavour.
///
/// One paragraph comes back as its runs alone, so a phrase pasted mid-sentence joins that sentence
/// rather than splitting it into three lines. `inline` is a paste into a list item or a table cell,
/// which hold inline content only, so everything arrives as one line there. A picture the markup
/// carries is kept only when `attachments` is given to register it in.
export function pastedHtml(
  doc: Document,
  html: string,
  inline = false,
  attachments: Attachments | null = null,
): string | null {
  const view = doc.defaultView as (Window & typeof globalThis) | null;
  if (!view?.DOMParser) return null;
  // A parsed document has no browsing context: its scripts never run and its pictures never load.
  const source = new view.DOMParser().parseFromString(html, "text/html");
  if (source.body) wordLists(source.body);
  const out = doc.createElement("div");
  const flow = new Flow(doc, out, inline);
  const paste = { doc, classes: classRules(view, source), attachments };
  for (const child of Array.from(source.body?.childNodes ?? [])) walk(child, PLAIN, flow, paste);
  flow.endLine();
  if (!out.textContent?.trim() && !out.querySelector("img")) return null;
  const only = out.childNodes.length === 1 ? (out.firstChild as Element) : null;
  return only?.localName === "p" ? only.innerHTML : out.innerHTML;
}

/// What one paste puts in the message.
export type PasteContent = { html: string } | { images: File[] } | { text: string };

/// Decides what a paste carries, the same way on every host.
///
/// **Formatted text beats a picture beside it.** A spreadsheet, a word processor or a presentation
/// puts a rendering of the selection on the clipboard next to its HTML, so a picture offered with
/// HTML that holds text is a picture *of* that text, and taking it would paste a table as a
/// screenshot of one. A browser's Copy Image offers HTML too, but only an `<img>` naming the
/// picture by URL, which holds nothing kept, so the picture is what arrives there.
export function pasteContent(
  doc: Document,
  transfer: DataTransfer | null | undefined,
  inline: boolean,
  attachments: Attachments | null = null,
): PasteContent {
  const html = pastedHtml(doc, transfer?.getData("text/html") ?? "", inline, attachments);
  if (html !== null) return { html };
  const images = imageFilesFrom(transfer);
  if (images.length > 0) return { images };
  return { text: transfer?.getData("text/plain") ?? "" };
}
