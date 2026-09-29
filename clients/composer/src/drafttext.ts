// A drafted reply's formatting (docs/ai.md, "Drafting a reply"): a small Markdown subset, built as
// elements through the DOM API. The draft is untrusted model output, so nothing in it is ever
// parsed as markup; `**<img>**` becomes a bold run whose text is `<img>`.
//
// The subset: `**bold**` and `*italic*`; a line starting with `-`, `*` or `•` and a space is a
// bulleted item, one starting with `1.` or `1)` and a space a numbered one, and consecutive items
// of one kind, blank lines between them included, are one list; a line starting with one to three
// `#` and a space is drawn as a bold line. Anything else, an unmatched marker included, is text.

const BULLET = /^\s*[-*•]\s+(.*)$/;
const NUMBERED = /^\s*\d{1,3}[.)]\s+(.*)$/;
const HEADING = /^\s*#{1,3}\s+(.*)$/;
const INLINE = /\*\*(\S(?:.*?\S)?)\*\*|\*(\S(?:.*?\S)?)\*/g;

type ListTag = "UL" | "OL";

/// The draft's lines as editor blocks: one `<div>` per line, as `setPlainText` fills a body, and a
/// `<ul>` or `<ol>` for each run of list items.
export function draftBlocks(doc: Document, text: string): HTMLElement[] {
  const lines = text.split("\n");
  const blocks: HTMLElement[] = [];
  let list: HTMLElement | null = null;
  lines.forEach((line, index) => {
    const item = listItem(line);
    if (item) {
      if (!list || list.tagName !== item.tag) {
        list = doc.createElement(item.tag);
        blocks.push(list);
      }
      const li = doc.createElement("li");
      appendInline(doc, li, item.rest);
      list.appendChild(li);
      return;
    }
    if (line.trim() === "" && list && continuesList(lines, index, list.tagName as ListTag)) return;
    list = null;
    const div = doc.createElement("div");
    const heading = HEADING.exec(line);
    if (heading) {
      const bold = doc.createElement("b");
      bold.textContent = heading[1] ?? "";
      div.appendChild(bold);
    } else if (line.length > 0) {
      appendInline(doc, div, line);
    } else {
      div.appendChild(doc.createElement("br"));
    }
    blocks.push(div);
  });
  return blocks;
}

function listItem(line: string): { tag: ListTag; rest: string } | null {
  const bullet = BULLET.exec(line);
  if (bullet) return { tag: "UL", rest: bullet[1] ?? "" };
  const numbered = NUMBERED.exec(line);
  if (numbered) return { tag: "OL", rest: numbered[1] ?? "" };
  return null;
}

/// Whether the next line with anything on it is an item of the same list.
function continuesList(lines: string[], blank: number, tag: ListTag): boolean {
  const next = lines.slice(blank + 1).find((line) => line.trim() !== "");
  return next !== undefined && listItem(next)?.tag === tag;
}

function appendInline(doc: Document, parent: HTMLElement, text: string): void {
  let from = 0;
  for (const match of text.matchAll(INLINE)) {
    const at = match.index ?? 0;
    if (at > from) parent.appendChild(doc.createTextNode(text.slice(from, at)));
    const mark = doc.createElement(match[1] !== undefined ? "b" : "i");
    mark.textContent = match[1] ?? match[2] ?? "";
    parent.appendChild(mark);
    from = at + match[0].length;
  }
  if (from < text.length) parent.appendChild(doc.createTextNode(text.slice(from)));
}
