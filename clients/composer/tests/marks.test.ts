import { describe, expect, test } from "bun:test";

import { documentBlocks } from "../src/document";
import { applyFontSize, clearColor } from "../src/format";
import { normalizeColor, normalizeFontElements } from "../src/marks";
import type { InlineContent, TextRun } from "../src/types";
import { harness } from "./support";

/// The text runs of the first paragraph.
function runs(editor: HTMLElement): TextRun[] {
  const block = documentBlocks(editor)[0] as { Paragraph: { content: InlineContent[] } };
  return block.Paragraph.content
    .filter((item): item is { Text: TextRun } => "Text" in item)
    .map((item) => item.Text);
}

describe("normalizeColor", () => {
  test("accepts every spelling an engine hands back and emits #rrggbb", () => {
    expect(normalizeColor("#FF0000")).toBe("#ff0000");
    expect(normalizeColor("#f00")).toBe("#ff0000");
    expect(normalizeColor("rgb(255, 0, 0)")).toBe("#ff0000");
    expect(normalizeColor("rgba(255, 0, 0, 0.5)")).toBe("#ff0000");
    expect(normalizeColor("rgb(100%, 0%, 0%)")).toBe("#ff0000");
  });

  test("a fully transparent colour is no colour, not black", () => {
    // This is how "no highlight" arrives. Treating it as #000000 would paint every un-highlighted
    // run with a black background.
    expect(normalizeColor("transparent")).toBeNull();
    expect(normalizeColor("rgba(0, 0, 0, 0)")).toBeNull();
  });

  test("a colour we cannot represent is dropped rather than passed to Rust", () => {
    // Rust rejects anything but #rrggbb and fails the whole document; the run simply renders in the
    // inherited colour instead.
    expect(normalizeColor("red")).toBeNull();
    expect(normalizeColor("color-mix(in srgb, red, blue)")).toBeNull();
    expect(normalizeColor("")).toBeNull();
    expect(normalizeColor(null)).toBeNull();
  });

  test("clamps out-of-range channels instead of emitting invalid hex", () => {
    expect(normalizeColor("rgb(300, -20, 0)")).toBe("#ff0000");
  });
});

describe("reading colour off the document", () => {
  test("an explicit marker becomes a run colour", () => {
    const h = harness('<p><span data-color="#ff0000">red</span> plain</p>');
    expect(runs(h.editor)).toEqual([
      { text: "red", bold: false, italic: false, underline: false, color: "#ff0000" },
      { text: " plain", bold: false, italic: false, underline: false },
    ]);
  });

  test("an inline style is read when there is no marker, in whatever spelling", () => {
    const h = harness('<p><span style="color: rgb(0, 128, 0)">green</span></p>');
    expect(runs(h.editor)[0]!.color).toBe("#008000");
  });

  test("a highlight is read from the background colour", () => {
    const h = harness('<p><span data-highlight="#ffff00">marked</span></p>');
    expect(runs(h.editor)[0]!.highlight).toBe("#ffff00");
  });

  test("inheriting text colour does not stamp one on every run", () => {
    // The whole reason marks are read from the element's own markers: computed colour is always
    // set, so reading it would put an explicit colour on every run in every message.
    const h = harness("<p>plain <b>bold</b></p>");
    for (const run of runs(h.editor)) {
      expect(run.color).toBeUndefined();
      expect(run.highlight).toBeUndefined();
    }
  });

  test("a nested colour overrides the one it inherits", () => {
    const h = harness(
      '<p><span data-color="#ff0000">red <span data-color="#0000ff">blue</span></span></p>',
    );
    expect(runs(h.editor).map((run) => run.color)).toEqual(["#ff0000", "#0000ff"]);
  });

  test("colour rides alongside the other marks rather than replacing them", () => {
    const h = harness('<p><b><span data-color="#ff0000">both</span></b></p>');
    expect(runs(h.editor)[0]).toEqual({
      text: "both",
      bold: true,
      italic: false,
      underline: false,
      color: "#ff0000",
    });
  });
});

describe("normalizeFontElements", () => {
  test("rewrites the <font> execCommand leaves behind into a marked span", () => {
    const h = harness('<p><font size="7">big</font></p>');
    normalizeFontElements(h.editor, "Large");
    expect(h.html()).toBe('<p><span data-size="Large" style="font-size: 18px;">big</span></p>');
    expect(runs(h.editor)[0]!.font_size).toBe("Large");
  });

  test("rewrites a legacy colour attribute", () => {
    const h = harness('<p><font color="#FF0000">red</font></p>');
    normalizeFontElements(h.editor, null);
    expect(runs(h.editor)[0]!.color).toBe("#ff0000");
  });

  test("keeps the element's own inline style", () => {
    // An engine that does not implement `hiliteColor` paints a highlight as
    // `<font style="background-color:…">`; dropping the attribute lost the mark just applied.
    const h = harness('<p><font style="background-color: #ffff00">marked</font></p>');
    normalizeFontElements(h.editor, null);
    expect(runs(h.editor)[0]!.highlight).toBe("#ffff00");
  });

  test("leaves the quote and the signature alone", () => {
    // Both round-trip to Rust as verbatim HTML, so rewriting inside them would edit content the
    // user never touched.
    const h = harness(
      '<div class="allodia-quote"><div class="aq-body"><font size="7">q</font></div></div>' +
        '<div class="allodia-signature"><font color="#ff0000">s</font></div>',
    );
    normalizeFontElements(h.editor, "Huge");
    expect(h.html()).toContain('<font size="7">q</font>');
    expect(h.html()).toContain('<font color="#ff0000">s</font>');
  });
});

describe("applyFontSize", () => {
  // `execCommand` does not exist under happy-dom, so what runs here is the half that matters: the
  // sweep that rewrites the `<span style="font-size:…">` an engine's CSS path emits.
  test("leaves sized runs outside the selection alone", () => {
    // A document can hold sized spans carrying no `data-size`: a stored signature reopened in the
    // Settings editor is exactly that, since the core's sanitiser keeps `style` and drops `data-*`.
    // An unscoped sweep resized every one of them because the user picked a size for one word.
    const h = harness(
      '<p id=first><span style="font-size: 24px">name</span></p>' +
        '<p id=second><span style="font-size: 13px">disclaimer</span></p>',
    );
    h.select("#first", "#first");
    applyFontSize(h.editor, "Large");
    expect(runs(h.editor)[0]!.font_size).toBe("Large");
    expect(h.html()).toContain("font-size: 13px");
  });

  test("with no caret in the editor it rewrites nothing", () => {
    const h = harness('<p><span style="font-size: 24px">name</span></p>');
    applyFontSize(h.editor, "Small");
    expect(h.html()).toContain("font-size: 24px");
  });
});

describe("clearColor", () => {
  test("removes the mark and the wrapper it was the only reason for", () => {
    const h = harness('<p id=p><span data-color="#ff0000" style="color: #ff0000">red</span></p>');
    h.caret("#p");
    h.select("#p", "#p");
    clearColor(h.editor, "color");
    expect(h.html()).toBe('<p id="p">red</p>');
    expect(runs(h.editor)[0]!.color).toBeUndefined();
  });

  test("keeps a wrapper that still carries other formatting", () => {
    const h = harness(
      '<p id=p><span data-color="#ff0000" data-size="Large" style="color: #ff0000; font-size: 18px">x</span></p>',
    );
    h.select("#p", "#p");
    clearColor(h.editor, "color");
    expect(runs(h.editor)[0]!.color).toBeUndefined();
    expect(runs(h.editor)[0]!.font_size).toBe("Large");
  });

  test("leaves colour outside the selection untouched", () => {
    const h = harness(
      '<p id=first><span data-color="#ff0000" style="color:#ff0000">a</span></p>' +
        '<p id=second><span data-color="#0000ff" style="color:#0000ff">b</span></p>',
    );
    h.select("#first", "#first");
    clearColor(h.editor, "color");
    expect(h.html()).toContain('<p id="first">a</p>');
    expect(h.html()).toContain('data-color="#0000ff"');
  });
});
