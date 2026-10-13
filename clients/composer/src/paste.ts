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

import { imageFilesFrom } from "./images";
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

/// The style properties a mark is read from, and the only ones read from a style sheet.
const MARK_PROPERTIES = ["font-weight", "font-style", "text-decoration-line", "text-decoration"];

/// A rule in the pasted markup's style sheet that gives a class one of the marks: `.xl63` or
/// `td.xl63`.
interface ClassRule {
  tag: string | null;
  className: string;
  style: CSSStyleDeclaration;
}

/// What the walk builds with: the editor's document, and the class rules of the markup it reads.
interface Paste {
  doc: Document;
  classes: ClassRule[];
}

/// The class rules in `source`'s style sheets that name a mark.
///
/// Excel writes every cell's bold, italic and underline as a class (`.xl63 {font-weight:700}`), and
/// a run inside a cell as `<font class="font5">`, so without these a sheet pastes with none of its
/// formatting. Each sheet is parsed into a stylesheet that is never applied, so nothing in it loads
/// or draws, and only `MARK_PROPERTIES` are read back. A selector counts when it is a single class,
/// alone or on one tag; one that depends on an element's place in the document is skipped, since
/// the walk does not keep that place.
function classRules(view: Window & typeof globalThis, source: Document): ClassRule[] {
  if (typeof view.CSSStyleSheet !== "function") return [];
  const rules: ClassRule[] = [];
  for (const element of Array.from(source.querySelectorAll("style"))) {
    const sheet = new view.CSSStyleSheet();
    try {
      sheet.replaceSync(element.textContent ?? "");
    } catch {
      continue;
    }
    for (const rule of Array.from(sheet.cssRules)) {
      if (!("selectorText" in rule) || !("style" in rule)) continue;
      const style = rule.style as CSSStyleDeclaration;
      if (!MARK_PROPERTIES.some((property) => style.getPropertyValue(property))) continue;
      for (const selector of String(rule.selectorText).split(",")) {
        const match = /^([a-z][a-z0-9]*)?\.(-?[_a-z][\w-]*)$/i.exec(selector.trim());
        if (match) rules.push({ tag: match[1]?.toLowerCase() ?? null, className: match[2]!, style });
      }
    }
  }
  return rules;
}

/// `property` on `element` as the cascade would settle it among what is read: the inline style,
/// else the class rule that names it, a rule on the element's tag over a bare class and a later
/// rule over an earlier one.
function declared(element: HTMLElement, property: string, classes: ClassRule[]): string {
  const inline = element.style?.getPropertyValue(property) ?? "";
  if (inline || classes.length === 0) return inline;
  let value = "";
  let rank = -1;
  for (const rule of classes) {
    if (rule.tag && rule.tag !== element.localName) continue;
    if (!element.classList?.contains(rule.className)) continue;
    const named = rule.style.getPropertyValue(property);
    const ruleRank = rule.tag ? 1 : 0;
    if (named && ruleRank >= rank) {
      value = named;
      rank = ruleRank;
    }
  }
  return value;
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
  for (const section of sections) {
    for (const row of Array.from(section.children) as HTMLElement[]) {
      if (row.localName !== "tr" || hidden(row)) continue;
      const cells: HTMLElement[] = [];
      for (const cell of Array.from(row.children) as HTMLElement[]) {
        if ((cell.localName !== "td" && cell.localName !== "th") || hidden(cell)) continue;
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
  const paste = { doc, classes: classRules(view, source) };
  for (const child of Array.from(source.body?.childNodes ?? [])) walk(child, PLAIN, flow, paste);
  flow.endLine();
  if (!out.textContent?.trim()) return null;
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
/// screenshot of one. A browser's Copy Image offers HTML too, but only an `<img>`, which holds no
/// text, so the picture is what arrives there.
export function pasteContent(
  doc: Document,
  transfer: DataTransfer | null | undefined,
  inline: boolean,
): PasteContent {
  const html = pastedHtml(doc, transfer?.getData("text/html") ?? "", inline);
  if (html !== null) return { html };
  const images = imageFilesFrom(transfer);
  if (images.length > 0) return { images };
  return { text: transfer?.getData("text/plain") ?? "" };
}
