// The desktop toolbar keeps to one row and moves what does not fit behind More, last first; a
// touch screen keeps every control in its sideways-scrolling row (docs/composer-layout.md, rule 7).
// happy-dom lays nothing out, so how many controls fit is stood in for.

import { describe, expect, test } from "bun:test";
import { Window } from "happy-dom";

import { installOverflow } from "../src/toolbar_overflow";

function mount(fits: number) {
  const window = new Window();
  const document = window.document as unknown as Document;
  document.body.innerHTML = `
    <div class="toolbar">
      <button id="bold">B</button><button id="italic">I</button><span class="sep"></span>
      <button id="list">L</button><button id="indent">></button><span class="sep"></span>
      <button id="link">K</button><button id="rule">-</button>
      <div class="menu toolbar-more" hidden>
        <button id="more">...</button><div class="toolbar toolbar-overflow" id="panel"></div>
      </div>
    </div>`;
  const root = document.querySelector(".toolbar") as HTMLElement;
  const more = document.querySelector(".toolbar-more") as HTMLElement;
  const panel = document.getElementById("panel") as HTMLElement;
  const room = { fits };
  // The row overflows while it holds more controls than fit, More included once it is drawn.
  const inRow = () => Array.from(root.children).filter((child) => child !== more).length;
  const overflow = installOverflow(root, more, panel, {
    overflows: () => inRow() + (more.hidden ? 0 : 1) > room.fits,
  });
  const ids = (parent: Element) =>
    Array.from(parent.children)
      .filter((child) => child !== more)
      .map((child) => child.id || "sep");
  return { document, root, more, panel, room, overflow, ids };
}

describe("the desktop toolbar's overflow", () => {
  test("keeps every control in the row, and More hidden, when they fit", () => {
    const { root, more, panel, overflow, ids } = mount(20);
    expect(overflow.fit()).toBe(false);
    expect(more.hidden).toBe(true);
    expect(ids(root)).toEqual(["bold", "italic", "sep", "list", "indent", "sep", "link", "rule"]);
    expect(ids(panel)).toEqual([]);
  });

  test("moves the last controls behind More, in their order, and leaves no separator at the row's end", () => {
    const { root, more, panel, overflow, ids } = mount(5);
    expect(overflow.fit()).toBe(true);
    expect(more.hidden).toBe(false);
    expect(ids(root)).toEqual(["bold", "italic", "sep", "list"]);
    expect(ids(panel)).toEqual(["indent", "sep", "link", "rule"]);
    expect(root.lastElementChild).toBe(more);
  });

  test("puts controls back in the row when it widens", () => {
    const { root, more, panel, room, overflow, ids } = mount(4);
    overflow.fit();
    room.fits = 20;
    expect(overflow.fit()).toBe(true);
    expect(more.hidden).toBe(true);
    expect(ids(root)).toEqual(["bold", "italic", "sep", "list", "indent", "sep", "link", "rule"]);
    expect(ids(panel)).toEqual([]);
  });

  test("answers that nothing changed when the same controls stay behind More", () => {
    const { overflow } = mount(5);
    overflow.fit();
    expect(overflow.fit()).toBe(false);
  });

  test("keeps every control in the row on a touch screen, which scrolls it sideways", () => {
    const { document, root, more, panel, overflow, ids } = mount(3);
    overflow.fit();
    document.body.classList.add("native-chrome");
    overflow.fit();
    expect(more.hidden).toBe(true);
    expect(ids(root)).toEqual(["bold", "italic", "sep", "list", "indent", "sep", "link", "rule"]);
    expect(ids(panel)).toEqual([]);
  });
});
