// The inline formatting commands: bold/italic/underline, font size, text colour and highlight.
//
// These are the one place the editor still calls `document.execCommand`. Applying a mark to a
// selection means splitting text nodes at both ends and re-wrapping every partially covered
// element between them, and the engines already implement that correctly; hand-rolling it would be
// a large amount of subtle code to replace something that works. Everything the commands leave
// behind is normalised straight afterwards (`marks.ts`), so what varies per engine never reaches
// the document.
//
// The consequence for testing is that `applyMark`/`applyFontSize`/`applyColor` cannot run under
// happy-dom, which has no `execCommand`. What *can*: and what carries the behaviour worth pinning
//: is the normalisation and `clearColor` below.

import { documentOf, focusEditor, rangeTouches, rangeWithin } from "./dom";
import { normalizeColor, normalizeFontElements } from "./marks";
import { type FontSize, SIZE_PX } from "./types";

type Legacy = Document & {
  execCommand?: (command: string, showUi?: boolean, value?: string) => boolean;
};

function exec(editor: HTMLElement, command: string, value?: string): boolean {
  focusEditor(editor);
  const doc = documentOf(editor) as Legacy;
  return doc.execCommand?.(command, false, value) ?? false;
}

/// Asks the engine to express marks as inline CSS rather than legacy presentational elements, so a
/// colour arrives as `style="color:…"` on a `<span>`. Best-effort: `normalizeFontElements` cleans
/// up the `<font>` elements an engine that ignores this produces anyway.
function preferCss(editor: HTMLElement): void {
  exec(editor, "styleWithCSS", "true");
}

export function applyMark(editor: HTMLElement, command: "bold" | "italic" | "underline"): void {
  exec(editor, command);
}

export function applyFontSize(editor: HTMLElement, size: FontSize): void {
  // `7` is the largest legacy `<font size>` value, used purely as a marker: it makes the runs the
  // command just touched findable, and `normalizeFontElements` then rewrites them to the real size.
  exec(editor, "fontSize", "7");
  normalizeFontElements(editor, size);
  // The CSS path emits a `<span style="font-size:xx-large">` instead of a `<font>`; rewrite those
  // to the product's pixel size so the editor shows what will be sent.
  //
  // Scoped to what the selection covers, not to the whole editor: a document may already hold sized
  // spans carrying no `data-size`: a stored signature reopened in the Settings editor is exactly
  // that, since the core's sanitiser keeps `style` and drops `data-*`: and an unscoped sweep would
  // resize every one of them because the user picked a size for one word.
  const range = rangeWithin(editor);
  for (const span of Array.from(editor.querySelectorAll<HTMLElement>("span[style*='font-size']"))) {
    if (span.dataset.size || span.closest(".allodia-quote, .allodia-signature")) continue;
    if (!range || !rangeTouches(range, span)) continue;
    span.dataset.size = size;
    span.style.fontSize = `${SIZE_PX[size]}px`;
  }
}

export type ColorKind = "color" | "highlight";

/// Paints the selection. `null` clears the mark instead of painting a new one.
export function applyColor(editor: HTMLElement, kind: ColorKind, color: string | null): void {
  const normalized = normalizeColor(color);
  if (!normalized) {
    clearColor(editor, kind);
    return;
  }
  preferCss(editor);
  if (kind === "color") {
    exec(editor, "foreColor", normalized);
  } else if (!exec(editor, "hiliteColor", normalized)) {
    // Blink implements the highlight as `backColor` when `styleWithCSS` is off, and some engines
    // report `hiliteColor` unsupported outright.
    exec(editor, "backColor", normalized);
  }
  normalizeFontElements(editor, null);
  stampColor(editor, kind, normalized);
}

/// Stamps `data-color`/`data-highlight` on the spans the command just painted, so the mark is read
/// from an explicit marker rather than from a style attribute whose spelling varies per engine.
function stampColor(editor: HTMLElement, kind: ColorKind, color: string): void {
  const property = kind === "color" ? "color" : "background-color";
  for (const span of Array.from(
    editor.querySelectorAll<HTMLElement>(`span[style*='${property}']`),
  )) {
    if (span.closest(".allodia-quote, .allodia-signature")) continue;
    const own = normalizeColor(kind === "color" ? span.style.color : span.style.backgroundColor);
    if (own === color) span.dataset[kind === "color" ? "color" : "highlight"] = color;
  }
}

/// Removes a colour mark from the elements the selection covers: the palette's "Automatic" (text)
/// and "None" (highlight).
///
/// Scoped to elements the selection touches, and it clears the whole element even when only part of
/// it is selected: unpainting exactly half a coloured run would mean splitting it, which is the work
/// `execCommand` exists to do and which has no command for *removing* one mark. Selecting the run
/// and clearing it does the right thing, which is how the control is used.
export function clearColor(editor: HTMLElement, kind: ColorKind): void {
  const range = rangeWithin(editor);
  if (!range) return;
  const attribute = kind === "color" ? "data-color" : "data-highlight";
  const property = kind === "color" ? "color" : "backgroundColor";

  for (const element of Array.from(editor.querySelectorAll<HTMLElement>("*"))) {
    if (element.closest(".allodia-quote, .allodia-signature")) continue;
    if (!element.hasAttribute(attribute) && !element.style[property]) continue;
    if (!rangeTouches(range, element)) continue;
    element.removeAttribute(attribute);
    element.style[property] = "";
    unwrapIfBare(element);
  }
}

/// Removes a `<span>` that no longer carries any formatting, lifting its children into its place,
/// otherwise clearing a colour leaves a growing pile of meaningless wrappers in the sent HTML.
function unwrapIfBare(element: HTMLElement): void {
  if (element.tagName !== "SPAN") return;
  const meaningful = Array.from(element.attributes).some(
    (attribute) => attribute.name !== "style" || attribute.value.trim() !== "",
  );
  const parent = element.parentElement;
  if (meaningful || !parent) return;
  while (element.firstChild) parent.insertBefore(element.firstChild, element);
  element.remove();
}
