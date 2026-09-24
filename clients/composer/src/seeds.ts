// Seeding the body from the host, and putting the caret in it.

import { caretInto, documentOf, focusEditor, rangeWithin, windowOf } from "./dom";
import { draftBlocks } from "./drafttext";

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
  editor.textContent = "";
  for (const div of lineDivs(editor, text)) editor.appendChild(div);
}

/// Puts a drafted reply into the open composer (`docs/ai.md`): replaces what the person has typed
/// above the signature and the quoted original, and leaves both of those exactly where they are.
///
/// The lead region is every node before the first direct child that is the signature or the quote.
/// Direct children only, for the reason `composerSignature` gives: a reply to our own mail carries
/// a second `.allodia-signature` inside the quote. The draft goes in through `draftBlocks`, one
/// `<div>` per line with its bold, italic and lists built as elements, and the caret ends up after
/// its last character, so the person picks up where the draft stops.
///
/// `draftId` is the id the core issued with the draft. The editor keeps it and `composerDocument`
/// hands it back as `ai_draft`, which is how a reply sent from a draft is kept out of every later
/// learning run; a later draft replaces it.
export function setComposerDraftText(editor: HTMLElement, text: unknown, draftId?: unknown): void {
  if (typeof draftId === "string" && draftId.length > 0) editor.dataset.aiDraft = draftId;
  const boundary = leadBoundary(editor);
  while (editor.firstChild && editor.firstChild !== boundary) {
    editor.removeChild(editor.firstChild);
  }
  const blocks = draftBlocks(documentOf(editor), String(text ?? ""));
  for (const block of blocks) editor.insertBefore(block, boundary);
  const last = blocks[blocks.length - 1];
  if (last) caretInto(last.lastElementChild?.tagName === "LI" ? last.lastElementChild : last, false);
}

/// Whether the person has put anything above the signature and the quoted original: text, or a
/// picture. A host asks before a draft replaces it.
export function composerLeadHasText(editor: HTMLElement): boolean {
  const boundary = leadBoundary(editor);
  for (let node = editor.firstChild; node && node !== boundary; node = node.nextSibling) {
    if ((node.textContent ?? "").trim().length > 0) return true;
    if (node instanceof windowOf(editor).Element) {
      if (node.nodeName === "IMG" || node.querySelector("img")) return true;
    }
  }
  return false;
}

/// The first direct child that is the signature or the quote, which ends the lead region.
function leadBoundary(editor: HTMLElement): Element | null {
  return (
    Array.from(editor.children).find(
      (child) =>
        child.classList.contains("allodia-signature") || child.classList.contains("allodia-quote"),
    ) ?? null
  );
}

/// One `<div>` per line of `text`, an empty line carrying a `<br>` so the browser gives it height.
function lineDivs(editor: HTMLElement, text: unknown): HTMLElement[] {
  const doc = documentOf(editor);
  return String(text ?? "")
    .split("\n")
    .map((line) => {
      const div = doc.createElement("div");
      if (line.length > 0) div.textContent = line;
      else div.appendChild(doc.createElement("br"));
      return div;
    });
}
