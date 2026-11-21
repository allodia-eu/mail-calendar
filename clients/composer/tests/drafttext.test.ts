// A drafted reply's formatting (`docs/ai.md` rule 21): a Markdown subset built as elements, never
// parsed as markup.

import { describe, expect, test } from "bun:test";

import { draftBlocks } from "../src/drafttext";
import { harness } from "./support";

function drawn(text: string): HTMLElement {
  const { editor } = harness("");
  for (const block of draftBlocks(editor.ownerDocument, text)) editor.appendChild(block);
  return editor;
}

const html = (editor: HTMLElement) => editor.innerHTML;

describe("a drafted reply's formatting", () => {
  test("bold and italic become marks, not asterisks", () => {
    expect(html(drawn("It is **ready** and *tested*."))).toBe("<div>It is <b>ready</b> and <i>tested</i>.</div>");
  });

  test("a heading line is drawn as a bold line", () => {
    expect(html(drawn("## Next steps\nCall me."))).toBe("<div><b>Next steps</b></div><div>Call me.</div>");
  });

  test("an empty line keeps its height", () => {
    expect(html(drawn("Hi,\n\nYes."))).toBe("<div>Hi,</div><div><br></div><div>Yes.</div>");
  });

  test("dash, star and bullet lines become one bulleted list", () => {
    const editor = drawn("Open points:\n- one\n* **two**: done\n• three");
    expect(editor.querySelectorAll("ul").length).toBe(1);
    expect(editor.querySelectorAll("ul li").length).toBe(3);
    expect(editor.querySelector("ul li b")?.textContent).toBe("two");
  });

  test("numbered lines become one ordered list, even with blank lines between the items", () => {
    const editor = drawn("1. first\n\n2) second\n\n3. third\n\nThanks,");
    expect(editor.querySelectorAll("ol").length).toBe(1);
    expect(editor.querySelectorAll("ol li").length).toBe(3);
    expect(editor.lastElementChild?.textContent).toBe("Thanks,");
  });

  test("a list of the other kind starts a list of its own", () => {
    const editor = drawn("- a\n1. b");
    expect(Array.from(editor.children).map((child) => child.tagName)).toEqual(["UL", "OL"]);
  });

  test("an unmatched marker and a lone star stay as written", () => {
    const editor = drawn("2 * 3 is **six\nsnake_case and *emphasis *");
    expect(editor.querySelector("b, i")).toBeNull();
    expect(editor.textContent).toBe("2 * 3 is **sixsnake_case and *emphasis *");
  });

  test("markup inside the formatting is still text", () => {
    const editor = drawn("**<img src=x onerror=alert(1)>**\n- <a href=x>link</a>\n<script>x()</script>");
    expect(editor.querySelector("img, a, script")).toBeNull();
    expect(editor.querySelector("b")?.textContent).toBe("<img src=x onerror=alert(1)>");
  });

  test("any line ending splits lines", () => {
    expect(drawn("one\r\ntwo\rthree").children.length).toBe(3);
  });
});
