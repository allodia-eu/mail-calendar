// Framed mode: the editor reports where its caret is and how far the message is scrolled, and a
// desktop host lays its column out from those (docs/composer-layout.md, rules 5 to 7). happy-dom
// lays nothing out, so the measurements are stood in for; what is tested is when a report goes out.

import { describe, expect, test } from "bun:test";
import { Window } from "happy-dom";

import { installFramed, lineIn, shownIn } from "../src/framed";

/// Mutation observers deliver on a microtask, and a report waits for the next animation frame.
const settled = () => new Promise((resolve) => setTimeout(resolve, 40));

function mount() {
  const window = new Window();
  const document = window.document as unknown as Document;
  document.body.innerHTML = `
    <div class="toolbar"><button id="bold">B</button></div>
    <main id="editor" contenteditable="true"><p>One line</p></main>`;
  const editor = document.getElementById("editor") as HTMLElement;
  const measured = {
    caret: { top: 40, bottom: 58 } as { top: number; bottom: number } | null,
    scroll: 0,
  };
  const carets: [number, number][] = [];
  const scrolls: number[] = [];
  const framed = installFramed(
    editor,
    { caret: (top, bottom) => carets.push([top, bottom]), scroll: (px) => scrolls.push(px) },
    { caret: () => measured.caret, scrollTop: () => measured.scroll },
  );
  const scrolled = () => editor.dispatchEvent(new window.Event("scroll") as never);
  return { document, editor, framed, measured, carets, scrolls, scrolled };
}

describe("framed mode", () => {
  test("reports nothing until the host switches it on", async () => {
    const { editor, carets, scrolls, scrolled } = mount();
    editor.innerHTML = "<p>Typed before</p>";
    scrolled();
    await settled();
    expect(carets).toEqual([]);
    expect(scrolls).toEqual([]);
  });

  test("marks the page and reports where the message is scrolled once on", async () => {
    const { document, framed, scrolls } = mount();
    framed.useFramedComposer();
    await settled();
    expect(document.documentElement.classList.contains("framed")).toBe(true);
    expect(document.body.classList.contains("framed")).toBe(true);
    expect(scrolls).toEqual([0]);
  });

  test("reports the caret when the text changes, and only when it moved", async () => {
    const { editor, framed, measured, carets } = mount();
    framed.useFramedComposer();
    editor.innerHTML = "<p>One line</p><p>And another</p>";
    await settled();
    editor.innerHTML = "<p>One line</p><p>And anothex</p>";
    await settled();
    measured.caret = { top: 60.4, bottom: 78.2 };
    editor.innerHTML = "<p>One line</p><p>And another</p><p>Third</p>";
    await settled();
    expect(carets).toEqual([[40, 58], [60, 79]]);
  });

  test("reports the message's scroll as it moves, and not the caret", async () => {
    const { framed, measured, carets, scrolls, scrolled } = mount();
    framed.useFramedComposer();
    await settled();
    measured.scroll = 120.4;
    measured.caret = { top: -80, bottom: -62 };
    scrolled();
    await settled();
    expect(scrolls).toEqual([0, 120]);
    expect(carets).toEqual([]);
  });

  test("says nothing about a caret outside the message", async () => {
    const { editor, framed, measured, carets } = mount();
    measured.caret = null;
    framed.useFramedComposer();
    editor.innerHTML = "<p>Changed</p>";
    await settled();
    expect(carets).toEqual([]);
  });

  test("sends both again for a host that has just announced it takes them", async () => {
    const { framed, carets, scrolls } = mount();
    framed.useFramedComposer();
    await settled();
    framed.resend();
    expect(carets).toEqual([[40, 58]]);
    expect(scrolls).toEqual([0, 0]);
  });

  test("never reports a caret above the page or ending above where it starts", async () => {
    // The core refuses such a notice outright, so the page must not send one.
    const { framed, measured, carets } = mount();
    measured.caret = { top: -12.6, bottom: -14 };
    framed.useFramedComposer();
    framed.resend();
    expect(carets).toEqual([[0, 0]]);
  });
});

describe("a caret with no rectangle of its own", () => {
  const style = { paddingTop: "12px", paddingBottom: "8px", lineHeight: "20px", fontSize: "14px" };

  test("is the first line of an empty message, never the whole message", () => {
    // An empty message is the editor alone, as tall as the column's visible part.
    expect(lineIn({ top: 36, bottom: 754 }, style, false)).toEqual({ top: 48, bottom: 68 });
  });

  test("after the last block is that block's last line", () => {
    expect(lineIn({ top: 100, bottom: 200 }, style, true)).toEqual({ top: 172, bottom: 192 });
  });

  test("a normal line height is the font's size and a fifth", () => {
    const normal = { ...style, lineHeight: "normal" };
    expect(lineIn({ top: 0, bottom: 500 }, normal, false)).toEqual({ top: 12, bottom: 12 + 14 * 1.2 });
  });

  test("never runs past the block", () => {
    expect(lineIn({ top: 0, bottom: 20 }, style, false)).toEqual({ top: 12, bottom: 20 });
  });
});

describe("a caret the message's own scroll took out of its box", () => {
  const box = { top: 36, bottom: 600 };

  test("is not reported, above the box or below it", () => {
    // Behind the toolbar, scrolled up out of the box; and a select-all's end far down the message.
    expect(shownIn({ top: 10, bottom: 28 }, box)).toBeNull();
    expect(shownIn({ top: 1800, bottom: 1818 }, box)).toBeNull();
  });

  test("is reported while any of its line is in the box", () => {
    expect(shownIn({ top: 48, bottom: 66 }, box)).toEqual({ top: 48, bottom: 66 });
    expect(shownIn({ top: 590, bottom: 608 }, box)).toEqual({ top: 590, bottom: 608 });
  });
});
