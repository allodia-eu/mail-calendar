import { describe, expect, test } from "bun:test";

import { autoformatBulletList } from "../src/autoformat";
import { documentBlocks } from "../src/document";
import { harness } from "./support";

describe("typing a marker starts a list", () => {
  test("a lone hyphen becomes a bullet, and the marker disappears", () => {
    const h = harness("<p id=p>-</p>");
    h.caretEnd("#p");
    expect(autoformatBulletList(h.editor)).toBe(true);
    expect(h.html()).toBe("<ul><li><br></li></ul>");
    expect(h.caretHost()?.closest("li")).not.toBeNull();
  });

  test("whatever followed the caret on that line becomes the item's text", () => {
    const h = harness("<p id=p>-already typed</p>");
    // The caret sits between the marker and the rest, which is where it is when the space is typed.
    const paragraph = h.caret("#p");
    const range = paragraph.ownerDocument.createRange();
    range.setStart(paragraph.firstChild as never, 1);
    range.collapse(true);
    const selection = paragraph.ownerDocument.defaultView!.getSelection()!;
    selection.removeAllRanges();
    selection.addRange(range as never);
    expect(autoformatBulletList(h.editor)).toBe(true);
    expect(h.html()).toBe("<ul><li>already typed</li></ul>");
  });

  test("the very first line of an empty composer, before anything wraps it", () => {
    // `contenteditable` only starts wrapping lines at the first Enter, so the most common case,
    // opening a composer and typing "- " straight away: has no block wrapper at all.
    const h = harness("-");
    const text = h.editor.firstChild!;
    const range = h.editor.ownerDocument.createRange();
    range.setStart(text as never, 1);
    range.collapse(true);
    const selection = h.editor.ownerDocument.defaultView!.getSelection()!;
    selection.removeAllRanges();
    selection.addRange(range as never);
    expect(autoformatBulletList(h.editor)).toBe(true);
    expect(h.html()).toBe("<ul><li><br></li></ul>");
  });

  test("the result is a real List block, not a paragraph that looks like one", () => {
    const h = harness("<p id=p>-</p>");
    h.caretEnd("#p");
    autoformatBulletList(h.editor);
    expect(documentBlocks(h.editor)).toEqual([
      { List: { kind: "Bullet", items: [{ content: [], child: null }] } },
    ]);
  });
});

describe("it stays out of the way", () => {
  test("a hyphen with words in front of it is just a hyphen", () => {
    const h = harness("<p id=p>well - actually</p>");
    h.caretEnd("#p");
    expect(autoformatBulletList(h.editor)).toBe(false);
    expect(h.html()).toBe('<p id="p">well - actually</p>');
  });

  test("a line that is not exactly the marker is left alone", () => {
    for (const body of ["<p id=p>--</p>", "<p id=p>-x</p>", "<p id=p> -</p>", "<p id=p></p>"]) {
      const h = harness(body);
      h.caretEnd("#p");
      expect(autoformatBulletList(h.editor), body).toBe(false);
    }
  });

  test("inside a list item the marker is text the user meant to type", () => {
    const h = harness("<ul><li id=a>-</li></ul>");
    h.caretEnd("#a");
    expect(autoformatBulletList(h.editor)).toBe(false);
  });

  test("inside a table cell it does not fire: the schema has nowhere to put a list", () => {
    // `TableCell` holds inline content only, so a <ul> built in a cell would be flattened into
    // loose text on send. Not firing leaves a literal "- ", which is honest.
    const h = harness("<table><tbody><tr><td id=c>-</td></tr></tbody></table>");
    h.caretEnd("#c");
    expect(autoformatBulletList(h.editor)).toBe(false);
  });

  test("an unwrapped line beside a signature does not sweep the signature into the list", () => {
    // With no block wrapper the editor's children are the line; unless something else is there.
    // A seeded signature is a block sibling, and eating it would destroy content the user never
    // touched (the failure `setComposerSignature` is scoped so carefully to avoid).
    const h = harness('-<div class="allodia-signature">Alice</div>');
    const text = h.editor.firstChild!;
    const range = h.editor.ownerDocument.createRange();
    range.setStart(text as never, 1);
    range.collapse(true);
    const selection = h.editor.ownerDocument.defaultView!.getSelection()!;
    selection.removeAllRanges();
    selection.addRange(range as never);
    expect(autoformatBulletList(h.editor)).toBe(false);
    expect(h.html()).toContain("allodia-signature");
  });

  test("it never replaces the quoted original's body wrapper", () => {
    // A plain-text original is text held directly by `.aq-body`, so the nearest block ancestor of a
    // caret at its start IS that container. Replacing it with a <ul> dropped the class `quoteBlock`
    // reads the body from, and the whole quoted message left the outgoing HTML.
    const h = harness(
      '<p>reply</p><div class="allodia-quote" data-quote-plain="orig">' +
        '<div class="aq-attr" contenteditable="false">On x wrote:</div>' +
        '<div class="aq-body aq-plain">-original</div></div>',
    );
    const body = h.editor.querySelector(".aq-body")!;
    const range = h.editor.ownerDocument.createRange();
    range.setStart(body.firstChild as never, 1);
    range.collapse(true);
    const selection = h.editor.ownerDocument.defaultView!.getSelection()!;
    selection.removeAllRanges();
    selection.addRange(range as never);
    expect(autoformatBulletList(h.editor)).toBe(false);
    const quote = documentBlocks(h.editor).find((block) => "Quote" in block) as {
      Quote: { body_html: string };
    };
    expect(quote.Quote.body_html).toBe("-original");
  });

  test("it does nothing without a caret in the editor", () => {
    const h = harness("<p id=p>-</p>");
    expect(autoformatBulletList(h.editor)).toBe(false);
  });
});
