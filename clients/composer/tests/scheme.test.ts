// The chrome's colour scheme: the host names the app's, and the stylesheet resolves against it.

import { describe, expect, test } from "bun:test";
import { Window } from "happy-dom";

import { setScheme } from "../src/scheme";

const stylesheet = await Bun.file(`${import.meta.dir}/../src/editor.css`).text();

function root(): HTMLElement {
  const window = new Window();
  return window.document.documentElement as unknown as HTMLElement;
}

describe("setComposerScheme", () => {
  test("names the scheme on the root", () => {
    const html = root();
    setScheme(html, "dark");
    expect(html.dataset.scheme).toBe("dark");
    setScheme(html, "light");
    expect(html.dataset.scheme).toBe("light");
  });

  test("hands anything else back to the system's scheme", () => {
    const html = root();
    setScheme(html, "dark");
    for (const value of ["", "Dark", "system", null, undefined, 1, {}]) {
      setScheme(html, "dark");
      setScheme(html, value);
      expect(html.dataset.scheme).toBeUndefined();
    }
  });
});

describe("the stylesheet", () => {
  // The root's block: everything up to its first closing brace.
  const tokens = stylesheet.slice(stylesheet.indexOf(":root {"), stylesheet.indexOf("}", stylesheet.indexOf(":root {")));

  test("lets a named scheme decide over the system's", () => {
    expect(stylesheet).toMatch(/:root\[data-scheme="light"\]\s*\{\s*color-scheme:\s*light;/);
    expect(stylesheet).toMatch(/:root\[data-scheme="dark"\]\s*\{\s*color-scheme:\s*dark;/);
  });

  // A chrome colour written as a plain value would stay the same in both schemes, which is the
  // toolbar drawn light in a dark app that this exists to prevent.
  test("gives every chrome colour a light and a dark value", () => {
    const chrome = [...tokens.matchAll(/^\s*(--(?:chrome|field|popover|swatch|accent)[\w-]*):\s*(.+);$/gm)];
    expect(chrome.length).toBeGreaterThan(10);
    for (const [, name, value] of chrome) {
      if (name === "--accent-text") continue;
      expect({ name, value }).toEqual({ name, value: expect.stringMatching(/^light-dark\(/) });
    }
  });

  // The page is the message as sent, so it must not move with the app.
  test("keeps the page light", () => {
    expect(stylesheet).toMatch(/\.editor\s*\{[^}]*color-scheme:\s*light;/);
    for (const name of ["--page", "--text", "--muted", "--border"]) {
      expect(tokens).toMatch(new RegExp(`${name}:\\s*#[0-9a-f]{6};`));
    }
  });
});
