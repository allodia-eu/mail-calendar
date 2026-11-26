// The page's own colours on the document: the core's page read, and drawn through the custom
// properties the stylesheet's page is made of (`editor.css`). Shared by the page (`page.ts`) and by
// the script in the document's head (`preset.ts`), which draws a page the host handed over before
// any of the editor's markup is painted.

export type PageScheme = "light" | "dark";

/// `mailcal_composer::ComposerPage`.
export interface PageColours {
  background: string;
  text: string;
  link: string;
  quote_rule: string;
  header_rule: string;
  table_rule: string;
}

/// The custom properties the page's colours are drawn through (`editor.css`).
export const PROPERTIES: Record<keyof PageColours, string> = {
  background: "--page",
  text: "--text",
  link: "--page-link",
  quote_rule: "--quote-rule",
  header_rule: "--header-rule",
  table_rule: "--table-rule",
};

/// A colour the core writes: `#rrggbb`, `rgb(…)` or `rgba(…)`. Anything else is not put into a
/// stylesheet.
export const COLOUR = /^(#[0-9a-f]{6}|rgba?\([0-9., ]+\))$/;

/// A page the core built, or `null` when `value` is not one.
export function pageOf(value: unknown): { scheme: PageScheme; colours: PageColours } | null {
  let source = value;
  if (typeof source === "string") {
    try {
      source = JSON.parse(source);
    } catch {
      return null;
    }
  }
  const record = source as Record<string, unknown> | null;
  if (!record || (record.scheme !== "light" && record.scheme !== "dark")) return null;
  const colours = {} as PageColours;
  for (const field of Object.keys(PROPERTIES) as (keyof PageColours)[]) {
    const colour = record[field];
    if (typeof colour !== "string" || !COLOUR.test(colour)) return null;
    colours[field] = colour;
  }
  return { scheme: record.scheme, colours };
}

/// Draws `scheme`'s page on `root`.
export function paintPage(root: HTMLElement, scheme: PageScheme, colours: PageColours): void {
  root.dataset.page = scheme;
  for (const [field, property] of Object.entries(PROPERTIES)) {
    root.style.setProperty(property, colours[field as keyof PageColours]);
  }
}

/// Takes the page off `root`, back to the light one the stylesheet starts on.
export function unpaintPage(root: HTMLElement): void {
  delete root.dataset.page;
  for (const property of Object.values(PROPERTIES)) root.style.removeProperty(property);
}
