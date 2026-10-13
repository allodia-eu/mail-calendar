// Reading a stored message body back into the editor's document: a draft reopened from Drafts, or a
// message moved back out of the Outbox.
//
// What this app wrote reads back as the document it was rendered from: the core's renderer and
// this reader are held to each other by fixtures both test suites read (`tests/fixtures/`), one of
// them generated across the whole schema and passed through the core's own save and reopen path.
// What Thunderbird and Outlook write reads back as they show it, held by the drafts each of them
// stored (`tests/fixtures/interop/`). Any other markup is read as closely as the editor's closed
// schema can hold it, and what it cannot hold is dropped here rather than shown, because the
// composer saves what it opened with: a construct drawn on screen but absent from the document
// would vanish from the draft on the next save without the user having touched it.
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
import { readQuotes, readSignatures, REGIONS } from "./read_regions";
import type { Block, InlineContent } from "./types";

/// The blocks `html` holds, for the composer `editor`. Pictures it carries as `data:` URIs are added
/// to `attachments`, and the blocks reference them by the ids minted there.
export function blocksFromHtml(editor: HTMLElement, html: string, attachments: Attachments): Block[] {
  const parsed = new (windowOf(editor).DOMParser)().parseFromString(html, "text/html");
  const body = parsed.body;
  readQuotes(body);
  readSignatures(body);
  for (const node of Array.from(body.querySelectorAll(INERT_TAGS))) {
    if (!inRegion(node)) node.remove();
  }
  adoptPictures(body, attachments);
  adoptStrayLists(body);
  emptyWordBlankLines(body);
  liftRules(body);
  breakPreformattedLines(body);
  collapseWhitespace(body);
  return documentBlocks(body).map(tidy);
}

/// Elements that are not message content. The core's sanitiser has already taken out what could
/// run; these are what it keeps that the reader must still not read as text. Inside a quoted
/// original or a signature they are that region's markup, and stay.
const INERT_TAGS = "style, title, template, noscript, script";

function inRegion(node: Node): boolean {
  return !!node.parentElement?.closest(REGIONS);
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

/// A list written directly inside another list rather than inside one of its items, which is how
/// Thunderbird and Outlook both nest a list: it belongs to the item above it, which is where a
/// browser draws it.
function adoptStrayLists(body: HTMLElement): void {
  for (const list of Array.from(body.querySelectorAll("ul > ul, ul > ol, ol > ul, ol > ol"))) {
    if (inRegion(list)) continue;
    let item = list.previousElementSibling;
    if (item?.tagName !== "LI") {
      item = list.ownerDocument.createElement("li");
      list.before(item);
    }
    item.appendChild(list);
  }
}

/// Word writes a blank line as a paragraph holding one no-break space (`<o:p>&nbsp;</o:p>`, whose
/// wrapper the sanitiser has already dropped). It is drawn as an empty line, so it reads as one.
function emptyWordBlankLines(body: HTMLElement): void {
  for (const line of Array.from(body.querySelectorAll("p[class^='Mso'], div[class^='Mso']"))) {
    if (inRegion(line) || line.querySelector("img, hr, table, ul, ol")) continue;
    if ((line.textContent ?? "").replace(/[\s ]/g, "") === "") line.textContent = "";
  }
}

/// A rule another client wrapped in a line of its own (Word puts it in a centred paragraph and a
/// span), lifted out to stand where that line stood, which is how it is drawn.
function liftRules(body: HTMLElement): void {
  for (const rule of Array.from(body.querySelectorAll("hr"))) {
    if (inRegion(rule)) continue;
    let holder: Element = rule;
    for (let parent = rule.parentElement; parent && parent !== body; parent = parent.parentElement) {
      if ((parent.textContent ?? "").trim() !== "" || parent.querySelectorAll("img, hr").length > 1) break;
      if (parent.tagName === "LI" || parent.tagName === "TD" || parent.tagName === "TH") break;
      holder = parent;
    }
    if (holder !== rule) holder.replaceWith(rule);
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
      text.nodeValue = value.replace(/[ \u00A0]/g, HARD_SPACE);
      continue;
    }
    const collapsed = value.replace(/[\t\n\r ]+/g, " ").replace(/\u00A0/g, HARD_SPACE);
    if (collapsed === " " && atLineEdge(text)) text.remove();
    else text.nodeValue = collapsed;
  }
}

/// Stands for a space no layout collapses while a line is being tidied. A private-use character,
/// so nothing a message carries can be mistaken for it.
const HARD_SPACE = "\uE000";

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
  if ("Paragraph" in block) return { Paragraph: { ...block.Paragraph, content: tidyLine(block.Paragraph.content) } };
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
