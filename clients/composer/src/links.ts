// Links in the message body: reading a link's target, making and removing one from the toolbar,
// and turning an address the user has just typed into one.
//
// In the editor a link is an `<a href>`; in the document it is the `link` on each run it covers.
// Rust checks every target again on submit (`mailcal_composer::LinkUrl`), so what this file
// refuses is the editor keeping its own promise, not the gate.

import {
  ancestorOf,
  documentOf,
  focusEditor,
  rangeTouches,
  rangeWithin,
  restoreSelection,
  type SavedSelection,
} from "./dom";

/// `mailcal_composer::LINK_SCHEMES`: the schemes a sent link may carry.
const SCHEMES = ["http:", "https:", "mailto:"];

/// The prefixes an address typed into the body starts with, and whether it opens over `https`.
/// The same four the reading view's linkifier recognises (`mailcal_app::linkify`), so what the user
/// sees become a link as they type is what their recipient's copy in this app shows as one.
const PREFIXES: [string, boolean][] = [
  ["https://", false],
  ["http://", false],
  ["mailto:", false],
  ["www.", true],
];

/// The sentence's punctuation, which an address may contain but does not end with.
const TRAILING = new Set([".", ",", ";", ":", "!", "?", "'", "*", "_"]);

/// The target of an `<a href>` in the editor, or `null` when it is not one this product sends.
export function safeLinkHref(value: string | null | undefined): string | null {
  const href = (value ?? "").trim();
  if (!href || /[\s\u0000-\u001f\u007f]/.test(href)) return null;
  const lower = href.toLowerCase();
  const scheme = SCHEMES.find((prefix) => lower.startsWith(prefix));
  if (!scheme) return null;
  return href.slice(scheme.length).replace(/^\/+/, "") ? href : null;
}

/// What the user typed into the link editor's address field, as a target: an address with a
/// scheme is kept when the scheme is allowed, an email address becomes `mailto:`, and a host
/// becomes `https://`. `null` when it cannot be a link.
export function normalizeLinkAddress(typed: string): string | null {
  const value = typed.trim();
  if (!value || /\s/.test(value)) return null;
  const scheme = /^([a-z][a-z0-9+.-]*):/i.exec(value);
  // `example.com:8080/x` reads as a scheme to that pattern; a dot says it is a host.
  if (scheme && !scheme[1]!.includes(".")) return safeLinkHref(value);
  if (/^[^@/:]+@[^@/:]+\.[^@/:]+$/.test(value)) return `mailto:${value}`;
  const host = value.replace(/^\/\//, "");
  if (!/^[\p{L}\p{N}][^/?#]*\.[^/?#]/u.test(host)) return null;
  return safeLinkHref(`https://${host}`);
}

/// The link the selection sits in, when all of it is inside one.
export function linkAtCaret(editor: HTMLElement): HTMLAnchorElement | null {
  const range = rangeWithin(editor);
  if (!range) return null;
  const start = ancestorOf(range.startContainer, editor, "a");
  return start && start === ancestorOf(range.endContainer, editor, "a") ? start : null;
}

/// The links a selection touches, for removing them. A bare caret touches only the link it is in,
/// not one it merely sits beside.
function linksTouched(editor: HTMLElement, range: Range): HTMLAnchorElement[] {
  const around = ancestorOf(range.startContainer, editor, "a");
  if (range.collapsed) return around ? [around] : [];
  const inside = Array.from(editor.querySelectorAll("a")).filter((a) => rangeTouches(range, a));
  return around && !inside.includes(around) ? [around, ...inside] : inside;
}

/// Whether the selection saved at `saved` touches a link: the editor offers to remove one then.
export function selectionHasLink(editor: HTMLElement, saved: SavedSelection | null): boolean {
  if (!saved) return false;
  const range = documentOf(editor).createRange();
  range.setStart(saved.startNode, saved.startOffset);
  range.setEnd(saved.endNode, saved.endOffset);
  return linksTouched(editor, range).length > 0;
}

/// What the link editor applies.
export interface LinkEdit {
  /// The target, already through `normalizeLinkAddress`.
  href: string;
  /// The words to show. Empty keeps what the selection or link already shows.
  text: string;
}

/// Makes or changes a link where the selection was when the editor opened.
///
/// Editing an existing link changes its target, and its words when the user changed them. A
/// selection becomes a link over the text it covers, marks and all, unless the user typed other
/// words, which replace it. A caret alone inserts the words (or the address) as a new link.
export function applyLink(
  editor: HTMLElement,
  saved: SavedSelection | null,
  existing: HTMLAnchorElement | null,
  edit: LinkEdit,
): void {
  focusEditor(editor);
  restoreSelection(editor, saved);
  const doc = documentOf(editor);
  if (existing && editor.contains(existing)) {
    existing.setAttribute("href", edit.href);
    if (edit.text && edit.text !== existing.textContent) existing.textContent = edit.text;
    caretAfter(existing);
    return;
  }
  const range = rangeWithin(editor);
  const selected = range?.toString() ?? "";
  if (range && !range.collapsed && (!edit.text || edit.text === selected)) {
    wrapRange(editor, range, edit.href);
    return;
  }
  const link = doc.createElement("a");
  link.setAttribute("href", edit.href);
  link.textContent = edit.text || displayed(edit.href);
  if (range) {
    range.deleteContents();
    range.insertNode(link);
  } else {
    editor.appendChild(link);
  }
  caretAfter(link);
}

/// Wraps a selection in a link. `createLink` splits a selection across blocks and marks the way
/// the engines already do correctly; the fallback covers an engine without it.
function wrapRange(editor: HTMLElement, range: Range, href: string): void {
  const doc = documentOf(editor) as Document & {
    execCommand?: (command: string, showUi?: boolean, value?: string) => boolean;
  };
  if (doc.execCommand?.("createLink", false, href)) return;
  const link = doc.createElement("a");
  link.setAttribute("href", href);
  link.appendChild(range.extractContents());
  range.insertNode(link);
  caretAfter(link);
}

/// Removes the link the caret is in, or every link the selection touches, keeping their words.
export function removeLink(editor: HTMLElement, saved: SavedSelection | null): void {
  focusEditor(editor);
  restoreSelection(editor, saved);
  const range = rangeWithin(editor);
  if (!range) return;
  for (const link of linksTouched(editor, range)) {
    const parent = link.parentNode;
    if (!parent) continue;
    while (link.firstChild) parent.insertBefore(link.firstChild, link);
    link.remove();
  }
}

/// The words a link inserted at a bare caret shows: its address, less a `mailto:`.
function displayed(href: string): string {
  return href.toLowerCase().startsWith("mailto:") ? href.slice("mailto:".length) : href;
}

/// Puts the caret just after `node`, outside it, so what is typed next is not part of the link.
function caretAfter(node: Node): void {
  const doc = documentOf(node);
  const selection = doc.defaultView?.getSelection();
  if (!selection || !node.parentNode) return;
  const range = doc.createRange();
  range.setStartAfter(node);
  range.collapse(true);
  selection.removeAllRanges();
  selection.addRange(range);
}

/// The address at the end of `word`, less the sentence's punctuation, and the target it opens.
export function typedAddress(word: string): { address: string; href: string } | null {
  const lower = word.toLowerCase();
  const match = PREFIXES.find(([prefix]) => lower.startsWith(prefix));
  if (!match) return null;
  const [prefix, secure] = match;
  const address = trimTrailing(word);
  const body = address.slice(prefix.length);
  if (address.length <= prefix.length) return null;
  const plausible =
    prefix === "mailto:"
      ? /^[^@]+@[^@]+$/.test(body)
      : prefix === "www."
        ? /^[\p{L}\p{N}][^/?#]*\.[^/?#]/u.test(body)
        : /^[\p{L}\p{N}]/u.test(body);
  if (!plausible) return null;
  const href = safeLinkHref(secure ? `https://${address}` : address);
  return href ? { address, href } : null;
}

/// Drops trailing punctuation, and a closing bracket the address did not open, so
/// `(see https://example.com/a)` links only the address.
function trimTrailing(word: string): string {
  let end = word;
  for (;;) {
    const last = end.slice(-1);
    const count = (char: string) => end.split(char).length - 1;
    const drop =
      TRAILING.has(last) ||
      (last === ")" && count("(") < count(")")) ||
      (last === "]" && count("[") < count("]"));
    if (!drop) return end;
    end = end.slice(0, -1);
  }
}

/// Turns the address the user has just finished typing into a link, when the word before the
/// caret is one. `typed` is the space the user pressed, which this inserts after the link itself
/// (so it is not swallowed into it), or `null` for Enter, whose default then runs as usual.
/// Returns whether it made a link; the caller cancels the key's default only for a space.
///
/// Only the word that ends at the caret, inside one text node and outside any existing link, is
/// considered: an address split across formatting, or one the user is editing in the middle of, is
/// left as it is.
export function autolinkBeforeCaret(editor: HTMLElement, typed: string | null): boolean {
  const range = rangeWithin(editor);
  if (!range?.collapsed || range.startContainer.nodeType !== 3) return false;
  const node = range.startContainer as Text;
  if (ancestorOf(node, editor, "a")) return false;
  const offset = range.startOffset;
  const before = node.data.slice(0, offset);
  const start = before.search(/\S+$/);
  if (start < 0) return false;
  const found = typedAddress(before.slice(start));
  if (!found) return false;

  const doc = documentOf(editor);
  const addressNode = node.splitText(start);
  const rest = addressNode.splitText(found.address.length);
  const link = doc.createElement("a");
  link.setAttribute("href", found.href);
  addressNode.replaceWith(link);
  link.appendChild(addressNode);

  // The caret was at the end of the word; the punctuation trimmed off the address is now the
  // start of `rest`, so the caret goes after it.
  const caret = offset - start - found.address.length;
  if (typed !== null) {
    // A plain space at the end of a line collapses and the caret has nowhere to sit; the editor
    // turns a no-break space back into a space on the way out (`document.ts`).
    const atEnd = caret >= rest.data.length || rest.data[caret] === " ";
    rest.insertData(caret, typed === " " && atEnd ? " " : typed);
    placeCaret(rest, caret + typed.length);
  } else {
    placeCaret(rest, caret);
  }
  return true;
}

function placeCaret(node: Text, offset: number): void {
  const doc = documentOf(node);
  const selection = doc.defaultView?.getSelection();
  if (!selection) return;
  const range = doc.createRange();
  range.setStart(node, Math.min(offset, node.data.length));
  range.collapse(true);
  selection.removeAllRanges();
  selection.addRange(range);
}

