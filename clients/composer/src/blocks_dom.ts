// Building the editor's DOM from a document: the inverse of `document.ts`, which reads it back.
//
// The two are held to each other by a test (`blocks_dom.test.ts`): every document built here reads
// back as itself, so a body reopened in the composer is the body that saves and sends. Each node is
// the shape the toolbar itself leaves (a `<span>` stamped with `data-*` for a size or colour, the
// quote and signature regions from their own builders), so a reopened run is edited exactly like a
// typed one.

import type { Attachments } from "./attachments";
import { documentOf } from "./dom";
import { quoteContainer } from "./quote";
import { signatureContainer } from "./signature";
import { type Block, type InlineContent, type ListValue, SIZE_PX, type TableValue, type TextRun } from "./types";

/// Replaces the editor's content with `blocks`. `attachments` is where each inline picture's bytes
/// are looked up; a picture it does not hold is left out rather than drawn broken, and the document
/// then no longer references it.
export function setBlocks(editor: HTMLElement, blocks: Block[], attachments: Attachments): void {
  const doc = documentOf(editor);
  editor.innerHTML = "";
  for (const block of blocks) editor.appendChild(blockNode(doc, block, attachments));
  // The caret needs a line of the message to land on. A body that opens on its signature or its
  // quote (everything above them deleted before it was saved) gets the empty line a reply opens
  // with, or the user's first keystroke would go into that region.
  const first = editor.firstElementChild;
  if (!first || first.classList.contains("allodia-signature") || first.classList.contains("allodia-quote")) {
    const lead = doc.createElement("p");
    lead.appendChild(doc.createElement("br"));
    editor.insertBefore(lead, first);
  }
}

function blockNode(doc: Document, block: Block, attachments: Attachments): HTMLElement {
  if ("Paragraph" in block) return lineNode(doc, "p", block.Paragraph.content, attachments);
  if ("List" in block) return listNode(doc, block.List, attachments);
  if ("Table" in block) return tableNode(doc, block.Table, attachments);
  if ("Quote" in block) return quoteContainer(doc, block.Quote);
  return signatureContainer(doc, block.Signature);
}

/// An element holding one line of inline content. An empty one holds the `<br>` that gives it
/// height, which the reader drops again.
function lineNode(
  doc: Document,
  tag: string,
  content: InlineContent[],
  attachments: Attachments,
): HTMLElement {
  const element = doc.createElement(tag);
  appendInlines(element, content, attachments);
  if (!element.firstChild) element.appendChild(doc.createElement("br"));
  return element;
}

function appendInlines(parent: HTMLElement, content: InlineContent[], attachments: Attachments): void {
  const doc = documentOf(parent);
  for (const inline of content) {
    const node = "Text" in inline ? runNode(doc, inline.Text) : imageNode(doc, inline.Image, attachments);
    if (node) parent.appendChild(node);
  }
}

/// One text run, as the nested elements its marks are.
function runNode(doc: Document, run: TextRun): Node {
  let node: Node = doc.createTextNode(run.text);
  if (run.underline) node = wrap(doc, "u", node);
  if (run.italic) node = wrap(doc, "em", node);
  if (run.bold) node = wrap(doc, "strong", node);
  if (run.font_size || run.color || run.highlight) {
    const span = doc.createElement("span");
    if (run.font_size) {
      span.dataset.size = run.font_size;
      span.style.fontSize = `${SIZE_PX[run.font_size]}px`;
    }
    if (run.color) {
      span.dataset.color = run.color;
      span.style.color = run.color;
    }
    if (run.highlight) {
      span.dataset.highlight = run.highlight;
      span.style.backgroundColor = run.highlight;
    }
    span.appendChild(node);
    node = span;
  }
  if (run.link) {
    const anchor = doc.createElement("a");
    // Assigned as an attribute, not parsed: the target came out of a document Rust validates
    // (`LinkUrl`), and reading it back re-checks it (`safeLinkHref`).
    anchor.setAttribute("href", run.link);
    anchor.appendChild(node);
    node = anchor;
  }
  return node;
}

function wrap(doc: Document, tag: string, child: Node): HTMLElement {
  const element = doc.createElement(tag);
  element.appendChild(child);
  return element;
}

function imageNode(
  doc: Document,
  image: { attachment_id: string; alt_text: string; width_px?: number | null },
  attachments: Attachments,
): HTMLImageElement | null {
  const src = attachments.dataUrlOf(image.attachment_id);
  if (!src) return null;
  const node = doc.createElement("img");
  node.dataset.attachmentId = image.attachment_id;
  node.src = src;
  node.alt = image.alt_text;
  if (image.width_px) node.width = image.width_px;
  return node;
}

function listNode(doc: Document, list: ListValue, attachments: Attachments): HTMLElement {
  const element = doc.createElement(list.kind === "Ordered" ? "ol" : "ul");
  for (const item of list.items) {
    const li = doc.createElement("li");
    appendInlines(li, item.content, attachments);
    if (item.child) li.appendChild(listNode(doc, item.child, attachments));
    if (!li.firstChild) li.appendChild(doc.createElement("br"));
    element.appendChild(li);
  }
  return element;
}

function tableNode(doc: Document, table: TableValue, attachments: Attachments): HTMLElement {
  const element = doc.createElement("table");
  const body = doc.createElement("tbody");
  for (const row of table.rows) {
    const tr = doc.createElement("tr");
    for (const cell of row.cells) tr.appendChild(lineNode(doc, "td", cell.content, attachments));
    body.appendChild(tr);
  }
  element.appendChild(body);
  return element;
}
