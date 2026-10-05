// Reading a stored message body back into the editor's document: a draft reopened from Drafts, or a
// message moved back out of the Outbox.
//
// What this app wrote reads back as the document it was rendered from: the core's renderer and
// this reader are held to each other by a fixture both test suites read
// (`tests/fixtures/rendered.json`). Another client's markup is read as closely as the editor's
// closed schema can hold it, and what it cannot hold is dropped here rather than shown, because the
// composer saves what it opened with: a construct drawn on screen but absent from the document
// would vanish from the draft on the next save without the user having touched it.
//
// The HTML arrives sanitised by the core, its `cid:` pictures already turned into `data:` URIs
// (`docs/composer-security.md`, Gate 17). It is parsed into a document of its own, which has no
// window: nothing in it runs, loads or lays out, and none of it is ever attached to the editor.
// Only the blocks read off it are, rebuilt by `blocks_dom.ts`. The two raw-HTML regions, a quoted
// original and a signature, keep their markup as those blocks always do (Gate 10).

import { type Attachments, mediaTypeOf } from "./attachments";
import { documentBlocks } from "./document";
import { windowOf } from "./dom";
import { isShowableImage } from "./images";
import { quoteContainer } from "./quote";
import type { Block, InlineContent, QuoteHeader, TextRun } from "./types";

/// The blocks `html` holds, for the composer `editor`. Pictures it carries as `data:` URIs are added
/// to `attachments`, and the blocks reference them by the ids minted there.
export function blocksFromHtml(editor: HTMLElement, html: string, attachments: Attachments): Block[] {
  const parsed = new (windowOf(editor).DOMParser)().parseFromString(html, "text/html");
  const body = parsed.body;
  for (const node of Array.from(body.querySelectorAll(INERT_TAGS))) node.remove();
  readOwnQuotes(body);
  readOtherQuotes(body);
  for (const signature of Array.from(body.querySelectorAll<HTMLElement>(".allodia-signature"))) {
    if (!signature.closest(".allodia-quote")) signature.dataset.signaturePlain = plainText(signature);
  }
  adoptPictures(body, attachments);
  breakPreformattedLines(body);
  collapseWhitespace(body);
  return documentBlocks(body).map(tidy);
}

/// Elements that are not message content. The core's sanitiser has already taken out what could
/// run; these are what it keeps that the reader must still not read as text.
const INERT_TAGS = "head, style, title, template, noscript, script";

/// The indented quote's left border, which only this app's renderer writes (`INDENTED_QUOTE_STYLE`
/// in `crates/mailcal-composer`): the mark that the paragraph before it is that quote's attribution
/// rather than the user's last line.
const OWN_INDENTED_BORDER = "border-left:2px solid #cccccc";

/// The colour of the divider over a line-and-header quote's header block, which this app's renderer
/// writes (`HEADER_DIVIDER_OPEN`).
const OWN_HEADER_DIVIDER = "rgb(181, 196, 223)";

function styleOf(element: Element): string {
  return (element.getAttribute("style") ?? "").replace(/\s+/g, " ").replace(/:\s/g, ":");
}

/// Turns the two quote shapes this app's renderer writes back into the quote regions they were
/// rendered from, attribution included.
function readOwnQuotes(body: HTMLElement): void {
  for (const quote of Array.from(body.querySelectorAll("blockquote"))) {
    if (!styleOf(quote).includes(OWN_INDENTED_BORDER)) continue;
    const attribution = quote.previousElementSibling;
    let line = "";
    if (attribution?.tagName === "P") {
      line = attribution.textContent ?? "";
      attribution.remove();
    }
    replaceWithQuote(quote, { style: "Indented", line, headers: [] }, quote);
  }
  for (const divider of Array.from(body.querySelectorAll("div"))) {
    if (!styleOf(divider).includes(OWN_HEADER_DIVIDER)) continue;
    const quoted = divider.nextElementSibling;
    if (quoted?.tagName !== "DIV") continue;
    const headers = headersOf(divider);
    divider.remove();
    replaceWithQuote(quoted, { style: "LineAndHeader", line: "", headers }, quoted);
  }
}

/// A quote another client wrote: every `<blockquote>` that is not inside one already. Its
/// attribution, if it has one, is ordinary text above it and stays that way.
function readOtherQuotes(body: HTMLElement): void {
  for (const quote of Array.from(body.querySelectorAll("blockquote"))) {
    if (quote.parentElement?.closest("blockquote, .allodia-quote, li, td, th")) continue;
    replaceWithQuote(quote, { style: "Indented", line: "", headers: [] }, quote);
  }
}

interface Attribution {
  style: "Indented" | "LineAndHeader";
  line: string;
  headers: QuoteHeader[];
}

/// Replaces `element` with a quote region whose body is `content`'s children.
function replaceWithQuote(element: Element, attribution: Attribution, content: Element): void {
  const doc = element.ownerDocument;
  const region = quoteContainer(doc, {
    style: attribution.style,
    attribution: { line: attribution.line, headers: attribution.headers },
    body_plain: plainText(content),
  });
  const quoted = region.querySelector(".aq-body")!;
  quoted.textContent = "";
  while (content.firstChild) quoted.appendChild(content.firstChild);
  element.replaceWith(region);
}

/// The `<strong>Label: </strong>value<br>` lines of a header block.
function headersOf(divider: Element): QuoteHeader[] {
  const headers: QuoteHeader[] = [];
  let current: QuoteHeader | null = null;
  const walk = (node: Node) => {
    for (const child of Array.from(node.childNodes)) {
      const tag = child.nodeType === 1 ? (child as Element).tagName : null;
      if (tag === "STRONG" || tag === "B") {
        current = { label: (child.textContent ?? "").replace(/:\s*$/, "").trim(), value: "" };
        headers.push(current);
      } else if (tag === "BR") {
        current = null;
      } else if (tag) {
        walk(child);
      } else if (current) {
        current.value += child.textContent ?? "";
      }
    }
  };
  walk(divider);
  return headers;
}

/// Gives each picture in the message's own lines the attachment it is shown from, and takes out
/// every picture the editor cannot hold: one on the web, or one whose part the core could not
/// resolve. Pictures inside a quote or a signature stay in that region's markup, as they always do.
function adoptPictures(body: HTMLElement, attachments: Attachments): void {
  for (const image of Array.from(body.querySelectorAll("img"))) {
    if (image.closest(".allodia-quote, .allodia-signature")) continue;
    const src = image.getAttribute("src") ?? "";
    if (!src.startsWith("data:") || !isShowableImage(mediaTypeOf(src))) {
      image.remove();
      continue;
    }
    image.dataset.attachmentId = attachments.addCapturedImage({
      data_url: src,
      file_name: "",
      media_type: "",
    });
  }
}

/// A `<pre>`'s lines, as the `<br>`s every other line break in the reader is.
function breakPreformattedLines(body: HTMLElement): void {
  for (const pre of Array.from(body.querySelectorAll("pre"))) {
    if (pre.closest(".allodia-quote, .allodia-signature")) continue;
    const walker = pre.ownerDocument.createTreeWalker(pre, 4);
    const texts: Text[] = [];
    for (let node = walker.nextNode(); node; node = walker.nextNode()) texts.push(node as Text);
    for (const text of texts) {
      const lines = (text.nodeValue ?? "").split("\n");
      if (lines.length < 2) continue;
      const parts: Node[] = [];
      lines.forEach((line, index) => {
        if (index > 0) parts.push(pre.ownerDocument.createElement("br"));
        if (line) parts.push(pre.ownerDocument.createTextNode(line));
      });
      text.replaceWith(...parts);
    }
  }
}

/// Collapses whitespace the way a browser lays it out, so the indentation of another client's
/// markup is not read as text: runs of it become one space, and a run with no words that sits
/// between two blocks is not a line at all. A quote's and a signature's markup are left alone.
function collapseWhitespace(body: HTMLElement): void {
  const walker = body.ownerDocument.createTreeWalker(body, 4);
  const texts: Text[] = [];
  for (let node = walker.nextNode(); node; node = walker.nextNode()) texts.push(node as Text);
  for (const text of texts) {
    if (text.parentElement?.closest(".allodia-quote, .allodia-signature")) continue;
    const collapsed = (text.nodeValue ?? "").replace(/[\t\n\r ]+/g, " ");
    if (collapsed === " " && betweenBlocks(text)) text.remove();
    else text.nodeValue = collapsed;
  }
}

function betweenBlocks(text: Text): boolean {
  const isBlock = (node: Node | null) =>
    node === null ||
    (node.nodeType === 1 &&
      /^(ADDRESS|ARTICLE|ASIDE|BLOCKQUOTE|BR|CENTER|DD|DIV|DL|DT|FIGCAPTION|FIGURE|FOOTER|H[1-6]|HEADER|HR|LI|MAIN|NAV|OL|P|PRE|SECTION|TABLE|TBODY|TD|TH|THEAD|TFOOT|TR|UL)$/.test(
        (node as Element).tagName,
      )) ||
    (node.nodeType === 1 && (node as Element).classList.contains("allodia-quote"));
  return isBlock(text.previousSibling) || isBlock(text.nextSibling);
}

/// One block made tidy the way the editor's own would be: no space a browser would not have shown
/// at either end of a line, and no two neighbouring runs that differ in nothing but being two.
function tidy(block: Block): Block {
  if ("Paragraph" in block) return { Paragraph: { content: tidyLine(block.Paragraph.content) } };
  if ("List" in block) {
    const list = (value: typeof block.List): typeof block.List => ({
      kind: value.kind,
      items: value.items.map((item) => ({
        content: tidyLine(item.content),
        child: item.child ? list(item.child) : null,
      })),
    });
    return { List: list(block.List) };
  }
  if ("Table" in block) {
    return {
      Table: {
        rows: block.Table.rows.map((row) => ({
          cells: row.cells.map((cell) => ({ content: tidyLine(cell.content) })),
        })),
      },
    };
  }
  return block;
}

function tidyLine(content: InlineContent[]): InlineContent[] {
  const merged: InlineContent[] = [];
  for (const inline of content) {
    const last = merged[merged.length - 1];
    if (last && "Text" in last && "Text" in inline && sameMarks(last.Text, inline.Text)) {
      merged[merged.length - 1] = { Text: { ...last.Text, text: last.Text.text + inline.Text.text } };
    } else {
      merged.push(inline);
    }
  }
  const first = merged[0];
  if (first && "Text" in first) first.Text.text = first.Text.text.replace(/^ +/, "");
  const last = merged[merged.length - 1];
  if (last && "Text" in last) last.Text.text = last.Text.text.replace(/ +$/, "");
  return merged.filter((inline) => !("Text" in inline) || inline.Text.text !== "");
}

function sameMarks(a: TextRun, b: TextRun): boolean {
  return (
    a.bold === b.bold &&
    a.italic === b.italic &&
    a.underline === b.underline &&
    a.font_size === b.font_size &&
    a.color === b.color &&
    a.highlight === b.highlight &&
    a.link === b.link
  );
}

/// The words of `element` with its lines kept apart: what a quote's or a signature's `text/plain`
/// half carries.
export function plainText(element: Element): string {
  const lines: string[] = [""];
  const breakLine = () => {
    if (lines[lines.length - 1] !== "") lines.push("");
  };
  const walk = (node: Node) => {
    if (node.nodeType === 3) {
      lines[lines.length - 1] += node.nodeValue ?? "";
      return;
    }
    if (node.nodeType !== 1) return;
    const tag = (node as Element).tagName;
    if (tag === "BR") {
      lines.push("");
      return;
    }
    const block = /^(P|DIV|LI|TR|H[1-6]|BLOCKQUOTE|PRE|TABLE|UL|OL)$/.test(tag);
    if (block) breakLine();
    for (const child of Array.from(node.childNodes)) walk(child);
    if (block) breakLine();
  };
  for (const child of Array.from(element.childNodes)) walk(child);
  return lines
    .map((line) => line.replace(/[\t\r ]+/g, " ").trim())
    .join("\n")
    .replace(/\n{3,}/g, "\n\n")
    .trim();
}
