// The plain-text rendering of a quote's or a signature's HTML: what rides beside it into a
// message's `text/plain` half.
//
// One function for every direction, so a region's text is always its HTML's: the quote a reply
// opens with, the same quote after the user trimmed it, and the quote a reopened draft reads back
// all carry the text of the markup they hold, and none can drift from the others.

/// The words of `element` with its lines kept apart. Preformatted text, a `<pre>` or anything laid
/// out with `white-space: pre`, keeps its spaces and line breaks exactly; everything else is laid
/// out as a reader would, with each run of whitespace one space and no space at a line's ends.
export function plainText(element: Element): string {
  const lines: Line[] = [{ text: "", pre: false }];
  const current = () => lines[lines.length - 1]!;
  const breakLine = () => {
    if (current().text !== "") lines.push({ text: "", pre: false });
  };
  const walk = (node: Node, pre: boolean) => {
    if (node.nodeType === 3) {
      const value = node.nodeValue ?? "";
      const parts = pre ? value.split("\n") : [value];
      parts.forEach((part, index) => {
        if (index > 0) lines.push({ text: "", pre });
        current().text += part;
        if (pre) current().pre = true;
      });
      return;
    }
    if (node.nodeType !== 1) return;
    const element = node as Element;
    const tag = element.tagName;
    if (tag === "BR") {
      lines.push({ text: "", pre: false });
      return;
    }
    const block = /^(P|DIV|LI|TR|H[1-6]|BLOCKQUOTE|PRE|TABLE|UL|OL)$/.test(tag);
    if (block) breakLine();
    const inner = pre || isPreformatted(element);
    for (const child of Array.from(element.childNodes)) walk(child, inner);
    if (block) breakLine();
  };
  for (const child of Array.from(element.childNodes)) walk(child, isPreformatted(element));

  const out: string[] = [];
  let blanks = 0;
  for (const line of lines) {
    const text = line.pre ? line.text : line.text.replace(/[\t\r ]+/g, " ").trim();
    // Blank lines between blocks are layout, and two say as much as ten; a preformatted body's own
    // blank lines are the sender's, and stay.
    blanks = text === "" && !line.pre ? blanks + 1 : 0;
    if (blanks <= 1) out.push(text);
  }
  while (out.length > 0 && out[0] === "") out.shift();
  while (out.length > 0 && out[out.length - 1] === "") out.pop();
  return out.join("\n");
}

interface Line {
  text: string;
  pre: boolean;
}

/// Whether `element` lays its text out as written: a `<pre>`, or `white-space: pre`, `pre-wrap` or
/// `pre-line` stated inline, which is how every client's editor writes it and the only way markup
/// sanitised for mail can carry it.
export function isPreformatted(element: Element): boolean {
  if (element.tagName === "PRE") return true;
  return /white-space\s*:\s*pre/i.test(element.getAttribute("style") ?? "");
}
