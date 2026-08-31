// Reading the inline marks (bold/italic/underline/size/colour) off the DOM, and normalising the
// markup `execCommand` leaves behind so those marks are readable at all.

import { type FontSize, type HexColor, type Marks, isFontSize, SIZE_PX } from "./types";

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

/// The marks in force on `element`, given those inherited from its ancestors.
///
/// Size and colour are read from the element's OWN markers: the `data-*` stamps the toolbar
/// writes, else its inline `style` attribute: never from `getComputedStyle`. Computed colour is
/// always set (every element inherits one), so reading it would stamp an explicit colour on every
/// run in every message and bloat the document with a colour nobody chose.
export function elementMarks(element: Element, inherited: Marks): Marks {
  const tag = element.tagName;
  const style = (element as HTMLElement).style;
  const computed = element.ownerDocument.defaultView?.getComputedStyle(element);
  const weight = Number.parseInt(computed?.fontWeight ?? "", 10);
  const decoration = `${computed?.textDecorationLine ?? ""} ${computed?.textDecoration ?? ""}`;

  const size = (element as HTMLElement).dataset.size;
  const color = normalizeColor((element as HTMLElement).dataset.color ?? style?.color);
  const highlight = normalizeColor(
    (element as HTMLElement).dataset.highlight ?? style?.backgroundColor,
  );

  return {
    bold: inherited.bold || tag === "B" || tag === "STRONG" || weight >= 600,
    italic: inherited.italic || tag === "I" || tag === "EM",
    underline: inherited.underline || tag === "U" || decoration.includes("underline"),
    size: isFontSize(size) ? size : inherited.size,
    color: color ?? inherited.color,
    highlight: highlight ?? inherited.highlight,
  };
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
export function normalizeFontElements(editor: HTMLElement, pendingSize: FontSize | null): void {
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
      span.dataset.size = pendingSize;
      span.style.fontSize = `${SIZE_PX[pendingSize]}px`;
    }
    const color = normalizeColor(node.getAttribute("color"));
    if (color) {
      span.dataset.color = color;
      span.style.color = color;
    }
    while (node.firstChild) span.appendChild(node.firstChild);
    node.replaceWith(span);
  }
}
