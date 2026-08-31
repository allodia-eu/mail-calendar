// Selection and tree helpers shared by the editing commands.
//
// Everything here takes the editor element and derives its document/window from it, rather than
// reaching for the `document` and `window` globals. That is what makes the commands testable: a
// test constructs a happy-dom `Window`, builds an editor in it, and calls the command directly,
// no global registration, and each test gets its own isolated DOM.

export function documentOf(node: Node): Document {
  return (node.ownerDocument ?? (node as unknown as Document)) as Document;
}

/// The editor's own window. Typed as `Window & typeof globalThis` (what `defaultView` yields), so
/// callers reach the constructors that live on the global object: `ResizeObserver` and friends,
/// rather than only the `Window` interface's own members.
export function windowOf(node: Node): Window & typeof globalThis {
  const view = documentOf(node).defaultView;
  if (!view) throw new Error("editor node is detached from a window");
  return view;
}

export function focusEditor(editor: HTMLElement): void {
  editor.focus({ preventScroll: true });
}

/// The current selection range, but only when it is inside `editor`: a caret left in the native
/// chrome around the WebView must not make a command act on the last place it happened to be.
export function rangeWithin(editor: HTMLElement): Range | null {
  const selection = windowOf(editor).getSelection();
  if (!selection || selection.rangeCount === 0) return null;
  const range = selection.getRangeAt(0);
  return editor.contains(range.startContainer) ? range : null;
}

/// The nearest ancestor of the caret matching `tags`, stopping at the editor: so a command can
/// never escape upwards into the page chrome. Returns `null` when there is no caret in the editor.
export function ancestorAtCaret<K extends keyof HTMLElementTagNameMap>(
  editor: HTMLElement,
  ...tags: K[]
): HTMLElementTagNameMap[K] | null {
  const range = rangeWithin(editor);
  return range ? ancestorOf(range.startContainer, editor, ...tags) : null;
}

/// The nearest ancestor of `node` (inclusive) whose tag is in `tags`, bounded by `editor`.
///
/// Tags are lowercase because that is how `HTMLElementTagNameMap` is keyed, which is what lets the
/// return type be the elements actually asked for: `"td", "th"` gives an `HTMLTableCellElement`,
/// `"li"` an `HTMLLIElement`: so the five table commands can require a cell rather than accept any
/// element and discover the mistake at runtime. Uppercase tags cannot do this: `Uppercase<K>` is
/// not an inference site, so `K` silently widens to every element in the map.
export function ancestorOf<K extends keyof HTMLElementTagNameMap>(
  node: Node | null,
  editor: HTMLElement,
  ...tags: K[]
): HTMLElementTagNameMap[K] | null {
  const wanted = new Set<string>(tags.map((tag) => tag.toUpperCase()));
  let current: Node | null = node;
  while (current && current !== editor) {
    if (current.nodeType === 1 && wanted.has((current as Element).tagName)) {
      return current as HTMLElementTagNameMap[K];
    }
    current = current.parentNode;
  }
  return null;
}

/// A selection captured across a structural edit.
export interface SavedSelection {
  startNode: Node;
  startOffset: number;
  endNode: Node;
  endOffset: number;
}

/// Captures the selection so a structural edit can put it back.
///
/// Reparenting a node; which is what indenting a list item is; drops the selection in WebKit,
/// collapsing it to the previous block: the caret lands on the bullet ABOVE the one that moved and
/// the user has to click back into their own line. Blink keeps it, so the same code behaves
/// differently per host, and the editor cannot rely on either. The text nodes themselves survive
/// the move, so re-applying the same (node, offset) pair afterwards restores the caret exactly.
export function saveSelection(editor: HTMLElement): SavedSelection | null {
  const range = rangeWithin(editor);
  if (!range) return null;
  return {
    startNode: range.startContainer,
    startOffset: range.startOffset,
    endNode: range.endContainer,
    endOffset: range.endOffset,
  };
}

/// Re-applies a captured selection. A no-op when the nodes did not survive the edit or the offsets
/// no longer fit them, leaving whatever caret the edit itself placed; so a command that
/// deliberately moves the caret (outdenting out of a list, which replaces the item with a
/// paragraph) still wins when its nodes are gone.
export function restoreSelection(editor: HTMLElement, saved: SavedSelection | null): void {
  if (!saved || !editor.contains(saved.startNode) || !editor.contains(saved.endNode)) return;
  const doc = documentOf(editor);
  const selection = doc.defaultView?.getSelection();
  if (!selection) return;
  const range = doc.createRange();
  try {
    range.setStart(saved.startNode, saved.startOffset);
    range.setEnd(saved.endNode, saved.endOffset);
  } catch {
    // The node survived but shrank; an offset past its end throws rather than clamping.
    return;
  }
  selection.removeAllRanges();
  selection.addRange(range);
}

/// Puts the caret inside `target`, at its start or end. Used after a structural edit so the user
/// carries on typing where they were rather than losing the caret to the top of the document.
export function caretInto(target: Node, atStart = true): void {
  const doc = documentOf(target);
  const selection = doc.defaultView?.getSelection();
  if (!selection) return;
  const range = doc.createRange();
  range.selectNodeContents(target);
  range.collapse(atStart);
  selection.removeAllRanges();
  selection.addRange(range);
}

// `Range.compareBoundaryPoints` comparison modes. Spelled out rather than read off the Range
// interface object, which not every DOM implementation exposes as a static.
const START_TO_END = 1;
const END_TO_START = 3;

/// Whether `range` overlaps `target`'s boundaries at all.
///
/// Built from `compareBoundaryPoints` rather than `Range.intersectsNode`, which is missing from
/// enough implementations that an optional call would silently answer "no": and a command that
/// reads "no" for every node either does nothing or, worse, falls back to acting on everything.
export function rangeOverlaps(range: Range, target: Range): boolean {
  // Overlap is `range.start <= target.end && range.end >= target.start`.
  return (
    range.compareBoundaryPoints(END_TO_START, target) <= 0 &&
    range.compareBoundaryPoints(START_TO_END, target) >= 0
  );
}

/// Whether `range` overlaps the contents of `node`.
export function rangeTouches(range: Range, node: Node): boolean {
  const nodeRange = documentOf(node).createRange();
  nodeRange.selectNodeContents(node);
  return rangeOverlaps(range, nodeRange);
}

/// An empty editable line. `contenteditable` needs the `<br>` to give the element height and to
/// let the caret land in it; without one an "empty" cell or paragraph cannot be clicked into.
export function emptyLine(doc: Document, tag = "p"): HTMLElement {
  const node = doc.createElement(tag);
  node.appendChild(doc.createElement("br"));
  return node;
}

/// Whether the element holds nothing but the placeholder `<br>` (or no children at all).
export function isBlank(element: Element): boolean {
  if (element.childNodes.length === 0) return true;
  const only = element.childNodes.length === 1 ? element.firstChild : null;
  return !!only && only.nodeType === 1 && (only as Element).tagName === "BR";
}
