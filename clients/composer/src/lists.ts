// Indenting and outdenting list items.
//
// Written as DOM surgery rather than `execCommand("indent")`/`("outdent")` for two reasons. The
// legacy commands emit a `<blockquote>` when the caret is not in a list: which `collectBlocks`
// then flattens into loose paragraphs, silently losing the structure; and Blink and WebKit
// disagree about the nesting they produce inside one. Owning the transform makes the result
// identical on every WebView and, because it is a pure function of the tree, testable.
//
// One rule decides every case: **the items that follow the one being moved keep their own depth.**
// Nested, that means they ride along as its new sub-list; at the top level it means the list
// splits around it, because they are already at the depth they should stay at.

import {
  ancestorOf,
  caretInto,
  documentOf,
  focusEditor,
  rangeOverlaps,
  rangeWithin,
  restoreSelection,
  saveSelection,
} from "./dom";

/// A range over the item's OWN line; its content up to the first nested sub-list.
///
/// Selecting from one nested item to another also spans the *parent* item's contents, because the
/// sub-list holding them is inside it. Testing against the whole item would therefore indent the
/// parent as well and collapse the structure the user was reordering. A line is what the selection
/// has to touch, and an item's line stops where its sub-list begins.
function ownLineRange(li: HTMLLIElement): Range {
  const range = documentOf(li).createRange();
  range.selectNodeContents(li);
  const sub = Array.from(li.children).find((child) => child.tagName === "UL" || child.tagName === "OL");
  if (sub) range.setEndBefore(sub);
  return range;
}

/// The list items the selection touches, in document order. A collapsed caret yields the one item
/// it sits in; a selection spanning several yields all of them, which is what makes Tab on a
/// multi-line selection indent the whole block the way a word processor does.
export function selectedListItems(editor: HTMLElement): HTMLLIElement[] {
  const range = rangeWithin(editor);
  if (!range) return [];
  if (range.collapsed) {
    const li = ancestorOf(range.startContainer, editor, "li");
    return li ? [li] : [];
  }
  return Array.from(editor.querySelectorAll("li")).filter((li) =>
    rangeOverlaps(range, ownLineRange(li)),
  );
}

function isListTag(element: Element | null | undefined): element is HTMLElement {
  return !!element && (element.tagName === "UL" || element.tagName === "OL");
}

/// The `<ul>`/`<ol>` nested at the end of `li`, if it has one.
function trailingList(li: Element): HTMLElement | null {
  const last = li.lastElementChild;
  return isListTag(last) ? last : null;
}

function createList(like: HTMLElement): HTMLElement {
  return documentOf(like).createElement(like.tagName.toLowerCase()) as HTMLElement;
}

/// Every sibling after `li`, detached from the list in order.
function takeFollowing(li: HTMLLIElement): Element[] {
  const following: Element[] = [];
  for (let node = li.nextElementSibling; node; ) {
    const next = node.nextElementSibling;
    following.push(node);
    node = next;
  }
  return following;
}

/// Moves `li` one level deeper: it becomes the last item of a sub-list inside the item above it.
///
/// The first item of a list cannot indent; there is nothing to nest it under; which is what a
/// word processor does too. Returns whether anything moved.
///
/// Indenting a run of consecutive items works by iterating them in document order: once the first
/// has moved into the previous item's sub-list, the next one's previous sibling is that same item,
/// so it lands in the same sub-list, in order.
export function indentItem(li: HTMLLIElement): boolean {
  const list = li.parentElement;
  if (!isListTag(list)) return false;
  const previous = li.previousElementSibling;
  if (!previous || previous.tagName !== "LI") return false;

  // Reuse the item's existing sub-list whatever its kind; the user made that sub-list under this
  // item, and starting a second one beside it would read as two lists where they see one.
  let nested = trailingList(previous);
  if (!nested) {
    nested = createList(list);
    previous.appendChild(nested);
  }
  nested.appendChild(li);
  return true;
}

/// Moves `li` one level shallower. Returns whether anything moved.
export function outdentItem(li: HTMLLIElement): boolean {
  const list = li.parentElement;
  if (!isListTag(list)) return false;
  const doc = documentOf(li);
  const parentItem = list.parentElement;
  const following = takeFollowing(li);

  if (parentItem?.tagName === "LI" && parentItem.parentElement) {
    // Nested: the followers are one level deeper than this item's new home, so they become its
    // sub-list and their depth is unchanged.
    if (following.length > 0) {
      const sub = trailingList(li) ?? createList(list);
      if (!sub.parentElement) li.appendChild(sub);
      for (const node of following) sub.appendChild(node);
    }
    parentItem.parentElement.insertBefore(li, parentItem.nextSibling);
    if (list.children.length === 0) list.remove();
    return true;
  }

  // Top level: out of the list altogether. The followers are already at the depth they should keep,
  // so they stay list items; the list splits around the paragraph this item becomes.
  const tail = createList(list);
  for (const node of following) tail.appendChild(node);

  // This item's own sub-list has nothing left to nest under, so it is promoted to a list of its own.
  const promoted = trailingList(li);
  promoted?.remove();

  const paragraph = doc.createElement("p");
  while (li.firstChild) paragraph.appendChild(li.firstChild);
  if (paragraph.childNodes.length === 0) paragraph.appendChild(doc.createElement("br"));

  const parent = list.parentElement;
  if (!parent) return false;
  let anchor: Element = list;
  const place = (node: Element) => {
    parent.insertBefore(node, anchor.nextSibling);
    anchor = node;
  };
  place(paragraph);
  if (promoted) place(promoted);
  if (tail.children.length > 0) place(tail);

  li.remove();
  if (list.children.length === 0) list.remove();
  caretInto(paragraph, false);
  return true;
}

/// Indents (or outdents) every list item the selection touches, in document order. Returns whether
/// the caret was in a list at all; the caller uses that to decide whether Tab should fall through
/// to its other meanings (the next table cell, or a literal tab).
///
/// The caret is captured and re-applied around the edit: moving a list item is a reparent, and
/// WebKit drops the selection when a node is reparented (see `saveSelection`). Without this the
/// caret lands on the line above the one the user just indented.
export function indentSelection(editor: HTMLElement, outdent: boolean): boolean {
  const items = selectedListItems(editor);
  if (items.length === 0) return false;
  const caret = saveSelection(editor);
  for (const li of items) {
    if (outdent) outdentItem(li);
    else indentItem(li);
  }
  restoreSelection(editor, caret);
  // The toolbar path cancels its own mousedown so focus never leaves, but a host that moves focus
  // for its own reasons would otherwise leave a restored selection nobody can type into.
  focusEditor(editor);
  return true;
}
