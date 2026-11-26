// Enter at the start of the signature, and Enter in a line the signature or a quoted body holds
// directly: the region is never split, so what was typed is never sent as a second region.

import { describe, expect, test } from "bun:test";

import { documentBlocks } from "../src/document";
import { setComposerQuote } from "../src/quote";
import { splitRegionLine } from "../src/region_lines";
import { openLineAboveSignature, setComposerSignature } from "../src/signature";
import type { QuoteValue } from "../src/types";
import { harness } from "./support";

/// A signature as the Settings editor stores one: its first line is text held by the region itself.
const SIGNATURE = {
  body_html: "Met vriendelijke groet,<div><br></div><div>Ada</div>",
  body_plain: "Met vriendelijke groet,\n\nAda",
};

function caretAt(editor: HTMLElement, node: Node, offset: number): void {
  const selection = editor.ownerDocument.defaultView!.getSelection()!;
  const range = editor.ownerDocument.createRange();
  range.setStart(node as never, offset);
  range.collapse(true);
  selection.removeAllRanges();
  selection.addRange(range as never);
}

function signed() {
  const h = harness("<div>Hello</div>");
  setComposerSignature(h.editor, SIGNATURE);
  const region = h.editor.querySelector(".allodia-signature")!;
  return { ...h, region, first: region.firstChild! };
}

describe("Enter at the start of the signature", () => {
  test("opens a line of the message above it", () => {
    const h = signed();
    caretAt(h.editor, h.first, 0);
    expect(openLineAboveSignature(h.editor)).toBe(true);
    expect(h.html()).toBe(
      `<div>Hello</div><div><br></div><div><br></div><div class="allodia-signature">${SIGNATURE.body_html}</div>`,
    );
    // The caret stays with the signature's first word, as Enter leaves it anywhere else.
    expect(h.editor.ownerDocument.defaultView!.getSelection()!.anchorNode).toBe(h.first as never);
  });

  test("looks past the white space between the signature's blocks", () => {
    const h = harness("<div>Hello</div>");
    setComposerSignature(h.editor, { body_html: "\n<div>Regards</div>", body_plain: "Regards" });
    h.caret(".allodia-signature > div");
    expect(openLineAboveSignature(h.editor)).toBe(true);
  });

  test("is not taken anywhere later in the signature", () => {
    const h = signed();
    caretAt(h.editor, h.first, 3);
    const before = h.html();
    expect(openLineAboveSignature(h.editor)).toBe(false);
    expect(h.html()).toBe(before);
  });

  test("is not taken in a quoted original's signature", () => {
    const h = harness(
      '<div id=reply>Hi</div><div class="allodia-quote"><div class="aq-body">' +
        '<div class="allodia-signature"><div id=quoted>Ada</div></div></div></div>',
    );
    h.caret("#quoted");
    expect(openLineAboveSignature(h.editor)).toBe(false);
  });
});

describe("Enter in a line the signature holds directly", () => {
  test("splits the line, not the signature", () => {
    // The engine splits the region instead and copies its class onto the new line, and every line
    // typed after it goes out as one more signature under its own `-- `.
    const h = signed();
    caretAt(h.editor, h.first, "Met ".length);
    expect(splitRegionLine(h.editor)).toBe(true);
    expect(h.editor.querySelectorAll(".allodia-signature")).toHaveLength(1);
    expect(h.region.innerHTML).toBe(
      "<div>Met </div><div>vriendelijke groet,</div><div><br></div><div>Ada</div>",
    );
    expect(h.caretHost()?.textContent).toBe("vriendelijke groet,");
  });

  test("at its end opens an empty line inside it, with the caret there", () => {
    const h = signed();
    caretAt(h.editor, h.first, "Met vriendelijke groet,".length);
    expect(splitRegionLine(h.editor)).toBe(true);
    expect(h.region.innerHTML).toBe(
      "<div>Met vriendelijke groet,</div><div><br></div><div><br></div><div>Ada</div>",
    );
    expect(h.caretHost()).toBe(h.region.children[1] as never);
  });

  test("keeps a mark that runs across the caret on both halves", () => {
    const h = harness('<div>Hi</div><div class="allodia-signature"><b>Ada Lovelace</b><br>Analytical Engines</div>');
    const bold = h.editor.querySelector("b")!.firstChild!;
    caretAt(h.editor, bold, "Ada".length);
    expect(splitRegionLine(h.editor)).toBe(true);
    expect(h.editor.querySelector(".allodia-signature")!.innerHTML).toBe(
      "<div><b>Ada</b></div><div><b> Lovelace</b><br>Analytical Engines</div>",
    );
  });

  test("over a selection, deletes it and splits the line, not the signature", () => {
    const h = signed();
    const selection = h.editor.ownerDocument.defaultView!.getSelection()!;
    const range = h.editor.ownerDocument.createRange();
    range.setStart(h.first as never, "Met ".length);
    range.setEnd(h.first as never, "Met vriendelijke".length);
    selection.removeAllRanges();
    selection.addRange(range as never);
    expect(splitRegionLine(h.editor)).toBe(true);
    expect(h.editor.querySelectorAll(".allodia-signature")).toHaveLength(1);
    expect(h.region.innerHTML).toBe("<div>Met </div><div> groet,</div><div><br></div><div>Ada</div>");
  });

  test("leaves a line of its own to the engine", () => {
    const h = signed();
    caretAt(h.editor, h.region.lastElementChild!.firstChild!, 2);
    expect(splitRegionLine(h.editor)).toBe(false);
  });

  test("leaves the message's own lines to the engine", () => {
    const h = signed();
    caretAt(h.editor, h.editor.firstElementChild!.firstChild!, 2);
    expect(splitRegionLine(h.editor)).toBe(false);
  });

  test("sends what was typed as one signature", () => {
    const h = signed();
    caretAt(h.editor, h.first, "Met vriendelijke groet,".length);
    splitRegionLine(h.editor);
    const signatures = documentBlocks(h.editor).filter((block) => "Signature" in block);
    expect(signatures).toHaveLength(1);
  });

  test("leaves the region's markup as it was until Enter is pressed", () => {
    // A reopened draft has to be the document it was saved from.
    const h = signed();
    expect(h.region.innerHTML).toBe(SIGNATURE.body_html);
  });
});

describe("Enter in a line a quoted body holds directly", () => {
  test("keeps both halves in the quote that is sent", () => {
    // `quoteBlock` reads the first `.aq-body`: of a split body, only the half above the caret is sent.
    const { editor } = harness("");
    setComposerQuote(editor, { style: "Indented", body_html: "Lunch?<br>Bob", body_plain: "Lunch?\nBob" });
    const body = editor.querySelector(".aq-body")!;
    caretAt(editor, body.firstChild!, "Lunch?".length);
    expect(splitRegionLine(editor)).toBe(true);
    expect(editor.querySelectorAll(".aq-body")).toHaveLength(1);
    const quote = documentBlocks(editor).find((block): block is { Quote: QuoteValue } => "Quote" in block);
    expect(quote?.Quote.body_html).toBe("<div>Lunch?</div><div><br>Bob</div>");
  });

  test("gives a half holding only white space a line of its own", () => {
    // Markup's white space draws nothing, so a line holding only that would have no height.
    const { editor } = harness("");
    setComposerQuote(editor, { style: "Indented", body_html: "Hi Bob,\n<p>Later</p>", body_plain: "" });
    const body = editor.querySelector(".aq-body")!;
    caretAt(editor, body.firstChild!, "Hi Bob,".length);
    expect(splitRegionLine(editor)).toBe(true);
    expect(body.innerHTML).toBe("<div>Hi Bob,</div><div><br></div><p>Later</p>");
  });

  test("leaves a plain-text original's wrapper to the engine", () => {
    const { editor } = harness("");
    setComposerQuote(editor, { style: "Indented", body_html: "", body_plain: "Hi\nAlice" });
    caretAt(editor, editor.querySelector(".aq-plain")!.firstChild!, 2);
    expect(splitRegionLine(editor)).toBe(false);
  });
});

/// Seeds a reply as a host does: the quote first, then the signature.
function reply() {
  const h = harness("");
  setComposerQuote(h.editor, { style: "Indented", body_html: "<p>Lunch?</p>", body_plain: "Lunch?" });
  return h;
}

const BLANK = "<div><br></div>";
const REGION = `<div class="allodia-signature">${SIGNATURE.body_html}</div>`;

describe("the signature is set apart by a real blank line", () => {
  // A real line rather than a margin of the editor's own: the message is spaced as the editor
  // shows it, and the person can delete it.
  test("in a new message, under the line the caret is in", () => {
    const h = harness("");
    setComposerSignature(h.editor, SIGNATURE);
    expect(h.html()).toBe(`${BLANK}${BLANK}${REGION}`);
  });

  test("under what is already written", () => {
    const h = harness("<div>Hello</div>");
    setComposerSignature(h.editor, SIGNATURE);
    expect(h.html()).toBe(`<div>Hello</div>${BLANK}${REGION}`);
  });

  test("not twice when a blank line is already there", () => {
    const h = harness(`<div>Hello</div>${BLANK}`);
    setComposerSignature(h.editor, SIGNATURE);
    expect(h.html()).toBe(`<div>Hello</div>${BLANK}${REGION}`);
  });

  test("in a reply, above the signature and between it and the quote", () => {
    const h = reply();
    setComposerSignature(h.editor, SIGNATURE);
    const children = Array.from(h.editor.children).map((child) => child.className || child.outerHTML);
    expect(children).toEqual([BLANK, BLANK, "allodia-signature", BLANK, "allodia-quote"]);
  });

  test("leaves a reply as it was when switched off and on again", () => {
    const h = reply();
    setComposerSignature(h.editor, SIGNATURE);
    const signed = h.html();
    setComposerSignature(h.editor, null);
    expect(Array.from(h.editor.children).map((child) => child.className || child.outerHTML)).toEqual([
      BLANK,
      BLANK,
      "allodia-quote",
    ]);
    setComposerSignature(h.editor, SIGNATURE);
    expect(h.html()).toBe(signed);
  });

  test("leaves the blank lines alone when the signature is swapped", () => {
    const h = reply();
    setComposerSignature(h.editor, SIGNATURE);
    setComposerSignature(h.editor, { body_html: "<div>Ada</div>", body_plain: "Ada" });
    expect(Array.from(h.editor.children).map((child) => child.className || child.outerHTML)).toEqual([
      BLANK,
      BLANK,
      "allodia-signature",
      BLANK,
      "allodia-quote",
    ]);
  });
});

describe("a reply without a signature", () => {
  test("opens with a blank line between the caret's line and the quote", () => {
    const h = reply();
    expect(Array.from(h.editor.children).map((child) => child.className || child.outerHTML)).toEqual([
      BLANK,
      BLANK,
      "allodia-quote",
    ]);
    expect(h.caretHost()).toBe(h.editor.firstElementChild as never);
  });
});

describe("the editor's stylesheet", () => {
  test("gives the signature and the quote no spacing the message does not carry", async () => {
    // The renderer writes these regions with no margin, so a margin here was a gap the person saw
    // and the recipient did not. The space around them is a real blank line instead.
    const stylesheet = await Bun.file(`${import.meta.dir}/../src/editor.css`).text();
    for (const selector of [".allodia-signature", ".allodia-quote", ".aq-attr"]) {
      const rule = new RegExp(`(^|\\n)${selector.replace(".", "\\.")}\\s*\\{([^}]*)\\}`).exec(stylesheet);
      expect(rule?.[2] ?? "").not.toMatch(/margin|padding/);
    }
  });
});
