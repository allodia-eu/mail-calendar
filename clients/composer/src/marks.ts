// Reading the inline marks (bold/italic/underline/size/typeface/colour) off the DOM, and normalising
// the markup `execCommand` leaves behind so those marks are readable at all.

import { safeLinkHref } from "./links";
import { canonicalFamily, familyCss, sizeOfCss, sizeOfLegacy } from "./typography";
import { type FontSize, type HexColor, type Marks, fontSizeOf } from "./types";

/// Normalises any colour the DOM might hand back into the `#rrggbb` Rust accepts, or `null` when
/// it is not a colour we can represent.
///
/// Engines disagree about what `element.style.color` reads back as: a hex literal in one, an
/// `rgb(…)` triple in another: and `execCommand` adds a third spelling via `<font color>`. Rust
/// validates strictly and rejects the document on submit, so the conversion happens here, where a
/// value we do not understand can be dropped harmlessly (the run renders in the inherited colour)
/// instead of failing the send.
///
/// A fully transparent colour is `null`, not `#000000`: that is how "no highlight" arrives, and
/// treating it as black would paint every un-highlighted run.
export function normalizeColor(value: string | null | undefined): HexColor | null {
  const raw = (value ?? "").trim().toLowerCase();
  if (!raw || raw === "transparent" || raw === "initial" || raw === "inherit") return null;
  const named = NAMED_COLOURS[raw];
  if (named) return named;

  const hex = /^#([0-9a-f]{3}|[0-9a-f]{6})$/.exec(raw);
  if (hex) {
    const digits = hex[1]!;
    return digits.length === 3
      ? `#${digits[0]}${digits[0]}${digits[1]}${digits[1]}${digits[2]}${digits[2]}`
      : `#${digits}`;
  }

  const rgb = /^rgba?\(([^)]+)\)$/.exec(raw);
  if (rgb) {
    const parts = rgb[1]!.split(/[\s,/]+/).filter(Boolean);
    if (parts.length < 3) return null;
    // A zero alpha is "no colour": see the note above.
    if (parts.length > 3 && Number.parseFloat(parts[3]!) === 0) return null;
    const channels = parts.slice(0, 3).map((part) => {
      const n = part.endsWith("%")
        ? Math.round((Number.parseFloat(part) / 100) * 255)
        : Number.parseInt(part, 10);
      return Number.isFinite(n) ? Math.min(255, Math.max(0, n)) : null;
    });
    if (channels.some((c) => c === null)) return null;
    return `#${channels.map((c) => c!.toString(16).padStart(2, "0")).join("")}`;
  }

  return null;
}

/// The colours CSS 2.1 names, which is the set older editors write into `<font color>`.
const NAMED_COLOURS: Record<string, HexColor> = {
  black: "#000000",
  silver: "#c0c0c0",
  gray: "#808080",
  grey: "#808080",
  white: "#ffffff",
  maroon: "#800000",
  red: "#ff0000",
  purple: "#800080",
  fuchsia: "#ff00ff",
  green: "#008000",
  lime: "#00ff00",
  olive: "#808000",
  yellow: "#ffff00",
  navy: "#000080",
  blue: "#0000ff",
  teal: "#008080",
  aqua: "#00ffff",
  orange: "#ffa500",
};

/// The marks in force on `element`, given those inherited from its ancestors.
///
/// Every mark is read from the element's OWN markers: its tag, the `data-*` stamps the toolbar
/// writes, its inline `style`, and a `<font>`'s attributes; never from `getComputedStyle`. A
/// computed value is inherited by everything inside the element that set it, so reading one would
/// stamp a colour nobody chose on every run, and would read every word of a heading as bold: a
/// heading is a line's style (`lines.ts`), not a mark on its words. Underline is the exception,
/// because `text-decoration` is not inherited and an engine may express it no other way.
///
/// A heading that sits where no line can be one, inside a list item or a table cell, is read as
/// the bold, larger run it looks like, since the document has no other way to hold it there.
export function elementMarks(element: Element, inherited: Marks): Marks {
  const tag = element.tagName;
  const html = element as HTMLElement;
  const style = html.style;
  const computed = element.ownerDocument.defaultView?.getComputedStyle(element);
  const fontWeight = style?.fontWeight || "";
  const weight = fontWeight.startsWith("bold") ? 700 : Number.parseInt(fontWeight, 10);
  const decoration = [
    computed?.textDecorationLine,
    computed?.textDecoration,
    style?.textDecorationLine,
    style?.textDecoration,
  ].join(" ");
  const inlineHeading = tag in HEADING_SIZES && !!element.parentElement?.closest("li, td, th");

  const color = normalizeColor(
    html.dataset.color ?? (style?.color || (tag === "FONT" ? element.getAttribute("color") : null)),
  );
  const highlight = normalizeColor(html.dataset.highlight ?? style?.backgroundColor);

  return {
    bold: inherited.bold || tag === "B" || tag === "STRONG" || inlineHeading || weight >= 600,
    italic: inherited.italic || tag === "I" || tag === "EM" || style?.fontStyle === "italic",
    // A link's underline is how the editor draws a link, not a mark the user chose: read from
    // the computed style it would send every link out underlined twice.
    underline:
      inherited.underline || tag === "U" || (tag !== "A" && decoration.includes("underline")),
    size: sizeOf(element) ?? (inlineHeading ? HEADING_SIZES[tag]! : null) ?? inherited.size,
    font: fontOf(element) ?? inherited.font,
    color: color ?? inherited.color,
    highlight: highlight ?? inherited.highlight,
    link: tag === "A" ? safeLinkHref(element.getAttribute("href")) : inherited.link,
  };
}

/// The run size a heading inside a list item or a table cell reads as. `H3` to `H6` are bold at
/// the body's size, which is what they look like beside the two larger ones.
const HEADING_SIZES: Record<string, FontSize | null> = {
  H1: 24,
  H2: 18,
  H3: null,
  H4: null,
  H5: null,
  H6: null,
};

/// The size `element` itself sets: the toolbar's stamp, its `font-size`, or a `<font size>`.
function sizeOf(element: Element): FontSize | null {
  const stamped = Number((element as HTMLElement).dataset.size);
  if (stamped > 0) return fontSizeOf(stamped);
  const css = sizeOfCss((element as HTMLElement).style?.fontSize);
  if (css !== null) return css;
  return element.tagName === "FONT" ? sizeOfLegacy(element.getAttribute("size")) : null;
}

/// The typeface `element` itself sets: the toolbar's stamp, its `font-family`, or a `<font face>`.
function fontOf(element: Element): string | null {
  const html = element as HTMLElement;
  return (
    canonicalFamily(html.dataset.font) ??
    canonicalFamily(html.style?.fontFamily) ??
    (element.tagName === "FONT" ? canonicalFamily(element.getAttribute("face")) : null)
  );
}

/// Rewrites the `<font>` elements `execCommand` leaves behind into marked-up `<span>`s.
///
/// `document.execCommand` is the one legacy the editor still leans on (bold/italic/underline and
/// the size/colour commands split a selection in ways that are genuinely hard to hand-roll), and
/// what it emits differs per engine: `<font size="7">` for a size, and either `<font color>` or an
/// inline `style` for a colour depending on `styleWithCSS`. Folding all of it into one shape here
/// means `elementMarks` has a single thing to read, and the outgoing document does not depend on
/// which WebView the user happens to be running.
///
/// `.allodia-quote` and `.allodia-signature` are left alone: both round-trip to Rust as verbatim
/// HTML, so rewriting inside them would edit content the user never touched.
export function normalizeFontElements(
  editor: HTMLElement,
  pendingSize: FontSize | null,
  pendingFont: string | null = null,
): void {
  for (const node of Array.from(editor.querySelectorAll("font"))) {
    if (node.closest(".allodia-quote, .allodia-signature")) continue;

    const span = editor.ownerDocument.createElement("span");
    // Carry the element's own inline style across before anything below overrides it: an engine
    // that does not implement `hiliteColor` paints a highlight as `<font style="background-color:…">`,
    // and dropping the attribute would lose the mark the user just applied.
    const inline = node.getAttribute("style");
    if (inline) span.setAttribute("style", inline);
    // execCommand("fontSize", "7") is how a size is applied: 7 is the largest legacy value, used
    // as a marker to find the runs the command just touched. The real size is `pendingSize`.
    if (node.getAttribute("size") === "7" && pendingSize) {
      stampSize(span, pendingSize);
    }
    // execCommand("fontName") leaves a `<font face>` naming what it was given, which is
    // `pendingFont` in the CSS spelling; the stamp holds the canonical one.
    const face = pendingFont ?? canonicalFamily(node.getAttribute("face"));
    if (node.hasAttribute("face") && face) stampFont(span, face);
    const color = normalizeColor(node.getAttribute("color"));
    if (color) {
      span.dataset.color = color;
      span.style.color = color;
    }
    while (node.firstChild) span.appendChild(node.firstChild);
    node.replaceWith(span);
  }
}

/// Marks `span` as `size` pixels, the shape the reader and the toolbar both know.
export function stampSize(span: HTMLElement, size: FontSize): void {
  span.dataset.size = String(size);
  span.style.fontSize = `${size}px`;
}

/// Marks `span` as set in the canonical family list `font`.
export function stampFont(span: HTMLElement, font: string): void {
  span.dataset.font = font;
  span.style.fontFamily = familyCss(font);
}
