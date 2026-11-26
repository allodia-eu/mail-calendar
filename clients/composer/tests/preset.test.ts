// The script in the document's head: a scheme and a page the host preset are drawn before any of
// the editor's markup is painted, because an engine may paint that markup while the editor's own
// script at the end of the body is still arriving.

import { describe, expect, test } from "bun:test";
import { Window } from "happy-dom";

import { applyPreset } from "../src/preset";

const built = await Bun.file(`${import.meta.dir}/../dist/editor.html`).text();

const DARK = {
  scheme: "dark",
  background: "#1e1e1e",
  text: "#e3e3e3",
  link: "#8ab4f8",
  quote_rule: "#5c5c5c",
  header_rule: "#4a5567",
  table_rule: "#3d4146",
};

function preset(values: { composerScheme?: unknown; composerPage?: unknown }) {
  const window = new Window();
  Object.assign(window, values);
  applyPreset(window as unknown as Window & typeof values & globalThis.Window);
  return window.document.documentElement as unknown as HTMLElement;
}

describe("the head's script", () => {
  test("draws the chrome's scheme and the page the host preset", () => {
    const root = preset({ composerScheme: "dark", composerPage: DARK });

    expect(root.dataset.scheme).toBe("dark");
    expect(root.dataset.page).toBe("dark");
    expect(root.style.getPropertyValue("--page")).toBe("#1e1e1e");
    expect(root.style.getPropertyValue("--text")).toBe("#e3e3e3");
  });

  test("takes the page as the preload hands it over, as JSON text too", () => {
    expect(preset({ composerPage: JSON.stringify(DARK) }).dataset.page).toBe("dark");
  });

  test("leaves a document no host preset as the stylesheet starts it", () => {
    const root = preset({});

    expect(root.dataset.scheme).toBeUndefined();
    expect(root.dataset.page).toBeUndefined();
    expect(root.getAttribute("style")).toBeNull();
  });

  test("draws no page from anything the core did not build", () => {
    for (const page of [{ ...DARK, background: "url(x)" }, { ...DARK, scheme: "dim" }, "{", null]) {
      expect(preset({ composerPage: page }).dataset.page).toBeUndefined();
    }
  });

  test("runs in the head, ahead of every piece of the editor's markup", () => {
    const head = built.slice(built.indexOf("<head>"), built.indexOf("</head>"));
    expect(head).toContain("applyPreset(window)");
    // Its own script, small, and not the editor's.
    expect(head).not.toContain("installPage");
    expect(built.indexOf("applyPreset(window)")).toBeLessThan(built.indexOf("<body>"));
  });
});
