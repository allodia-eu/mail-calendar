// The quoted original a reply or forward is seeded with, and what survives the round trip back out
// of the editor as a `Quote` block.

import { describe, expect, test } from "bun:test";

import { documentBlocks } from "../src/document";
import { setComposerQuote } from "../src/quote";
import type { Block, QuoteValue } from "../src/types";
import { harness } from "./support";

/// The original in the bug this file exists for: a sender's client wrote the thread in plain text,
/// so every line break, blank line and `>` level is carried by the text itself.
const PLAIN_ORIGINAL = ["Hi Alice,", "", "Thanks for getting back to me.", "", "> Earlier line"].join(
  "\n",
);

function quoteOf(editor: HTMLElement): QuoteValue {
  const blocks: Block[] = documentBlocks(editor);
  const quote = blocks.find((block): block is { Quote: QuoteValue } => "Quote" in block);
  if (!quote) throw new Error("no Quote block");
  return quote.Quote;
}

describe("a plain-text original", () => {
  test("rides out with its line breaks in the markup, not in the editor's stylesheet", () => {
    // The regression: the body used to be one `textContent` assignment whose line breaks were laid
    // out by `.aq-plain { white-space: pre-wrap }`. That rule lives in the editor's own stylesheet
    // and goes nowhere near the recipient, so the message the core rendered collapsed the whole
    // quoted thread onto one line. Whatever holds the breaks has to be in the HTML itself.
    const { editor } = harness("");
    setComposerQuote(editor, { style: "Indented", body_html: "", body_plain: PLAIN_ORIGINAL });

    const { body_html } = quoteOf(editor);
    expect(body_html).toContain("white-space: pre-wrap");
    expect(body_html).toContain("Hi Alice,\n\nThanks for getting back to me.");
  });

  test("is inserted as text, so a `<` in the original cannot become an element", () => {
    const { editor } = harness("");
    setComposerQuote(editor, {
      style: "Indented",
      body_html: "",
      body_plain: "On Sat <sender@remote.test> wrote:\n> <img src=x onerror=alert(1)>",
    });

    expect(editor.querySelector("img")).toBeNull();
    expect(quoteOf(editor).body_html).toContain("&lt;img src=x onerror=alert(1)&gt;");
  });

  test("keeps the original in `body_plain` for the outgoing text part", () => {
    const { editor } = harness("");
    setComposerQuote(editor, { style: "Indented", body_html: "", body_plain: PLAIN_ORIGINAL });

    expect(quoteOf(editor).body_plain).toBe(PLAIN_ORIGINAL);
  });
});

describe("an HTML original", () => {
  test("is emitted verbatim, with no plain-text wrapper around it", () => {
    const { editor } = harness("");
    setComposerQuote(editor, {
      style: "Indented",
      body_html: "<p>Already sanitised</p>",
      body_plain: "Already sanitised",
    });

    expect(quoteOf(editor).body_html).toBe("<p>Already sanitised</p>");
  });
});
