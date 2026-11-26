// The colours of a quoted original and a signature, shown as the reading view shows them.
//
// Both are markup the editor keeps as it arrived (`read_regions.ts`), sent back out verbatim, and
// their colours are wherever their sender put them: a `style`, a `bgcolor`, a `<font color>`, a
// `<style>` sheet, `!important` included. A stylesheet of the editor's own cannot outrank an
// `!important` inline declaration, so the page shows each of these moved in place: the value the
// core answered is set on the element, and the value as written is kept beside it, with the value
// shown, so it can be put back. The two are read only by `writtenHtml`, which is what every path
// that takes a region's markup out of the editor goes through.
//
// The value shown is kept so a change the engine made since can be told from the page's own: an
// `execCommand` that adds a declaration to a quoted element's `style` keeps that declaration, and
// only the colours the page moved go back to what was written.

import { windowOf } from "./dom";

/// Where the editor holds markup as written rather than as its own runs.
export const WRITTEN_MARKUP = ".allodia-quote .aq-body, .allodia-signature";

/// One piece of a region the core is asked about: `mailcal_composer::ColourPiece`.
export type RegionPiece =
  | { attribute: { element: string; attribute: string; value: string } }
  | { sheet: string };

/// What the core answered for a piece on the page in force: the value to show, `null` for the
/// value as written, `undefined` while it has not been asked.
export type Answered = (piece: RegionPiece) => string | null | undefined;

const writtenName = (name: string) => `data-written-${name}`;
const shownName = (name: string) => `data-shown-${name}`;

/// The colour-bearing attributes the reading view moves, on `element`. A `<style>` element's text
/// is the `sheet`.
function namesOf(element: Element): string[] {
  if (element.tagName === "STYLE") return ["sheet"];
  const names = ["style", "bgcolor"];
  if (element.tagName === "FONT") names.push("color");
  return names;
}

function current(element: Element, name: string): string | null {
  return name === "sheet" ? element.textContent : element.getAttribute(name);
}

function put(element: Element, name: string, value: string | null): void {
  if (current(element, name) === value) return;
  if (name === "sheet") element.textContent = value ?? "";
  else if (value === null) element.removeAttribute(name);
  else element.setAttribute(name, value);
}

/// `name` on `element` as it was written: what the page moved put back, and whatever else changed
/// since kept.
function asWritten(element: Element, name: string): string | null {
  const now = current(element, name);
  const written = element.getAttribute(writtenName(name));
  const shown = element.getAttribute(shownName(name));
  if (written === null || shown === null || now === null) return now;
  if (now === shown) return written;
  return name === "style" ? mergeStyle(element, now, shown, written) : now;
}

/// `now` with each declaration still as the page showed it put back as `written` has it.
function mergeStyle(element: Element, now: string, shown: string, written: string): string {
  const doc = element.ownerDocument;
  const declarations = (css: string) => {
    const scratch = doc.createElement("span");
    scratch.setAttribute("style", css);
    return (scratch as HTMLElement).style;
  };
  const result = declarations(now);
  const before = declarations(shown);
  const original = declarations(written);
  for (const property of Array.from(before)) {
    if (
      result.getPropertyValue(property) === before.getPropertyValue(property) &&
      result.getPropertyPriority(property) === before.getPropertyPriority(property)
    ) {
      result.setProperty(property, original.getPropertyValue(property), original.getPropertyPriority(property));
    }
  }
  return result.cssText;
}

function pieceOf(element: Element, name: string, value: string): RegionPiece {
  if (name === "sheet") return { sheet: value };
  return { attribute: { element: element.tagName.toLowerCase(), attribute: name, value } };
}

/// Every element the page moves: those inside a region, or inside `editor` itself when the whole
/// editor is one (the signature editor).
export function regionElements(editor: HTMLElement, whole: boolean): Element[] {
  if (whole) return Array.from(editor.querySelectorAll("*"));
  return Array.from(editor.querySelectorAll(`:is(${WRITTEN_MARKUP}) *`));
}

/// What showing the regions takes: the writes that bring each element to what `answered` says,
/// not yet made, and the pieces not asked about yet. An element waiting on an answer keeps what it
/// shows until one arrives.
export interface RegionPlan {
  writes: (() => void)[];
  missing: RegionPiece[];
}

export function planRegions(editor: HTMLElement, whole: boolean, answered: Answered): RegionPlan {
  const plan: RegionPlan = { writes: [], missing: [] };
  for (const element of regionElements(editor, whole)) {
    for (const name of namesOf(element)) {
      const written = asWritten(element, name);
      const kept = element.getAttribute(writtenName(name));
      const shown = element.getAttribute(shownName(name));
      if (written === null) {
        if (kept !== null || shown !== null) plan.writes.push(() => forget(element, name));
        continue;
      }
      const piece = pieceOf(element, name, written);
      const answer = answered(piece);
      if (answer === undefined) {
        plan.missing.push(piece);
        continue;
      }
      const target = answer ?? written;
      if (target === written) {
        if (current(element, name) !== written || kept !== null || shown !== null) {
          plan.writes.push(() => {
            put(element, name, written);
            forget(element, name);
          });
        }
      } else if (current(element, name) !== target || kept !== written || shown !== target) {
        plan.writes.push(() => {
          put(element, name, target);
          element.setAttribute(writtenName(name), written);
          element.setAttribute(shownName(name), target);
        });
      }
    }
  }
  return plan;
}

/// Every element showing a colour other than the one written.
const SHOWN = "[data-written-style], [data-written-bgcolor], [data-written-color], [data-written-sheet]";

/// Whether anything in `editor` shows a colour other than the one written.
export function showsRegions(editor: HTMLElement): boolean {
  return editor.querySelector(SHOWN) !== null;
}

/// Puts every element in `editor` back to what was written, page marks and all, for an edit to run
/// on: one an edit carried out of its region included.
export function restoreRegions(editor: HTMLElement): void {
  for (const element of Array.from(editor.querySelectorAll(SHOWN))) {
    for (const name of namesOf(element)) {
      if (!element.hasAttribute(writtenName(name))) continue;
      put(element, name, asWritten(element, name));
      forget(element, name);
    }
  }
}

function forget(element: Element, name: string): void {
  element.removeAttribute(writtenName(name));
  element.removeAttribute(shownName(name));
}

/// `container`'s markup with every colour as it was written: what a region sends, and what a
/// draft stores. The editor's own elements are not touched; a copy is.
export function writtenHtml(container: Element): string {
  if (!container.querySelector(SHOWN)) {
    return container.innerHTML;
  }
  const copy = container.cloneNode(true) as Element;
  for (const element of Array.from(copy.querySelectorAll("*"))) {
    for (const name of namesOf(element)) {
      if (element.hasAttribute(writtenName(name))) put(element, name, asWritten(element, name));
      forget(element, name);
    }
  }
  return copy.innerHTML;
}

/// Whether `records` could have changed what a region shows: a node added in or above one, or an
/// attribute it reads changed inside one.
export function touchesRegions(records: MutationRecord[], editor: HTMLElement, whole: boolean): boolean {
  if (whole) return records.some((record) => record.type !== "characterData");
  const Element = windowOf(editor).Element;
  return records.some((record) => {
    const target = record.target instanceof Element ? record.target : record.target.parentElement;
    if (target?.closest(WRITTEN_MARKUP)) return record.type !== "characterData" || target.tagName === "STYLE";
    if (record.type !== "childList") return false;
    return Array.from(record.addedNodes).some(
      (node) => node instanceof Element && (node.matches(WRITTEN_MARKUP) || node.querySelector(WRITTEN_MARKUP) !== null),
    );
  });
}
