// Word's lists, turned back into lists before the paste walk reads the markup.
//
// Word, and Outlook, which edits mail with Word, write a list to the clipboard as paragraphs: each
// item names its list and level in its own style (`mso-list:l0 level2 lfo1`) and draws its bullet or
// number as text, inside a span marked `mso-list:Ignore` that sits behind a conditional comment
// for Word's own benefit. Read as written, each item pastes as a line starting with "·" and a run
// of spaces. Rewriting those paragraphs as `<ul>`/`<ol>` in the parsed copy lets the walk in
// `paste.ts` treat them as the lists they are.
//
// The marker is read and dropped, never kept: a numbered item says "1." or "a)" or "iv.", which is
// what decides that its list is ordered; the number itself is the reader's list to count.

/// The level of a Word list paragraph, or `null` for any other element.
///
/// Read from the `style` attribute as written: `mso-list` is no CSS property, so a parsed style
/// declaration drops it. An `<li>` is never one: Word for Mac writes a list that came to it as HTML
/// as `<ul>`/`<ol>` again, with `mso-list` on each item, and that is a list already.
function levelOf(element: Element): number | null {
  if (element.localName === "li") return null;
  const match = /mso-list\s*:\s*l\d+\s+level(\d+)/i.exec(element.getAttribute("style") ?? "");
  return match ? Number(match[1]) : null;
}

/// Whether `element` is the marker Word draws an item's bullet or number in.
function isMarker(element: Element): boolean {
  return /mso-list\s*:\s*ignore/i.test(element.getAttribute("style") ?? "");
}

/// Whether a marker's text numbers its item: "1.", "a)", "(iv)", rather than a bullet glyph.
function numbers(marker: string): boolean {
  return /^\(?([0-9]+|[a-z]{1,3})[.)]$/i.test(marker.replace(/[\s ]+/g, ""));
}

/// Rewrites every run of consecutive Word list paragraphs under `root` as nested lists.
export function wordLists(root: Element): void {
  const doc = root.ownerDocument;
  const parents = new Set<Element>();
  for (const element of Array.from(root.querySelectorAll("[style]"))) {
    if (levelOf(element) !== null && element.parentElement) parents.add(element.parentElement);
  }
  for (const parent of parents) {
    let run: Element[] = [];
    for (const node of Array.from(parent.childNodes)) {
      const between = node.nodeType === 8 || (node.nodeType === 3 && !node.nodeValue?.trim());
      if (node.nodeType === 1 && levelOf(node as Element) !== null) {
        run.push(node as Element);
      } else if (!between) {
        rewrite(doc, run);
        run = [];
      }
    }
    rewrite(doc, run);
  }
}

/// Replaces one run of list paragraphs with the lists they describe, in place of the first.
function rewrite(doc: Document, run: Element[]): void {
  const first = run[0];
  if (!first) return;
  const lists = doc.createDocumentFragment();
  const open: { list: Element; level: number; ordered: boolean }[] = [];
  for (const paragraph of run) {
    const level = levelOf(paragraph) ?? 1;
    const markers = Array.from(paragraph.querySelectorAll("[style]")).filter(isMarker);
    const ordered = numbers(markers[0]?.textContent ?? "");
    for (const marker of markers) marker.remove();

    while (open.length > 0 && open[open.length - 1]!.level > level) open.pop();
    const top = open[open.length - 1];
    if (top && top.level === level && top.ordered !== ordered) open.pop();
    let current = open[open.length - 1];
    if (!current || current.level < level) {
      const list = doc.createElement(ordered ? "ol" : "ul");
      const host = current?.list.lastElementChild ?? current?.list;
      (host ?? lists).appendChild(list);
      current = { list, level, ordered };
      open.push(current);
    }
    const item = doc.createElement("li");
    while (paragraph.firstChild) item.appendChild(paragraph.firstChild);
    current.list.appendChild(item);
  }
  first.parentNode?.insertBefore(lists, first);
  for (const paragraph of run) paragraph.remove();
}
