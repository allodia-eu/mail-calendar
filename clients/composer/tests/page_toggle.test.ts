// The toolbar's sun or moon: drawn only where there is something to switch, and saying what a
// press does.

import { describe, expect, test } from "bun:test";

import { DEFAULT_LABELS } from "../src/labels";
import type { PageScheme } from "../src/page";
import { installPageToggle } from "../src/page_toggle";
import { harness } from "./support";

function setup() {
  const { editor } = harness("");
  const doc = editor.ownerDocument;
  const toolbar = doc.createElement("div");
  toolbar.innerHTML =
    '<span class="sep" id="page-background-sep" hidden></span><button id="page-background" hidden></button>';
  doc.body.prepend(toolbar);
  const state = { page: null as PageScheme | null, appDark: true, heard: true, fits: 0, chosen: [] as PageScheme[] };
  const toggle = installPageToggle(
    toolbar,
    () => state.page,
    () => state.appDark,
    () => state.heard,
    () => DEFAULT_LABELS,
    (scheme) => state.chosen.push(scheme),
    () => (state.fits += 1),
  );
  const button = doc.getElementById("page-background") as unknown as HTMLButtonElement;
  return { state, toggle, button, separator: doc.getElementById("page-background-sep")! };
}

describe("the page toggle", () => {
  test("is drawn only once a host has named a page, in a dark app, to a host that listens", () => {
    const { state, toggle, button, separator } = setup();
    toggle.update();
    expect(button.hidden).toBe(true);
    state.page = "dark";
    toggle.update();
    expect(button.hidden).toBe(false);
    expect(separator.hidden).toBe(false);
    expect(state.fits).toBe(1);
    for (const change of [() => (state.appDark = false), () => (state.heard = false)]) {
      state.appDark = true;
      state.heard = true;
      toggle.update();
      change();
      toggle.update();
      expect(button.hidden).toBe(true);
    }
  });

  test("names what pressing it does, and asks for the other page", () => {
    const { state, toggle, button } = setup();
    state.page = "dark";
    toggle.update();
    expect(button.getAttribute("aria-label")).toBe(DEFAULT_LABELS.lightBackground);
    expect(button.dataset.page).toBe("dark");
    button.click();
    state.page = "light";
    toggle.update();
    expect(button.getAttribute("aria-label")).toBe(DEFAULT_LABELS.darkBackground);
    button.click();
    expect(state.chosen).toEqual(["light", "dark"]);
  });
});
