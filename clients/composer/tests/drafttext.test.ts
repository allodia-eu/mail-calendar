import { describe, expect, test } from "bun:test";
import { documentBlocks } from "../src/document";
import { setComposerDraftText } from "../src/seeds";
import { harness } from "./support";

const SIGNATURE = `<div class="allodia-signature" data-signature-plain="Sam"><p>Sam</p></div>`;

describe("a drafted reply's formatting", () => {
  test("bold and italic become marks, not asterisks", () => {
    const { editor, html } = harness(SIGNATURE);
    setComposerDraftText(editor, "It is **ready** and *tested*.");
    expect(html()).toBe(`<div>It is <b>ready</b> and <i>tested</i>.</div>${SIGNATURE}`);
  });

  test("a heading line is drawn as a bold line", () => {
    const { editor, html } = harness("");
    setComposerDraftText(editor, "## Next steps\nCall me.");
    expect(html()).toBe("<div><b>Next steps</b></div><div>Call me.</div>");
  });

  test("dash, star and bullet lines become one bulleted list", () => {
    const { editor } = harness(SIGNATURE);
    setComposerDraftText(editor, "Open points:\n- one\n* **two**: done\n• three");
    const blocks = documentBlocks(editor);
    const list = blocks[1] as { List: { kind: string; items: { content: unknown[] }[] } };
    expect(list.List.kind).toBe("Bullet");
    expect(list.List.items.length).toBe(3);
    expect(editor.querySelector("ul li b")?.textContent).toBe("two");
  });

  test("numbered lines become one ordered list, even with blank lines between the items", () => {
    const { editor } = harness("");
    setComposerDraftText(editor, "1. first\n\n2) second\n\n3. third\n\nThanks,");
    expect(editor.querySelectorAll("ol").length).toBe(1);
    expect(editor.querySelectorAll("ol li").length).toBe(3);
    expect(editor.lastElementChild?.textContent).toBe("Thanks,");
  });

  test("a list of the other kind starts a list of its own", () => {
    const { editor } = harness("");
    setComposerDraftText(editor, "- a\n1. b");
    expect(Array.from(editor.children).map((child) => child.tagName)).toEqual(["UL", "OL"]);
  });

  test("an unmatched marker and a lone star stay as written", () => {
    const { editor } = harness("");
    setComposerDraftText(editor, "2 * 3 is **six\nsnake_case and *emphasis *");
    expect(editor.querySelector("b, i")).toBeNull();
    expect(editor.textContent).toBe("2 * 3 is **sixsnake_case and *emphasis *");
  });

  test("markup inside the formatting is still text", () => {
    const { editor } = harness("");
    setComposerDraftText(editor, "**<img src=x onerror=alert(1)>**\n- <a href=x>link</a>");
    expect(editor.querySelector("img, a")).toBeNull();
    expect(editor.querySelector("b")?.textContent).toBe("<img src=x onerror=alert(1)>");
  });

  test("the caret ends in the last list item when the draft ends with a list", () => {
    const { editor, caretHost } = harness(SIGNATURE);
    setComposerDraftText(editor, "Points:\n- first\n- last");
    expect(caretHost()?.textContent).toBe("last");
  });
});
