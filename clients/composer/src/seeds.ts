// Seeding the body from the host, and putting the caret in it.

import { caretInto, documentOf, focusEditor, rangeWithin } from "./dom";

/// Focuses the message area so the composer opens ready to type; a host calls this when it opens a
/// reply or forward, where the addresses and subject are already filled in and writing is the only
/// thing left to do. On a touch client this is also what raises the keyboard (once the host gives
/// its web view native focus; DOM focus alone will not).
///
/// The caret is only *placed* when there is not already one in the editor: `setComposerQuote` has
/// its own opinion about where it goes (the end of a pre-filled lead paragraph, above the quote) and
/// this must not overrule it. Otherwise it lands at the top of the lead paragraph; above the quoted
/// original, never inside it.
export function focusComposerBody(editor: HTMLElement): void {
  const lead = editor.firstElementChild;
  if (!rangeWithin(editor) && lead) caretInto(lead, true);
  focusEditor(editor);
}

/// Seeds the body from plain text: an assistant's draft (`docs/mcp.md`).
///
/// One `<div>` per line, NOT `editor.textContent = text`. The editor body has no `white-space:
/// pre-wrap` (only a quoted plain-text original does, for exactly this reason), so a raw text node
/// renders every newline as a space: a drafted message with paragraphs arrives as one run-on line,
/// and `documentBlocks` then serializes it as a single paragraph, so the collapse survives into what
/// is actually sent. A line per `<div>` is the shape `collectBlocks` already reads back as one
/// paragraph each, and an empty div carries a blank line through (`<br>` so the browser gives it
/// height).
export function setPlainText(editor: HTMLElement, text: unknown): void {
  const doc = documentOf(editor);
  editor.textContent = "";
  for (const line of String(text ?? "").split("\n")) {
    const div = doc.createElement("div");
    if (line.length > 0) div.textContent = line;
    else div.appendChild(doc.createElement("br"));
    editor.appendChild(div);
  }
}
