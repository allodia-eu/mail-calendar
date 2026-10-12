// Reading a stored message body back into the editor's document: a draft reopened from Drafts, or a
// message moved back out of the Outbox.
//
// What this app wrote reads back as the document it was rendered from: the core's renderer and
// this reader are held to each other by fixtures both test suites read (`tests/fixtures/`), one of
// them generated across the whole schema and passed through the core's own save and reopen path.
// Another client's markup is read as closely as the editor's closed schema can hold it, and what it
// cannot hold is dropped here rather than shown, because the composer saves what it opened with: a
// construct drawn on screen but absent from the document would vanish from the draft on the next
// save without the user having touched it.
//
// The HTML arrives sanitised by the core, its `cid:` pictures already turned into `data:` URIs
// (`docs/composer-security.md`, Gate 17). It is parsed into a document of its own, which has no
// window: nothing in it runs, loads or lays out, and none of it is ever attached to the editor.
// Only the blocks read off it are, rebuilt by `blocks_dom.ts`. The two raw-HTML regions, a quoted
// original and a signature, keep their markup exactly as it arrived (Gate 10).

import { type Attachments, mediaTypeOf } from "./attachments";
import { canonicalLine, documentBlocks } from "./document";
import { windowOf } from "./dom";
import { isShowableImage } from "./images";
import { isPreformatted } from "./plain_text";
import { quoteContainer } from "./quote";
import type { Block, InlineContent, QuoteHeader } from "./types";

/// The blocks `html` holds, for the composer `editor`. Pictures it carries as `data:` URIs are added
/// to `attachments`, and the blocks reference them by the ids minted there.
export function blocksFromHtml(editor: HTMLElement, html: string, attachments: Attachments): Block[] {
  const parsed = new (windowOf(editor).DOMParser)().parseFromString(html, "text/html");
  const body = parsed.body;
  readQuotes(body);
  for (const node of Array.from(body.querySelectorAll(INERT_TAGS))) {
    if (!inRegion(node)) node.remove();
  }
  adoptPictures(body, attachments);
  breakPreformattedLines(body);
  collapseWhitespace(body);
  return documentBlocks(body).map(tidy);
}

/// Elements that are not message content. The core's sanitiser has already taken out what could
/// run; these are what it keeps that the reader must still not read as text. Inside a quoted
/// original or a signature they are that region's markup, and stay.
const INERT_TAGS = "style, title, template, noscript, script";

const REGIONS = ".allodia-quote, .allodia-signature";

function inRegion(node: Node): boolean {
  return !!node.parentElement?.closest(REGIONS);
}

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

/// Turns each quoted original into a quote region, outermost first and in one pass, so a message
/// quoted inside it, which carries its own quote and perhaps its own signature, stays part of the
/// quoted markup rather than being read as a second region of this message.
///
/// The two shapes this app's renderer writes come back with their attribution. A quote another
/// client wrote is any `<blockquote>` not inside a list or a table; its attribution, if it has one,
/// is ordinary text above it and stays that way.
function readQuotes(body: HTMLElement): void {
  for (const element of Array.from(body.querySelectorAll("blockquote, div"))) {
    if (element.closest(REGIONS)) continue;
    if (element.tagName === "BLOCKQUOTE") {
      if (styleOf(element).includes(OWN_INDENTED_BORDER)) {
        const attribution = element.previousElementSibling;
        let line = "";
        if (attribution?.tagName === "P") {
          line = attribution.textContent ?? "";
          attribution.remove();
        }
        replaceWithQuote(element, { style: "Indented", line, headers: [] }, element);
      } else if (!element.parentElement?.closest("li, td, th")) {
        replaceWithQuote(element, { style: "Indented", line: "", headers: [] }, element);
      }
    } else if (styleOf(element).includes(OWN_HEADER_DIVIDER)) {
      const quoted = element.nextElementSibling;
      if (quoted?.tagName !== "DIV") continue;
      const headers = headersOf(element);
      element.remove();
      replaceWithQuote(quoted, { style: "LineAndHeader", line: "", headers }, quoted);
    }
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
///
/// A picture keeps the `Content-ID` its part had (`data-cid`, which the core writes beside the
/// `data:` URI it inlined), so the next save names it as the last one did.
function adoptPictures(body: HTMLElement, attachments: Attachments): void {
  for (const image of Array.from(body.querySelectorAll("img"))) {
    if (image.closest(REGIONS)) {
      // The core's note for the editor, not the region's markup, which goes back as it came.
      image.removeAttribute("data-cid");
      continue;
    }
    const src = image.getAttribute("src") ?? "";
    if (!src.startsWith("data:") || !isShowableImage(mediaTypeOf(src))) {
      image.remove();
      continue;
    }
    image.dataset.attachmentId = attachments.addCapturedImage({
      data_url: src,
      file_name: "",
      media_type: "",
      content_id: image.getAttribute("data-cid") ?? "",
    });
  }
}

/// A preformatted element's lines, as the `<br>`s every other line break in the reader is.
function breakPreformattedLines(body: HTMLElement): void {
  for (const text of textNodes(body)) {
    if (inRegion(text) || !preformatted(text)) continue;
    const lines = (text.nodeValue ?? "").split("\n");
    if (lines.length < 2) continue;
    const doc = body.ownerDocument;
    const parts: Node[] = [];
    lines.forEach((line, index) => {
      if (index > 0) parts.push(doc.createElement("br"));
      if (line) parts.push(doc.createTextNode(line));
    });
    text.replaceWith(...parts);
  }
}

/// Lays whitespace out the way a browser does, so what reads back is what was on screen.
///
/// Outside preformatted text, each run of collapsible whitespace becomes one space, and a run with
/// no words that sits at the edge of a line (the indentation between another client's blocks) is
/// not a line at all. A no-break space is not collapsible, and neither is any space in preformatted
/// text: both are marked here as hard, so that trimming a line's ends (`tidyLine`) leaves them,
/// and become ordinary spaces again once the line is read. A quote's and a signature's markup are
/// left alone.
function collapseWhitespace(body: HTMLElement): void {
  for (const text of textNodes(body)) {
    if (inRegion(text)) continue;
    const value = text.nodeValue ?? "";
    if (preformatted(text)) {
      text.nodeValue = value.replace(/[  ]/g, HARD_SPACE);
      continue;
    }
    const collapsed = value.replace(/[\t\n\r ]+/g, " ").replace(/ /g, HARD_SPACE);
    if (collapsed === " " && atLineEdge(text)) text.remove();
    else text.nodeValue = collapsed;
  }
}

/// Stands for a space no layout collapses while a line is being tidied. A private-use character,
/// so nothing a message carries can be mistaken for it.
const HARD_SPACE = "";

function textNodes(root: Node): Text[] {
  const walker = root.ownerDocument!.createTreeWalker(root, 4);
  const texts: Text[] = [];
  for (let node = walker.nextNode(); node; node = walker.nextNode()) texts.push(node as Text);
  return texts;
}

function preformatted(node: Node): boolean {
  for (let element = node.parentElement; element; element = element.parentElement) {
    if (isPreformatted(element)) return true;
    if (element.tagName === "BODY") return false;
  }
  return false;
}

/// Whether `text` meets the start or the end of a line: looking outward through the inline
/// elements around it, the first thing on one side is a block, or nothing at all. A space between
/// two formatted words is inside an element of its own with no siblings, and is still between them.
function atLineEdge(text: Text): boolean {
  return edge(text, (node) => node.previousSibling) || edge(text, (node) => node.nextSibling);
}

function edge(node: Node, step: (node: Node) => Node | null): boolean {
  for (let current: Node = node; ; ) {
    const sibling = step(current);
    if (sibling) return isBlock(sibling);
    const parent = current.parentNode;
    if (!parent || parent.nodeType !== 1 || isBlock(parent)) return true;
    current = parent;
  }
}

function isBlock(node: Node): boolean {
  if (node.nodeType !== 1) return false;
  const element = node as Element;
  return BLOCK_TAG.test(element.tagName) || element.matches(REGIONS);
}

const BLOCK_TAG =
  /^(ADDRESS|ARTICLE|ASIDE|BLOCKQUOTE|BODY|BR|CENTER|DD|DIV|DL|DT|FIGCAPTION|FIGURE|FOOTER|H[1-6]|HEADER|HR|LI|MAIN|NAV|OL|P|PRE|SECTION|TABLE|TBODY|TD|TH|THEAD|TFOOT|TR|UL)$/;

/// One block made tidy the way a browser lays it out.
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

/// A line's collapsible spaces as a browser shows them: none at either end, and never two side by
/// side, even when they belong to two differently formatted runs. Its hard spaces then become the
/// ordinary spaces the editor's document holds.
function tidyLine(content: InlineContent[]): InlineContent[] {
  const out: InlineContent[] = [];
  let afterSpace = true;
  for (const inline of content) {
    if (!("Text" in inline)) {
      out.push(inline);
      afterSpace = false;
      continue;
    }
    let text = "";
    for (const ch of inline.Text.text) {
      if (ch === " " && afterSpace) continue;
      afterSpace = ch === " ";
      text += ch;
    }
    out.push({ Text: { ...inline.Text, text } });
  }
  for (let index = out.length - 1; index >= 0; index--) {
    const inline = out[index]!;
    if (!("Text" in inline)) break;
    const text = inline.Text.text.replace(/ +$/, "");
    out[index] = { Text: { ...inline.Text, text } };
    if (text !== "") break;
  }
  return canonicalLine(
    out.map((inline) =>
      "Text" in inline ? { Text: { ...inline.Text, text: inline.Text.text.split(HARD_SPACE).join(" ") } } : inline,
    ),
  );
}
