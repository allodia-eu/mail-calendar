// The page the message is written on (`docs/reading-dark-mode.md`, "Writing in a dark app").
//
// The message is shown as the reading view will show it once it is sent: on the reading view's
// canvas, with every colour moved by the same transform. The document keeps the colours it was
// written with, since it is what is sent, so nothing here changes what the editor hands out; it
// changes what is drawn. The host names the page (`setComposerPage`, built by the core), and the
// core answers how each colour in the message looks on it, over the `colours` request. Nothing
// here decides a colour.
//
// The editor's own runs carry their colour as a stamp (`data-color`, `data-highlight`), so they
// are shown moved by a stylesheet keyed on the stamp, and their markup is never touched. A quoted
// original and a signature are moved in place (`page_regions.ts`).
//
// A host that never names a page keeps the light page the stylesheet starts on, and is never asked.

import { windowOf } from "./dom";
import { COLOUR, type PageColours, type PageScheme, pageOf, paintPage, unpaintPage } from "./page_paint";
import { planRegions, type RegionPiece, restoreRegions, showsRegions, touchesRegions } from "./page_regions";

export type { PageScheme } from "./page_paint";

/// `mailcal_composer::ColourPiece`.
export type ColourPiece = { ink: string } | { fill: string } | RegionPiece;

/// The core's answer to one request: one entry per piece, in its order.
export type AskColours = (scheme: PageScheme, pieces: ColourPiece[]) => Promise<(string | null)[] | null>;

/// What a request may weigh, in bytes of JSON: `MAX_COLOURS_REQUEST_LEN` less room for the
/// envelope. A piece heavier than this on its own is shown as written.
export const MAX_BATCH_BYTES = 1000 * 1024;

/// How long an answer is waited for before the pieces are asked about again on the next change. A
/// host that cannot parse a request sends nothing back.
export const ANSWER_MS = 5000;

const STAMP = /^#[0-9a-f]{6}$/;

export interface Page {
  /// Puts the message on the page the host names. Anything that is not one keeps the page as it is.
  set(page: unknown): void;
  /// The page in force, or `null` while no host has named one.
  scheme(): PageScheme | null;
  /// Whether the whole editor is markup as written (the signature editor).
  setWhole(whole: boolean): void;
  /// Runs an engine edit on the colours as written (`asWritten`).
  asWritten<T>(edit: () => T): T;
}

/// `quietly` runs a change of the page's own without it reading as an edit: not a step of the
/// history, not a revision.
export function installPage(
  editor: HTMLElement,
  ask: AskColours,
  quietly: (change: () => void) => void,
  onScheme: (scheme: PageScheme) => void = () => {},
): Page {
  const doc = editor.ownerDocument;
  const root = doc.documentElement;
  const sheet = doc.createElement("style");
  sheet.id = "page-colours";
  doc.head.appendChild(sheet);

  let scheme: PageScheme | null = null;
  let colours: PageColours | null = null;
  let whole = false;
  /// The core's answers, by page and piece.
  const answers = new Map<string, string | null>();
  const asked = new Set<string>();
  const keyOf = (page: PageScheme, piece: ColourPiece) => `${page}\u0000${JSON.stringify(piece)}`;
  const answered = (piece: ColourPiece) => (scheme ? answers.get(keyOf(scheme, piece)) : null);

  const win = windowOf(editor);
  const encoder = new win.TextEncoder();

  /// Asks for `sent` on `page`. An answer that does not come, or does not fit, leaves the pieces
  /// unasked, so the next change asks again rather than the page waiting for good.
  const send = (page: PageScheme, sent: ColourPiece[]) => {
    const late = new Promise<null>((resolve) => win.setTimeout(() => resolve(null), ANSWER_MS));
    void Promise.race([ask(page, sent), late]).then((reply) => {
      if (!reply || reply.length !== sent.length) {
        for (const piece of sent) asked.delete(keyOf(page, piece));
        return;
      }
      sent.forEach((piece, at) => {
        const value = reply[at];
        answers.set(keyOf(page, piece), typeof value === "string" ? value : null);
      });
      if (scheme === page) show();
    });
  };

  /// The pieces waiting to be asked about, sent together once the change that found them is over:
  /// a drag that resizes a picture finds one per move.
  let queued: ColourPiece[] = [];
  let flushing = false;
  const flush = () => {
    flushing = false;
    if (!scheme) return;
    const page = scheme;
    let batch: ColourPiece[] = [];
    let bytes = 0;
    for (const piece of queued) {
      const key = keyOf(page, piece);
      if (asked.has(key)) continue;
      // What the core measures: UTF-8 bytes.
      const weight = encoder.encode(key).length;
      if (weight > MAX_BATCH_BYTES) continue;
      asked.add(key);
      if (bytes + weight > MAX_BATCH_BYTES) {
        send(page, batch);
        batch = [];
        bytes = 0;
      }
      batch.push(piece);
      bytes += weight;
    }
    queued = [];
    if (batch.length > 0) send(page, batch);
  };
  const request = (pieces: ColourPiece[]) => {
    queued.push(...pieces);
    if (flushing) return;
    flushing = true;
    win.setTimeout(flush, 0);
  };

  /// The rules that show each stamped run moved; the runs themselves are never touched.
  const runRules = (missing: ColourPiece[]): string => {
    const rules: string[] = [];
    const stamped = (attribute: string, property: string, piece: (value: string) => ColourPiece) => {
      const values = new Set<string>();
      for (const element of Array.from(editor.querySelectorAll(`[${attribute}]`))) {
        const value = element.getAttribute(attribute) ?? "";
        if (STAMP.test(value)) values.add(value);
      }
      for (const value of values) {
        const answer = answered(piece(value));
        if (answer === undefined) missing.push(piece(value));
        else if (answer !== null && COLOUR.test(answer)) {
          rules.push(`.editor [${attribute}="${value}"]{${property}:${answer}!important}`);
        }
      }
    };
    stamped("data-color", "color", (value) => ({ ink: value }));
    stamped("data-highlight", "background-color", (value) => ({ fill: value }));
    return rules.join("\n");
  };

  /// Shows the runs, and the regions too when `regions` says they may have changed.
  const show = (regions = true) => {
    if (!scheme || written > 0) return;
    const missing: ColourPiece[] = [];
    const rules = runRules(missing);
    if (sheet.textContent !== rules) sheet.textContent = rules;
    if (regions) {
      const plan = planRegions(editor, whole, (piece) => answered(piece));
      missing.push(...plan.missing);
      if (plan.writes.length > 0) {
        quietly(() => {
          for (const write of plan.writes) write();
        });
        // The page's own changes, which need no second look.
        observer.takeRecords();
      }
    }
    if (missing.length > 0) request(missing);
  };

  // A run stamped, a quote seeded, a signature swapped, an undo that put a moved element back.
  const Observer = windowOf(editor).MutationObserver;
  const observer = new Observer((records) => {
    if (!scheme) return;
    const regions = touchesRegions(records, editor, whole);
    const runs = records.some(
      (record) => record.type === "childList" || record.attributeName?.startsWith("data-") === true,
    );
    if (regions || runs) show(regions);
  });

  const paint = () => {
    if (!scheme || !colours) return;
    paintPage(root, scheme, colours);
  };

  // An edit runs on the colours as written. An engine writes the computed style of what it works
  // on into new markup: the content a delete across two paragraphs moves, the typing style Enter
  // carries to the next line, the selection a copy puts on the clipboard. On a dark page that is
  // the colour shown, not the one written, and it would be sent. So for the length of such an edit
  // the page is the light one the stylesheet starts on, every moved colour is put back, and the
  // page returns before anything is painted.
  let written = 0;
  const shows = () => scheme === "dark" || showsRegions(editor);
  const enter = () => {
    written += 1;
    if (written > 1) return;
    if (showsRegions(editor)) {
      quietly(() => restoreRegions(editor));
      observer.takeRecords();
    }
    if (scheme !== "dark") return;
    sheet.textContent = "";
    unpaintPage(root);
  };
  const leave = () => {
    if (written === 0) return;
    written -= 1;
    if (written > 0) return;
    paint();
    show();
  };
  const asWritten = <T>(edit: () => T): T => {
    enter();
    try {
      return edit();
    } finally {
      leave();
    }
  };

  // Hooked into the editor once a page is named, never before: until then every hook answers
  // nothing, and on iOS WebKit holding them delays the key after a drafted reply past what an undo
  // waits for.
  let watching = false;
  const watch = () => {
    if (watching) return;
    watching = true;
    observer.observe(editor, {
      childList: true,
      subtree: true,
      characterData: true,
      attributes: true,
      attributeFilter: ["style", "bgcolor", "color", "data-color", "data-highlight"],
    });
    // The engine's own edits: a key, a cut, a drop, a spelling correction. A character typed or
    // deleted inside one text node carries no style anywhere and is left alone, so typing in a
    // long quote costs no pass over it.
    editor.addEventListener("beforeinput", (event) => {
      if (!shows() || event.defaultPrevented || !carriesStyle(event, win)) return;
      enter();
      let left = false;
      const done = () => {
        if (left) return;
        left = true;
        leave();
      };
      editor.addEventListener("input", done, { once: true });
      // A `beforeinput` someone cancelled has no `input` after it.
      win.setTimeout(done, 0);
    });
    // A copy or a cut serialises the selection after its event and before any other: the page is
    // back once the task that copied is over.
    for (const type of ["copy", "cut"]) {
      editor.addEventListener(type, () => {
        if (!shows()) return;
        enter();
        win.setTimeout(leave, 0);
      });
    }
  };

  const page: Page = {
    set(value) {
      const named = pageOf(value);
      if (!named) return;
      watch();
      colours = named.colours;
      const changed = scheme !== named.scheme;
      scheme = named.scheme;
      if (written === 0) paint();
      show();
      if (changed) onScheme(named.scheme);
    },
    scheme: () => scheme,
    setWhole(value) {
      whole = value;
      show();
    },
    asWritten,
  };
  pages.set(doc, page);
  return page;
}

/// Each document's page, for the commands that run an engine edit (`asWritten`).
const pages = new WeakMap<Document, Page>();

/// Runs `edit`, an engine command on `doc`'s editor, on the colours as written (`installPage`). A
/// document with no page runs it as it is.
export function asWritten<T>(doc: Document, edit: () => T): T {
  const page = pages.get(doc);
  return page ? page.asWritten(edit) : edit();
}

/// Whether an engine edit can write a computed style into the message. Every edit can (Enter
/// carries the caret's style to the new line as a typing style) except a character typed at a
/// caret, one deleted inside the text node the caret is in, and an input method's composition,
/// which is never interrupted.
function carriesStyle(event: InputEvent, win: Window): boolean {
  const type = event.inputType;
  if (type === "insertCompositionText") return false;
  const selection = win.getSelection();
  if (!selection || selection.rangeCount === 0 || !selection.isCollapsed) return true;
  if (type === "insertText") return false;
  const node = selection.anchorNode;
  const offset = selection.anchorOffset;
  if (node?.nodeType !== 3) return true;
  if (type === "deleteContentBackward") return offset === 0;
  if (type === "deleteContentForward") return offset >= (node.nodeValue ?? "").length;
  return true;
}
