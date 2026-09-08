// Reading the editor's DOM back out as a `ComposerDocument`: the JSON `composerDocument()`
// returns and Rust validates, renders and sends.

import { elementMarks } from "./marks";
import { quoteBlock } from "./quote";
import { signatureBlock } from "./signature";
import {
  type Block,
  type InlineContent,
  type ListItem,
  type ListValue,
  type Marks,
  type TextRun,
  emptyMarks,
} from "./types";

// contenteditable stores a typed space as U+00A0 wherever a plain one would collapse;
// the document model only ever wants the plain one.
function normalizeText(value: string): string {
  return value.replace(/\u00A0/g, " ");
}

function inlineText(text: string, marks: Marks): InlineContent {
  const run: TextRun = {
    text: normalizeText(text),
    bold: marks.bold,
    italic: marks.italic,
    underline: marks.underline,
  };
  // Omitted rather than sent as null: Rust defaults them, and the discard probe diffs this
  // document on every close, so absent keys keep an untouched draft byte-identical to its seed.
  if (marks.size) run.font_size = marks.size;
  if (marks.color) run.color = marks.color;
  if (marks.highlight) run.highlight = marks.highlight;
  return { Text: run };
}

function inlineImage(element: HTMLElement): InlineContent | null {
  const id = element.dataset.attachmentId;
  if (!id) return null;
  const width = Number.parseInt(element.getAttribute("width") || element.style.width, 10);
  return {
    Image: {
      attachment_id: id,
      alt_text: element.getAttribute("alt") || "",
      width_px: Number.isFinite(width) && width > 0 ? width : null,
    },
  };
}

function inlinesFrom(node: Node, marks: Marks): InlineContent[] {
  if (node.nodeType === 3) {
    return node.nodeValue ? [inlineText(node.nodeValue, marks)] : [];
  }
  if (node.nodeType !== 1) return [];
  const element = node as HTMLElement;
  if (element.tagName === "BR") {
    // A line break, not body text: `leafParagraphs` splits on these, so one reached here (nested
    // inside a span, say) contributes no inline content.
    return [];
  }
  if (element.tagName === "IMG") {
    const image = inlineImage(element);
    return image ? [image] : [];
  }
  const nextMarks = elementMarks(element, marks);
  return Array.from(element.childNodes).flatMap((child) => inlinesFrom(child, nextMarks));
}

function blockInlines(element: Element): InlineContent[] {
  return Array.from(element.childNodes).flatMap((child) => inlinesFrom(child, emptyMarks()));
}

function isBr(node: Node): boolean {
  return node.nodeType === 1 && (node as Element).tagName === "BR";
}

/// Emits a leaf line as one paragraph per `<br>`-separated run, so a soft break (Shift+Enter, or a
/// `<br>` in pasted text) becomes a real line break instead of gluing the two sides into one run.
/// A single trailing `<br>` is `contenteditable`'s placeholder for an empty or last line and is
/// dropped, so it adds no spurious paragraph.
function leafParagraphs(element: Element, blocks: Block[]): void {
  const children = Array.from(element.childNodes);
  if (children.length > 0 && isBr(children[children.length - 1]!)) children.pop();
  let content: InlineContent[] = [];
  const flush = () => {
    blocks.push({ Paragraph: { content } });
    content = [];
  };
  for (const child of children) {
    if (isBr(child)) flush();
    else content.push(...inlinesFrom(child, emptyMarks()));
  }
  flush();
}

/// A `<ul>`/`<ol>` as a `List`: its kind, plus one item per `<li>`. An `<li>` may carry inline
/// content AND a nested list, which becomes that item's `child` (recursively), so sub-lists stay
/// structured rather than flattened into their parent.
export function listValue(element: Element): ListValue {
  return {
    kind: element.tagName === "OL" ? "Ordered" : "Bullet",
    items: Array.from(element.children)
      .filter((child) => child.tagName === "LI")
      .map(listItem),
  };
}

function listItem(li: Element): ListItem {
  const content: InlineContent[] = [];
  let child: ListValue | null = null;
  for (const node of Array.from(li.childNodes)) {
    if (node.nodeType === 1 && ((node as Element).tagName === "UL" || (node as Element).tagName === "OL")) {
      child = listValue(node as Element);
    } else {
      content.push(...inlinesFrom(node, emptyMarks()));
    }
  }
  return { content, child };
}

/// This table's own rows only: direct `<tr>` children and those under a direct
/// `<tbody>`/`<thead>`/`<tfoot>`. A descendant query would hoist a nested table's rows (a table
/// inserted inside a cell) into this one and corrupt its shape.
export function tableRows(table: Element): Element[] {
  const rows: Element[] = [];
  for (const child of Array.from(table.children)) {
    if (child.tagName === "TR") {
      rows.push(child);
    } else if (child.tagName === "TBODY" || child.tagName === "THEAD" || child.tagName === "TFOOT") {
      for (const row of Array.from(child.children)) {
        if (row.tagName === "TR") rows.push(row);
      }
    }
  }
  return rows;
}

function tableBlock(element: Element): Block {
  return {
    Table: {
      rows: tableRows(element).map((row) => ({
        cells: Array.from(row.children)
          .filter((cell) => cell.tagName === "TD" || cell.tagName === "TH")
          .map((cell) => ({ content: blockInlines(cell) })),
      })),
    },
  };
}

const BLOCK_TAGS = new Set(["DIV", "P", "UL", "OL", "TABLE", "BLOCKQUOTE", "SECTION", "ARTICLE"]);

function hasBlockChild(element: Element): boolean {
  return Array.from(element.children).some((child) => BLOCK_TAGS.has(child.tagName));
}

/// Walks a container's children into a flat list of paragraph/list/table blocks.
///
/// `contenteditable` wraps lines (and often lists and tables) in `<div>`s, so a list or table can
/// be nested rather than a direct editor child; recursing into block wrappers keeps them
/// structured instead of flattening them into one paragraph. Adjacent inline nodes (text, `<b>`,
/// `<img>`, …) accumulate into a single paragraph.
function collectBlocks(container: Element, blocks: Block[]): void {
  let inlineRun: Node[] = [];
  const flush = () => {
    if (inlineRun.length === 0) return;
    const content = inlineRun.flatMap((node) => inlinesFrom(node, emptyMarks()));
    if (content.length > 0) blocks.push({ Paragraph: { content } });
    inlineRun = [];
  };

  for (const child of Array.from(container.childNodes)) {
    const element = child.nodeType === 1 ? (child as HTMLElement) : null;
    const tag = element?.tagName ?? null;

    // A quoted original is its own block: never recursed into as a generic `<div>`, which would
    // flatten the quoted body into loose paragraphs.
    if (element?.classList.contains("allodia-quote")) {
      flush();
      blocks.push(quoteBlock(element));
      continue;
    }
    // The signature likewise: its own block, emitted verbatim. Recursing would scatter it into
    // paragraphs and lose the `-- ` delimiter the text part needs.
    if (element?.classList.contains("allodia-signature")) {
      flush();
      blocks.push(signatureBlock(element));
      continue;
    }

    if (tag === "UL" || tag === "OL") {
      flush();
      blocks.push({ List: listValue(element!) });
    } else if (tag === "TABLE") {
      flush();
      blocks.push(tableBlock(element!));
    } else if (tag === "DIV" || tag === "P") {
      flush();
      if (hasBlockChild(element!)) collectBlocks(element!, blocks);
      else leafParagraphs(element!, blocks);
    } else if (tag === "BR") {
      // A top-level `<br>` ends the current line.
      flush();
    } else {
      inlineRun.push(child);
    }
  }
  flush();
}

export function documentBlocks(editor: HTMLElement): Block[] {
  const blocks: Block[] = [];
  collectBlocks(editor, blocks);
  return blocks.length > 0 ? blocks : [{ Paragraph: { content: [] } }];
}

/// The attachment ids the emitted blocks actually reference, so the manifest can be pruned to
/// them (`Attachments.list`).
///
/// Read off the blocks rather than off the DOM: an `<img>` inside a quoted original or a
/// signature is emitted as part of that block's raw HTML, never as an `InlineContent::Image`, and
/// an inline attachment nothing references fails Rust's validation.
export function referencedAttachmentIds(blocks: Block[]): Set<string> {
  const ids = new Set<string>();
  const fromInlines = (content: InlineContent[]) => {
    for (const inline of content) {
      if ("Image" in inline) ids.add(inline.Image.attachment_id);
    }
  };
  const fromList = (list: ListValue) => {
    for (const item of list.items) {
      fromInlines(item.content);
      if (item.child) fromList(item.child);
    }
  };
  for (const block of blocks) {
    if ("Paragraph" in block) fromInlines(block.Paragraph.content);
    else if ("List" in block) fromList(block.List);
    else if ("Table" in block) {
      for (const row of block.Table.rows) for (const cell of row.cells) fromInlines(cell.content);
    }
  }
  return ids;
}
