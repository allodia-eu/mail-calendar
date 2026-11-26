// Enter in a line that a region holds directly.
//
// An engine's Enter splits the nearest block around the caret and copies that block's class onto
// the half it makes. The signature and a quoted body are regions the document reads by class, and
// either can hold its first line as bare text (a Settings signature always does). Split by the
// engine, such a region becomes two: a signature then goes out twice, each under its own `-- `,
// and a quoted body loses its second half, since `quoteBlock` reads the first `.aq-body`.
//
// So that one case is split here: the line becomes a `<div>` and is split in two, inside the region.
// The region's markup is left alone until then, so a reopened draft is the document it was saved
// from.

import { documentOf } from "./dom";

/// The regions whose own element must never be split.
const REGIONS = ".allodia-signature, .aq-body";

/// Elements that are a line or a block of their own; anything else is inline content of a line.
const BLOCKS = new Set([
  "P", "DIV", "H1", "H2", "H3", "H4", "H5", "H6", "UL", "OL", "LI", "TABLE", "TD", "TH",
  "BLOCKQUOTE", "PRE", "HR",
]);

/// Splits the line at the caret when the line is held by a region itself, first deleting what is
/// selected when the selection stays inside that region. Returns whether it did; anywhere else
/// Enter is the engine's.
export function splitRegionLine(editor: HTMLElement): boolean {
  const doc = documentOf(editor);
  const selection = doc.defaultView?.getSelection();
  if (!selection || selection.rangeCount === 0) return false;
  const caret = selection.getRangeAt(0);
  const region = lineHolder(editor, caret.startContainer);
  if (!region) return false;
  if (!caret.collapsed) {
    if (!region.contains(caret.endContainer)) return false;
    // The start keeps its place: `deleteContents` truncates the node it is in and collapses there.
    caret.deleteContents();
  }

  const anchor = childOf(region, caret.startContainer, caret.startOffset);
  const run = looseRun(anchor);
  if (run.length === 0) return false;
  const line = doc.createElement("div");
  region.insertBefore(line, run[0]!);
  for (const node of run) line.appendChild(node);

  // A caret inside one of the moved nodes still names the same point: they moved, they were not
  // copied. One between two of the region's own children is restated inside the line.
  let container = caret.startContainer;
  let offset = caret.startOffset;
  if (container === region) {
    offset = anchor && anchor === region.childNodes[caret.startOffset - 1] ? run.indexOf(anchor) + 1 : 0;
    container = line;
  }
  const after = doc.createRange();
  after.setStart(container, offset);
  after.setEnd(line, line.childNodes.length);
  const next = doc.createElement("div");
  next.appendChild(after.extractContents());
  line.after(next);
  // A half holding nothing a reader draws, white space included, has no height without its `<br>`.
  for (const half of [line, next]) {
    if (/^[\t\n\f\r ]*$/.test(half.textContent ?? "") && !half.querySelector("img, br")) {
      half.replaceChildren(doc.createElement("br"));
    }
  }

  const start = doc.createRange();
  start.setStart(next, 0);
  start.collapse(true);
  selection.removeAllRanges();
  selection.addRange(start);
  return true;
}

/// The region that is the nearest block around `node`, or `null` when a line of its own is nearer.
function lineHolder(editor: HTMLElement, node: Node): HTMLElement | null {
  let current: Node | null = node.nodeType === 1 ? node : node.parentNode;
  while (current && current !== editor) {
    const element = current as HTMLElement;
    if (element.matches(REGIONS)) return element;
    if (BLOCKS.has(element.tagName)) return null;
    current = current.parentNode;
  }
  return null;
}

/// The child of `region` that holds the point (`container`, `offset`). A point between two of its
/// children is held by the inline one before it, else by the one after it.
function childOf(region: Node, container: Node, offset: number): Node | null {
  if (container === region) {
    const before = region.childNodes[offset - 1];
    return before && !isBlock(before) ? before : (region.childNodes[offset] ?? null);
  }
  let current: Node = container;
  while (current.parentNode !== region) current = current.parentNode!;
  return current;
}

/// The run of inline siblings around `anchor`, in order.
function looseRun(anchor: Node | null): Node[] {
  if (!anchor || isBlock(anchor)) return [];
  let first = anchor;
  while (first.previousSibling && !isBlock(first.previousSibling)) first = first.previousSibling;
  const run: Node[] = [];
  for (let node: Node | null = first; node && !isBlock(node); node = node.nextSibling) run.push(node);
  return run;
}

function isBlock(node: Node): boolean {
  return node.nodeType === 1 && BLOCKS.has((node as Element).tagName);
}
