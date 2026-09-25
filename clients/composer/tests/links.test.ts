import { describe, expect, test } from "bun:test";

import { documentBlocks } from "../src/document";
import { saveSelection } from "../src/dom";
import {
  applyLink,
  autolinkBeforeCaret,
  linkAtCaret,
  normalizeLinkAddress,
  removeLink,
  safeLinkHref,
  typedAddress,
} from "../src/links";
import type { InlineContent, TextRun } from "../src/types";
import { harness } from "./support";

/// How `innerHTML` writes the no-break space a space typed at the end of a line becomes.
const NBSP = "&nbsp;";

/// Puts the caret at `offset` inside the first text node of the element matching `selector`.
function caretAt(h: ReturnType<typeof harness>, selector: string, offset: number): void {
  const element = h.caret(selector);
  const doc = element.ownerDocument;
  const range = doc.createRange();
  range.setStart(element.firstChild as never, offset);
  range.collapse(true);
  const selection = doc.defaultView!.getSelection()!;
  selection.removeAllRanges();
  selection.addRange(range as never);
}

function runs(body: HTMLElement): TextRun[] {
  const first = documentBlocks(body)[0]!;
  const content: InlineContent[] = "Paragraph" in first ? first.Paragraph.content : [];
  return content.filter((item): item is { Text: TextRun } => "Text" in item).map((item) => item.Text);
}

describe("an address typed into the body becomes a link", () => {
  test("pressing space after an address links it and puts the space after the link", () => {
    const h = harness("<p id=p>see https://example.com/a</p>");
    caretAt(h, "#p", "see https://example.com/a".length);
    expect(autolinkBeforeCaret(h.editor, " ")).toBe(true);
    expect(h.html()).toBe(`<p id="p">see <a href="https://example.com/a">https://example.com/a</a>${NBSP}</p>`);
    // The caret is outside the link, so the next word the user types is not part of it.
    expect(h.caretHost()?.closest("a")).toBeNull();
  });

  test("a space typed before more text is a plain space", () => {
    const h = harness("<p id=p>https://example.comand more</p>");
    caretAt(h, "#p", "https://example.com".length);
    expect(autolinkBeforeCaret(h.editor, " ")).toBe(true);
    expect(h.html()).toBe(`<p id="p"><a href="https://example.com">https://example.com</a> and more</p>`);
  });

  test("the sentence's punctuation stays out of the link", () => {
    const h = harness("<p id=p>(see https://example.com/a).</p>");
    caretAt(h, "#p", "(see https://example.com/a).".length);
    autolinkBeforeCaret(h.editor, " ");
    expect(h.html()).toBe(
      `<p id="p">(see <a href="https://example.com/a">https://example.com/a</a>).${NBSP}</p>`,
    );
  });

  test("a www host is linked over https and keeps its written form", () => {
    const h = harness("<p id=p>www.example.com</p>");
    caretAt(h, "#p", "www.example.com".length);
    autolinkBeforeCaret(h.editor, null);
    expect(h.html()).toBe(`<p id="p"><a href="https://www.example.com">www.example.com</a></p>`);
  });

  test("the link reaches the document as a run with a target", () => {
    const h = harness("<p id=p>go https://example.com</p>");
    caretAt(h, "#p", "go https://example.com".length);
    autolinkBeforeCaret(h.editor, " ");
    expect(runs(h.editor).map((run) => [run.text, run.link])).toEqual([
      ["go ", undefined],
      ["https://example.com", "https://example.com"],
      [" ", undefined],
    ]);
  });

  test("an ordinary word, an address already linked, and a prefix alone are left alone", () => {
    for (const body of ["<p id=p>hello</p>", "<p id=p>https://</p>", "<p id=p>www.example</p>"]) {
      const h = harness(body);
      const text = h.caret("#p").textContent ?? "";
      caretAt(h, "#p", text.length);
      expect(autolinkBeforeCaret(h.editor, " ")).toBe(false);
    }
    const linked = harness(`<p><a id=a href="https://example.com">https://example.com</a></p>`);
    caretAt(linked, "#a", "https://example.com".length);
    expect(autolinkBeforeCaret(linked.editor, " ")).toBe(false);
  });

  test("only the schemes a message may carry are recognised", () => {
    expect(typedAddress("javascript:alert(1)")).toBeNull();
    expect(typedAddress("ftp://example.com")).toBeNull();
    expect(typedAddress("mailto:someone@example.com")?.href).toBe("mailto:someone@example.com");
    expect(typedAddress("HTTPS://EXAMPLE.COM")?.href).toBe("HTTPS://EXAMPLE.COM");
  });
});

describe("reading a link out of the editor", () => {
  test("a link and its marks become runs sharing one target", () => {
    const h = harness(`<p><a href="https://example.com">the <b>new</b> plan</a></p>`);
    expect(runs(h.editor).map((run) => [run.text, run.bold, run.link])).toEqual([
      ["the ", false, "https://example.com"],
      ["new", true, "https://example.com"],
      [" plan", false, "https://example.com"],
    ]);
  });

  test("a link whose target the product does not send reaches the document as text", () => {
    const h = harness(`<p><a href="javascript:alert(1)">click</a></p>`);
    expect(runs(h.editor)[0]?.link).toBeUndefined();
  });

  test("an unlinked run carries no link key", () => {
    const h = harness("<p>plain</p>");
    expect("link" in runs(h.editor)[0]!).toBe(false);
  });
});

describe("the link editor's address field", () => {
  test("keeps an allowed scheme and completes a host or an email address", () => {
    expect(normalizeLinkAddress(" https://example.com/a ")).toBe("https://example.com/a");
    expect(normalizeLinkAddress("example.com/docs")).toBe("https://example.com/docs");
    expect(normalizeLinkAddress("example.com:8080/x")).toBe("https://example.com:8080/x");
    expect(normalizeLinkAddress("someone@example.com")).toBe("mailto:someone@example.com");
    expect(normalizeLinkAddress("mailto:someone@example.com")).toBe("mailto:someone@example.com");
  });

  test("refuses what cannot be a link", () => {
    for (const typed of ["", "   ", "javascript:alert(1)", "file:///etc/passwd", "hello", "exa mple.com"]) {
      expect(normalizeLinkAddress(typed)).toBeNull();
    }
    expect(safeLinkHref("https://")).toBeNull();
  });
});

describe("making, changing and removing a link", () => {
  test("a selection becomes a link over the words it covers", () => {
    const h = harness("<p id=p>read the agenda first</p>");
    const paragraph = h.caret("#p");
    const doc = paragraph.ownerDocument;
    const range = doc.createRange();
    range.setStart(paragraph.firstChild as never, 5);
    range.setEnd(paragraph.firstChild as never, 15);
    const selection = doc.defaultView!.getSelection()!;
    selection.removeAllRanges();
    selection.addRange(range as never);
    applyLink(h.editor, saveSelection(h.editor), null, { href: "https://example.com", text: "" });
    expect(h.html()).toBe(`<p id="p">read <a href="https://example.com">the agenda</a> first</p>`);
  });

  test("a caret alone inserts the address as a link, or the words the user gave", () => {
    const h = harness("<p id=p>x</p>");
    caretAt(h, "#p", 1);
    applyLink(h.editor, saveSelection(h.editor), null, { href: "mailto:a@example.com", text: "" });
    expect(h.html()).toBe(`<p id="p">x<a href="mailto:a@example.com">a@example.com</a></p>`);

    const named = harness("<p id=p>x</p>");
    caretAt(named, "#p", 1);
    applyLink(named.editor, saveSelection(named.editor), null, { href: "https://example.com", text: "site" });
    expect(named.html()).toBe(`<p id="p">x<a href="https://example.com">site</a></p>`);
  });

  test("editing a link changes its target and, when asked, its words", () => {
    const h = harness(`<p><a id=a href="https://old.example">old</a></p>`);
    caretAt(h, "#a", 1);
    const link = linkAtCaret(h.editor);
    expect(link?.id).toBe("a");
    applyLink(h.editor, saveSelection(h.editor), link, { href: "https://new.example", text: "new" });
    expect(h.html()).toBe(`<p><a id="a" href="https://new.example">new</a></p>`);
  });

  test("a caret beside a link, not in it, removes nothing", () => {
    const h = harness(`<p id=p>see <a href="https://example.com">site</a> now</p>`);
    const paragraph = h.caret("#p");
    const doc = paragraph.ownerDocument;
    const range = doc.createRange();
    range.setStart(paragraph.lastChild as never, 0);
    range.collapse(true);
    const selection = doc.defaultView!.getSelection()!;
    selection.removeAllRanges();
    selection.addRange(range as never);
    removeLink(h.editor, saveSelection(h.editor));
    expect(h.html()).toBe(`<p id="p">see <a href="https://example.com">site</a> now</p>`);
  });

  test("removing a link keeps its words", () => {
    const h = harness(`<p>see <a id=a href="https://example.com">the <b>site</b></a>.</p>`);
    caretAt(h, "#a", 1);
    removeLink(h.editor, saveSelection(h.editor));
    expect(h.html()).toBe("<p>see the <b>site</b>.</p>");
  });
});
