// The line commands: heading level, alignment, indent and the rule, each writing the one shape the
// reader reads, so a line formatted here saves and reopens as it was.

import { describe, expect, test } from "bun:test";

import { documentBlocks } from "../src/document";
import { indentLines, insertRule, setAlign, setHeading } from "../src/paragraphs";
import { canonicalFamily, familyCss, sizeOfCss, sizeOfLegacy } from "../src/typography";
import { harness } from "./support";

describe("headings", () => {
  test("make the caret's line a heading, and back into body text", () => {
    const h = harness("<div id=a>Agenda</div><p id=b>Items</p>");
    h.caret("#a");
    setHeading(h.editor, "H2");
    expect(h.html()).toBe('<h2 id="a">Agenda</h2><p id="b">Items</p>');
    expect(documentBlocks(h.editor)[0]).toEqual({
      Paragraph: { content: [{ Text: { text: "Agenda", bold: false, italic: false, underline: false } }], heading: "H2" },
    });
    h.caret("#a");
    setHeading(h.editor, null);
    expect(h.html()).toBe('<div id="a">Agenda</div><p id="b">Items</p>');
  });

  test("leave a list item, a table cell and the quoted original alone", () => {
    const h = harness(
      '<ul><li id=li>Item</li></ul><table><tbody><tr><td id=td>Cell</td></tr></tbody></table>' +
        '<div class="allodia-quote"><div class="aq-body"><p id=q>Quoted</p></div></div>',
    );
    for (const selector of ["#li", "#td", "#q"]) {
      h.caret(selector);
      expect(setHeading(h.editor, "H1")).toBe(false);
    }
    expect(h.editor.querySelector("h1")).toBeNull();
  });
});

describe("alignment", () => {
  test("aligns every selected line, and sets it back at the start", () => {
    const h = harness("<p id=a>One</p><p id=b>Two</p><p id=c>Three</p>");
    h.select("#a", "#b");
    setAlign(h.editor, "Center");
    const aligns = documentBlocks(h.editor).map((block) => ("Paragraph" in block ? block.Paragraph.align : null));
    expect(aligns).toEqual(["Center", "Center", undefined]);
    h.select("#a", "#a");
    setAlign(h.editor, null);
    expect(h.editor.querySelector("#a")!.hasAttribute("style")).toBe(false);
  });
});

describe("indent", () => {
  test("steps a line in by 40px and out again, never below the margin", () => {
    const h = harness("<div id=a>One</div>");
    h.caret("#a");
    indentLines(h.editor, false);
    indentLines(h.editor, false);
    expect(documentBlocks(h.editor)[0]).toMatchObject({ Paragraph: { indent_px: 80 } });
    indentLines(h.editor, true);
    indentLines(h.editor, true);
    indentLines(h.editor, true);
    expect(documentBlocks(h.editor)[0]).toEqual({
      Paragraph: { content: [{ Text: { text: "One", bold: false, italic: false, underline: false } }] },
    });
  });

  test("keeps another client's step: a quarter inch indents to 64px", () => {
    const h = harness('<p id=a style="margin-left: 24px">One</p>');
    h.caret("#a");
    indentLines(h.editor, false);
    expect(documentBlocks(h.editor)[0]).toMatchObject({ Paragraph: { indent_px: 64 } });
  });
});

describe("rule", () => {
  test("goes below the caret's line, with a line after it for the caret", () => {
    const h = harness("<div id=a>Above</div>");
    h.caret("#a");
    insertRule(h.editor);
    expect(h.html()).toBe('<div id="a">Above</div><hr><div><br></div>');
    expect(h.caretHost()?.tagName).toBe("DIV");
    expect(documentBlocks(h.editor)).toEqual([
      { Paragraph: { content: [{ Text: { text: "Above", bold: false, italic: false, underline: false } }] } },
      { Rule: {} },
      { Paragraph: { content: [] } },
    ]);
  });
});

describe("typography", () => {
  test("reads a size in any unit mail uses", () => {
    expect(sizeOfCss("16.0pt")).toBe(21);
    expect(sizeOfCss("13px")).toBe(13);
    expect(sizeOfCss("x-large")).toBe(24);
    expect(sizeOfCss("1.2em")).toBeNull();
    expect(sizeOfLegacy("6")).toBe(32);
    expect(sizeOfLegacy("+1")).toBe(18);
    expect(sizeOfLegacy("huge")).toBeNull();
  });

  test("holds a family in one spelling, and refuses one that is not a font name", () => {
    expect(canonicalFamily('"Comic Sans MS"')).toBe("Comic Sans MS");
    expect(canonicalFamily(" 'Segoe UI Emoji' ,sans-serif")).toBe("Segoe UI Emoji, sans-serif");
    expect(familyCss("Segoe UI Emoji, sans-serif")).toBe("'Segoe UI Emoji', sans-serif");
    expect(canonicalFamily("Arial;color:red")).toBeNull();
    expect(canonicalFamily("")).toBeNull();
  });
});
