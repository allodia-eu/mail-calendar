import { describe, expect, test } from "bun:test";

import { documentBlocks } from "../src/document";
import { restoreSelection, saveSelection } from "../src/dom";
import { indentSelection, indentItem, outdentItem } from "../src/lists";
import { harness } from "./support";

describe("indent", () => {
  test("nests an item under the one above it", () => {
    const h = harness("<ul><li id=a>a</li><li id=b>b</li></ul>");
    expect(indentItem(h.caret("#b"))).toBe(true);
    expect(h.html()).toBe('<ul><li id="a">a<ul><li id="b">b</li></ul></li></ul>');
  });

  test("does nothing to the first item: there is nothing to nest it under", () => {
    const h = harness("<ul><li id=a>a</li><li id=b>b</li></ul>");
    expect(indentItem(h.caret("#a"))).toBe(false);
    expect(h.html()).toBe('<ul><li id="a">a</li><li id="b">b</li></ul>');
  });

  test("reuses the sub-list already under the item above", () => {
    const h = harness("<ul><li id=a>a<ul><li id=b>b</li></ul></li><li id=c>c</li></ul>");
    indentItem(h.caret("#c"));
    expect(h.html()).toBe('<ul><li id="a">a<ul><li id="b">b</li><li id="c">c</li></ul></li></ul>');
  });

  test("a run of selected items collapses into one sub-list, in order", () => {
    const h = harness("<ul><li id=a>a</li><li id=b>b</li><li id=c>c</li></ul>");
    h.select("#b", "#c");
    expect(indentSelection(h.editor, false)).toBe(true);
    expect(h.html()).toBe(
      '<ul><li id="a">a<ul><li id="b">b</li><li id="c">c</li></ul></li></ul>',
    );
  });

  test("an ordered list nests an ordered sub-list", () => {
    const h = harness("<ol><li id=a>a</li><li id=b>b</li></ol>");
    indentItem(h.caret("#b"));
    expect(h.html()).toBe('<ol><li id="a">a<ol><li id="b">b</li></ol></li></ol>');
  });

  test("reports no list when the caret is in a plain paragraph", () => {
    const h = harness("<p id=p>text</p>");
    h.caret("#p");
    expect(indentSelection(h.editor, false)).toBe(false);
  });
});

describe("outdent", () => {
  test("a nested item rejoins its parent list after the item it was under", () => {
    const h = harness("<ul><li id=a>a<ul><li id=b>b</li></ul></li></ul>");
    expect(outdentItem(h.caret("#b"))).toBe(true);
    expect(h.html()).toBe('<ul><li id="a">a</li><li id="b">b</li></ul>');
  });

  test("the items after it keep their depth by becoming its sub-list", () => {
    const h = harness("<ul><li id=a>a<ul><li id=b>b</li><li id=c>c</li></ul></li></ul>");
    outdentItem(h.caret("#b"));
    expect(h.html()).toBe(
      '<ul><li id="a">a</li><li id="b">b<ul><li id="c">c</li></ul></li></ul>',
    );
  });

  test("a top-level item leaves the list, splitting it around the paragraph", () => {
    const h = harness("<ul><li id=a>a</li><li id=b>b</li><li id=c>c</li></ul>");
    outdentItem(h.caret("#b"));
    expect(h.html()).toBe('<ul><li id="a">a</li></ul><p>b</p><ul><li id="c">c</li></ul>');
  });

  test("a top-level item's own sub-list is promoted rather than dropped", () => {
    const h = harness("<ul><li id=a>a<ul><li id=b>b</li></ul></li></ul>");
    outdentItem(h.caret("#a"));
    expect(h.html()).toBe('<p>a</p><ul><li id="b">b</li></ul>');
  });

  test("outdenting the only item removes the empty list", () => {
    const h = harness("<ul><li id=a>a</li></ul>");
    outdentItem(h.caret("#a"));
    expect(h.html()).toBe("<p>a</p>");
  });

  test("a selected run outdents to a flat list", () => {
    const h = harness("<ul><li id=a>a<ul><li id=b>b</li><li id=c>c</li></ul></li></ul>");
    h.select("#b", "#c");
    indentSelection(h.editor, true);
    expect(h.html()).toBe(
      '<ul><li id="a">a</li><li id="b">b</li><li id="c">c</li></ul>',
    );
  });
});

describe("round-trip into the composer document", () => {
  test("a nested list survives as a child list, not a flattened one", () => {
    const h = harness("<ul><li id=a>a</li><li id=b>b</li></ul>");
    indentItem(h.caret("#b"));
    const blocks = documentBlocks(h.editor);
    expect(blocks).toEqual([
      {
        List: {
          kind: "Bullet",
          items: [
            {
              content: [{ Text: { text: "a", bold: false, italic: false, underline: false } }],
              child: {
                kind: "Bullet",
                items: [
                  {
                    content: [
                      { Text: { text: "b", bold: false, italic: false, underline: false } },
                    ],
                    child: null,
                  },
                ],
              },
            },
          ],
        },
      },
    ]);
  });

  test("a bullet list nesting an ordered one keeps both kinds", () => {
    const h = harness("<ul><li>a<ol><li>one</li></ol></li></ul>");
    const blocks = documentBlocks(h.editor);
    const list = (blocks[0] as { List: { kind: string; items: { child: { kind: string } | null }[] } }).List;
    expect(list.kind).toBe("Bullet");
    expect(list.items[0]!.child!.kind).toBe("Ordered");
  });
});

describe("the caret follows the item, not the structure", () => {
  // Indenting reparents the <li>, and WebKit drops the selection when a node is reparented,
  // collapsing it to the previous block: the caret landed on the bullet ABOVE the one being
  // indented and the user had to click back into their own line. Blink keeps the selection, which
  // is why this only showed on macOS and iOS; and why happy-dom, which behaves like Blink, cannot
  // reproduce it. So these tests split the job: the first drives the whole command and pins the
  // intent, the second simulates the collapse to exercise the restore, which is the part that
  // would actually be broken.

  test("indenting leaves the caret in the item that moved", () => {
    const h = harness("<ul><li id=a>Bullet 1</li><li id=b>Bullet 2</li></ul>");
    h.caret("#b");
    indentSelection(h.editor, false);
    expect(h.caretHost()?.closest("li")?.id).toBe("b");
  });

  test("the caret is put back even when the edit drops it", () => {
    const h = harness("<ul><li id=a>Bullet 1</li><li id=b>Bullet 2</li></ul>");
    const b = h.caret<HTMLLIElement>("#b");
    const caret = saveSelection(h.editor);
    indentItem(b);
    // What WebKit does to the selection during that reparent.
    h.caret("#a");
    restoreSelection(h.editor, caret);
    expect(h.caretHost()?.closest("li")?.id).toBe("b");
  });

  test("a caret whose node did not survive leaves the command's own placement alone", () => {
    // Outdenting out of a list replaces the <li> with a <p>, taking an empty item's placeholder
    // <br> with it. Restoring must not fight the caret the command deliberately placed.
    const h = harness("<ul><li id=a><br></li></ul>");
    const a = h.caret<HTMLLIElement>("#a");
    const caret = saveSelection(h.editor);
    outdentItem(a);
    restoreSelection(h.editor, caret);
    expect(h.caretHost()?.tagName).toBe("P");
  });

  test("outdenting leaves the caret in the item that moved", () => {
    const h = harness("<ul><li id=a>a<ul><li id=b>b</li></ul></li></ul>");
    h.caret("#b");
    indentSelection(h.editor, true);
    expect(h.caretHost()?.closest("li")?.id).toBe("b");
  });
});
