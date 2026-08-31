// Turning a typed marker into real structure, the way a word processor does.
//
// `- ` at the start of a line becomes a bullet list: the marker disappears and the line becomes the
// first item. It is what Outlook, Word and every markdown editor do, and it is the difference
// between writing a list and reaching for the toolbar for each one.

import { ancestorOf, caretInto, documentOf, rangeWithin } from "./dom";

/// The text that turns into a bullet. Checked against the line's content *before the caret*, so it
/// only ever fires on the marker the user just finished typing.
const MARKER = "-";

/// Block-level tags whose presence means the editor's direct children are not one bare line; a
/// quoted original, a signature, or any earlier paragraph. Used only for the unwrapped case below.
const BLOCK_TAGS = new Set(["DIV", "P", "UL", "OL", "TABLE", "BLOCKQUOTE", "SECTION", "ARTICLE"]);

/// The region wrappers a bullet list may never replace. A plain-text quoted original is text held
/// directly by `.aq-body`, so the nearest block ancestor of a caret at its start IS that container:
/// replacing it takes the class `quoteBlock` reads the body from with it, and the whole quoted
/// original leaves the outgoing message. The regions' *inner* blocks are still fair game.
const REGION_CLASSES = ["allodia-quote", "aq-attr", "aq-body", "allodia-signature"];

/// Converts the current line into a bullet list when the user has just typed the marker and is
/// about to type the space after it. Returns whether it did, so the caller can swallow that space.
///
/// It fires only when the text between the start of the line and the caret is **exactly** the
/// marker, which is what keeps it out of the way: a hyphen anywhere else in a sentence, or a line
/// that already has words in front of it, is left alone. Every guard below can only make it fire
/// less often, never more: the failure mode is a literal "- ", which is what happens today.
export function autoformatBulletList(editor: HTMLElement): boolean {
  const range = rangeWithin(editor);
  if (!range?.collapsed) return false;

  // Inside a list the marker is just text the user meant to type. Inside a table cell there is
  // nowhere for a list to go: the document schema gives a cell inline content only, so a `<ul>`
  // built there would be flattened into loose text on send; worse than not firing.
  if (ancestorOf(range.startContainer, editor, "li", "td", "th")) return false;

  const doc = documentOf(editor);
  const block = ancestorOf(range.startContainer, editor, "p", "div");
  if (block && REGION_CLASSES.some((name) => block.classList.contains(name))) return false;
  const line = block ?? editor;

  // With no block wrapper the "line" is the editor itself: which is the common case for the very
  // first line of an empty composer, since `contenteditable` only starts wrapping at the first
  // Enter. Then the editor's children ARE the line, but only if it holds nothing else: a seeded
  // signature or quoted original is a block sibling, and sweeping it into the list item would eat
  // content the user never touched.
  if (!block && Array.from(editor.children).some((child) => BLOCK_TAGS.has(child.tagName))) {
    return false;
  }

  const before = doc.createRange();
  before.selectNodeContents(line);
  before.setEnd(range.startContainer, range.startOffset);
  if (before.toString() !== MARKER) return false;

  before.deleteContents();

  const list = doc.createElement("ul");
  const item = doc.createElement("li");
  // Whatever followed the caret on that line becomes the item's text.
  while (line.firstChild) item.appendChild(line.firstChild);
  // Deleting the marker can leave its text node behind, emptied; engines differ on whether a
  // fully-covered node is removed or just cleared. So "is this item empty?" has to be a question
  // about content, not about child count: an `<li>` holding one empty text node and no `<br>` has
  // no height and the caret cannot land in it, which is the whole point of the placeholder.
  for (const node of Array.from(item.childNodes)) {
    if (node.nodeType === 3 && node.nodeValue === "") node.remove();
  }
  if (item.childNodes.length === 0) item.appendChild(doc.createElement("br"));
  list.appendChild(item);

  if (block) block.replaceWith(list);
  else editor.appendChild(list);

  caretInto(item, true);
  return true;
}
