// Growing mode: the editor reports the height its content needs and where its caret is, and a host
// lays it out from those (docs/composer-layout.md, rules 5 and 6). happy-dom lays nothing out, so
// the measurements are stood in for; what is tested is when a report goes out.

import { describe, expect, test } from "bun:test";
import { Window } from "happy-dom";

import { installGrowing } from "../src/growing";

/// Mutation observers deliver on a microtask, and a report waits for the next animation frame.
const settled = () => new Promise((resolve) => setTimeout(resolve, 40));

function mount() {
  const window = new Window();
  const document = window.document as unknown as Document;
  document.body.innerHTML = `
    <div class="toolbar"><button id="bold">B</button></div>
    <main id="editor" contenteditable="true"><p>One line</p></main>`;
  const editor = document.getElementById("editor") as HTMLElement;
  const toolbar = document.querySelector(".toolbar") as HTMLElement;
  const measured = { height: 120, caret: { top: 40, bottom: 58 } as { top: number; bottom: number } | null };
  const heights: number[] = [];
  const carets: [number, number][] = [];
  const growing = installGrowing(
    editor,
    toolbar,
    { height: (px) => heights.push(px), caret: (top, bottom) => carets.push([top, bottom]) },
    { contentHeight: () => measured.height, caret: () => measured.caret },
  );
  return { document, editor, growing, measured, heights, carets };
}

describe("growing mode", () => {
  test("reports nothing until the host switches it on", async () => {
    const { editor, heights, carets } = mount();
    editor.innerHTML = "<p>Typed before</p>";
    await settled();
    expect(heights).toEqual([]);
    expect(carets).toEqual([]);
  });

  test("marks the page and reports its height and caret once on", async () => {
    const { document, growing, heights, carets } = mount();
    growing.useGrowingComposer();
    await settled();
    expect(document.documentElement.classList.contains("growing")).toBe(true);
    expect(document.body.classList.contains("growing")).toBe(true);
    expect(heights).toEqual([120]);
    expect(carets).toEqual([[40, 58]]);
  });

  test("reports a new height when the text grows, and nothing when it did not", async () => {
    const { editor, growing, measured, heights } = mount();
    growing.useGrowingComposer();
    await settled();
    measured.height = 160.4;
    editor.innerHTML = "<p>One line</p><p>And another</p>";
    await settled();
    editor.innerHTML = "<p>One line</p><p>And anothex</p>";
    await settled();
    expect(heights).toEqual([120, 161]);
  });

  test("says nothing about a caret outside the message", async () => {
    const { editor, growing, measured, carets } = mount();
    measured.caret = null;
    growing.useGrowingComposer();
    await settled();
    editor.innerHTML = "<p>Changed</p>";
    await settled();
    expect(carets).toEqual([]);
  });

  test("sends both again for a host that has just announced it takes them", async () => {
    const { growing, heights, carets } = mount();
    growing.useGrowingComposer();
    await settled();
    growing.resend();
    expect(heights).toEqual([120, 120]);
    expect(carets).toEqual([[40, 58], [40, 58]]);
  });

  test("never reports a caret that ends above where it starts", async () => {
    // The core refuses such a notice outright, so the page must not send one.
    const { growing, measured, carets } = mount();
    measured.caret = { top: 50.6, bottom: 50.2 };
    growing.useGrowingComposer();
    await settled();
    expect(carets).toHaveLength(1);
    const [top, bottom] = carets[0] ?? [0, -1];
    expect(bottom).toBeGreaterThanOrEqual(top);
  });
});
