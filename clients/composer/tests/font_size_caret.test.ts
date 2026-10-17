// A size picked at a bare caret, with nothing selected. The engine keeps it as a typing style, so
// the first run typed afterwards is where the exact size has to be stamped. happy-dom has no
// `execCommand`, so these build the run the engine would have made and type into it by hand.

import { describe, expect, test } from "bun:test";

import { applyFontSize, stampTypedSize } from "../src/format";
import { nearestLegacyLevel } from "../src/typography";
import { harness } from "./support";

function type(editor: HTMLElement) {
  const window = editor.ownerDocument.defaultView!;
  editor.dispatchEvent(new window.Event("beforeinput", { bubbles: true }));
  editor.dispatchEvent(new window.Event("input", { bubbles: true }));
}

describe("a size picked at a bare caret", () => {
  test("is drawn at the nearest real level, never the size-7 marker", () => {
    expect(nearestLegacyLevel(13)).toBe("2");
    expect(nearestLegacyLevel(15)).toBe("3");
    expect(nearestLegacyLevel(18)).toBe("4");
    expect(nearestLegacyLevel(24)).toBe("5");
  });

  test("is stamped exactly on the run typed at the caret", () => {
    const { editor, caretEnd } = harness(`<p>Some <font size="4">bigger</font></p>`);
    caretEnd("font");
    applyFontSize(editor, 18);
    type(editor);
    const run = editor.querySelector("font") as HTMLElement;
    expect(run.dataset.size).toBe("18");
    expect(run.style.fontSize).toBe("18px");
  });

  test("is stamped on the span the CSS path makes", () => {
    const { editor, caretEnd } = harness(`<p><span style="font-size: large">bigger</span></p>`);
    caretEnd("span");
    applyFontSize(editor, 18);
    type(editor);
    expect((editor.querySelector("span") as HTMLElement).dataset.size).toBe("18");
  });

  test("is not stamped when the caret moved before anything was typed", () => {
    // Moving the caret drops the typing style, so the run typed into next is someone else's.
    const { editor, caretEnd } = harness(`<p>Some <font size="4">bigger</font></p><p>Elsewhere</p>`);
    caretEnd("font");
    applyFontSize(editor, 24);
    caretEnd("p:last-child");
    type(editor);
    expect((editor.querySelector("font") as HTMLElement).dataset.size).toBeUndefined();
  });
});

describe("stamping the typed run", () => {
  test("leaves a run that already carries the toolbar's size", () => {
    const { editor, caretEnd } = harness(`<p><span data-size="13" style="font-size: 13px">small</span></p>`);
    caretEnd("span");
    stampTypedSize(editor, 24);
    expect((editor.querySelector("span") as HTMLElement).dataset.size).toBe("13");
  });

  test("never reaches into the quoted original", () => {
    const { editor, caretEnd } = harness(`<div class="allodia-quote"><font size="4">quoted</font></div>`);
    caretEnd("font");
    stampTypedSize(editor, 24);
    expect((editor.querySelector("font") as HTMLElement).dataset.size).toBeUndefined();
  });
});
