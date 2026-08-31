// Mounts the REAL markup from `src/index.html` and installs the REAL toolbar over it.
//
// This is the test the module split earns. `installToolbar` resolves its controls by id and throws
// when one is missing, so a button renamed in the markup but not in `toolbar.ts` (or the reverse) is
// a composer that fails to initialise; no toolbar, no seams, and on a real host a blank editor with
// nothing in the log to say why. Here it is a failing assertion.
//
// It mounts the markup rather than executing the built `editor.html`, because happy-dom's script
// sandbox does not expose every global a bundle uses (`Set`, among others): running the artifact
// there would test the sandbox, not the editor. `bun run check` covers the other half: that the
// committed artifact is what these sources produce.

import { beforeEach, describe, expect, test } from "bun:test";
import { Window } from "happy-dom";

import { DEFAULT_LABELS, type Labels } from "../src/labels";
import { installToolbar, type Toolbar } from "../src/toolbar";

const SHELL = await Bun.file(new URL("../src/index.html", import.meta.url)).text();
const BODY = SHELL.slice(SHELL.indexOf("<body>") + "<body>".length, SHELL.indexOf("<!--EDITOR_BUNDLE-->"));

let window: Window;
let document: Document;
let editor: HTMLElement;
let toolbar: Toolbar;
let labels: Labels;

function mount() {
  window = new Window();
  document = window.document as unknown as Document;
  document.body.innerHTML = BODY;
  editor = document.getElementById("editor") as HTMLElement;
  labels = DEFAULT_LABELS;
  toolbar = installToolbar(editor, document.querySelector(".toolbar") as HTMLElement, () => labels);
}

function click(id: string) {
  document.getElementById(id)!.dispatchEvent(new window.Event("click", { bubbles: true }) as never);
}

beforeEach(mount);

describe("wiring", () => {
  test("every control the toolbar resolves is present in the markup", () => {
    // `installToolbar` throws on a missing id, so `beforeEach` having succeeded is the assertion,
    // name them too, so a failure says which control went missing rather than just "threw".
    for (const id of [
      "font-size", "bullet-list", "ordered-list", "indent", "outdent",
      "text-colour", "text-colour-menu", "text-colour-bar",
      "highlight", "highlight-menu", "highlight-bar",
      "table", "table-menu",
    ]) {
      expect(document.getElementById(id), `#${id}`).not.toBeNull();
    }
  });

  test("a toolbar press does not steal the selection from the editor", () => {
    // A <button> collapses the selection on mousedown, so bold/indent/colour would apply to
    // nothing. The handler is delegated, which is what covers the popovers' buttons too.
    const event = new window.MouseEvent("mousedown", { bubbles: true, cancelable: true });
    document.getElementById("indent")!.dispatchEvent(event as never);
    expect(event.defaultPrevented).toBe(true);
  });

  test("nor does a press on a control that is not a button", () => {
    // The table picker's cells are <span>s: they steal nothing, but a mousedown on them starts a
    // new selection where the pointer went down, which loses the caret just as thoroughly. With it
    // gone `insertTable` has no block to anchor to and appends the table at the end of the
    // document; below the quoted original on a reply.
    click("table");
    const cell = document.querySelector("#table-menu .grid-picker span") as HTMLElement;
    const event = new window.MouseEvent("mousedown", { bubbles: true, cancelable: true });
    cell.dispatchEvent(event as never);
    expect(event.defaultPrevented).toBe(true);
  });

  test("but the font-size select still opens its native dropdown", () => {
    const event = new window.MouseEvent("mousedown", { bubbles: true, cancelable: true });
    document.getElementById("font-size")!.dispatchEvent(event as never);
    expect(event.defaultPrevented).toBe(false);
  });
});

describe("labels", () => {
  test("reach the controls they name, and an unnamed key keeps English", () => {
    labels = { ...DEFAULT_LABELS, indent: "Inspringen", table: "Tabel", placeholder: "Schrijf je bericht" };
    toolbar.applyLabels(labels);
    expect(document.getElementById("indent")!.title).toBe("Inspringen");
    expect(document.getElementById("table")!.title).toBe("Tabel");
    expect(editor.dataset.placeholder).toBe("Schrijf je bericht");
    expect(editor.getAttribute("aria-label")).toBe("Schrijf je bericht");
    expect(document.getElementById("outdent")!.title).toBe("Decrease indent");
  });

  test("rename the font sizes without touching the tokens the document is keyed on", () => {
    labels = { ...DEFAULT_LABELS, sizeLarge: "Groot" };
    toolbar.applyLabels(labels);
    const select = document.getElementById("font-size") as HTMLSelectElement;
    const large = Array.from(select.options).find((option) => option.value === "Large")!;
    expect(large.textContent).toBe("Groot");
    expect(Array.from(select.options).map((o) => o.value)).toEqual([
      "Normal", "Small", "Large", "Huge",
    ]);
  });
});

describe("popovers", () => {
  test("the colour palette opens with its swatches and a reset", () => {
    const panel = document.getElementById("text-colour-menu")!;
    expect(panel.hasAttribute("hidden")).toBe(true);
    click("text-colour");
    expect(panel.hasAttribute("hidden")).toBe(false);
    expect(panel.querySelectorAll(".swatches button").length).toBe(12);
    expect(panel.querySelector(".reset")!.textContent).toBe("Automatic");
  });

  test("the highlight palette names its reset differently: it clears, it does not restore", () => {
    click("highlight");
    expect(document.querySelector("#highlight-menu .reset")!.textContent).toBe("No highlight");
  });

  test("opening one popover closes the other", () => {
    click("text-colour");
    click("highlight");
    expect(document.getElementById("text-colour-menu")!.hasAttribute("hidden")).toBe(true);
    expect(document.getElementById("highlight-menu")!.hasAttribute("hidden")).toBe(false);
  });

  test("Escape closes an open popover", () => {
    click("text-colour");
    document.dispatchEvent(new window.KeyboardEvent("keydown", { key: "Escape", bubbles: true }) as never);
    expect(document.getElementById("text-colour-menu")!.hasAttribute("hidden")).toBe(true);
  });

  test("picking a swatch records it on the button, so the control shows what it applies", () => {
    click("text-colour");
    const swatch = document.querySelector("#text-colour-menu .swatches button:nth-child(4)") as HTMLElement;
    expect(swatch.title).toBe("#ff0000");
    swatch.dispatchEvent(new window.Event("click", { bubbles: true }) as never);
    expect(document.getElementById("text-colour-bar")!.style.background).toBe("#ff0000");
    expect(document.getElementById("text-colour-menu")!.hasAttribute("hidden")).toBe(true);
  });

  test("the table menu offers only the size picker outside a table", () => {
    click("table");
    const panel = document.getElementById("table-menu")!;
    expect(panel.querySelectorAll(".grid-picker span").length).toBe(48);
    expect(panel.querySelector(".menu-items")).toBeNull();
  });

  test("the shape commands appear once the caret is in a table", () => {
    editor.innerHTML = "<table><tbody><tr><td id=cell>x</td></tr></tbody></table>";
    const range = document.createRange();
    range.selectNodeContents(document.getElementById("cell")! as never);
    range.collapse(true);
    const selection = window.getSelection()!;
    selection.removeAllRanges();
    selection.addRange(range as never);

    click("table");
    const items = document.querySelectorAll("#table-menu .menu-items button");
    expect(Array.from(items).map((item) => item.textContent)).toEqual([
      "Insert row above",
      "Insert row below",
      "Insert column left",
      "Insert column right",
      "Delete row",
      "Delete column",
      "Delete table",
    ]);
  });

  test("the size picker inserts the table it previewed", () => {
    editor.innerHTML = "<p id=p><br></p>";
    click("table");
    // The 3rd row, 2nd column of the picker is a 3x2 table.
    const cells = document.querySelectorAll("#table-menu .grid-picker span");
    (cells[2 * 8 + 1] as HTMLElement).dispatchEvent(new window.Event("click", { bubbles: true }) as never);
    expect(editor.querySelectorAll("tr").length).toBe(3);
    expect(editor.querySelectorAll("tr:first-child td").length).toBe(2);
  });
});

describe("a popover stays inside the editor's viewport", () => {
  // The toolbar wraps. In a narrow composer; macOS's detail column, a split Windows pane; the
  // last controls drop to a second row and start again at the LEFT, so the table button is at the
  // right edge in a wide window and the left edge in a narrow one. The table menu used to carry a
  // hardcoded `align-end`, which in the narrow case opened it off the left of the WebView: clipped
  // away entirely, no scrollbar, no overflow, just a menu that is not there.
  function placeAt(buttonLeft: number, panelWidth: number, viewportWidth: number) {
    (window as unknown as { innerWidth: number }).innerWidth = viewportWidth;
    const button = document.getElementById("table")!;
    const panel = document.getElementById("table-menu")!;
    button.getBoundingClientRect = (() => ({ left: buttonLeft, width: 32 })) as never;
    panel.getBoundingClientRect = (() => ({ left: buttonLeft, width: panelWidth })) as never;
    return panel;
  }

  test("hangs from the left of its button when there is room", () => {
    const panel = placeAt(24, 180, 600);
    click("table");
    expect(panel.classList.contains("align-end")).toBe(false);
  });

  test("flips to the right edge only when it would overflow", () => {
    const panel = placeAt(500, 180, 600);
    click("table");
    expect(panel.classList.contains("align-end")).toBe(true);
  });

  test("the same button re-decides when the pane is resized", () => {
    // One composer, dragged narrower: the button wraps to the second row's left edge and the menu
    // has to come back. A class set once at open and never cleared would leave it clipped.
    const panel = placeAt(500, 180, 600);
    click("table");
    expect(panel.classList.contains("align-end")).toBe(true);
    click("table"); // close
    placeAt(24, 180, 600);
    click("table");
    expect(panel.classList.contains("align-end")).toBe(false);
  });
});
