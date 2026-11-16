// Framed mode: the editor as a desktop composer column holds it (`docs/composer-layout.md`, rules 5
// to 7). The host gives the page the height of the column's visible part and scrolls its header
// away first; the message then scrolls inside its own box, under a toolbar the page never moves.
// So the toolbar stays where it is drawn in the same frame as the text, which nothing the host
// could tell the page as it scrolls would manage.
//
// The page reports two things on the host channel. Where the caret is, in the page's own
// coordinates, so a host whose column still shows part of its header can bring a line typed under
// the column's edge into view; and how far the message is scrolled and where its text ends, so a
// host can hand a gesture back to its column once the message is at its top, and say when there is
// more of the message below what is shown.
//
// A host whose web view scrolls the page by every wheel step it is given, before the host sees the
// step, holds the steps down while its column still has its header to carry away
// (`holdWheelDown`), so the column and the message do not both move by one.

import { windowOf } from "./dom";

/// Where the reports go: the host channel's `caret` and `scroll` notices.
export interface FramedReports {
  caret(top: number, bottom: number): void;
  scroll(px: number, end: number): void;
}

/// What framed mode measures, in CSS pixels from the top of the page.
export interface FramedMeasures {
  /// The caret's top and bottom, or `null` when the selection is not in the message.
  caret(): { top: number; bottom: number } | null;
  /// How far the message is scrolled inside its box.
  scrollTop(): number;
  /// The bottom of the message's last line, as it is scrolled now.
  end(): number;
}

export interface Framed {
  useFramedComposer(): void;
  /// Whether a wheel step down over the page is the host column's rather than the message's.
  holdWheelDown(hold: boolean): void;
  /// Sends both reports again, for a host that has just announced it takes them.
  resend(): void;
}

export function installFramed(
  editor: HTMLElement,
  reports: FramedReports,
  measures: FramedMeasures = domMeasures(editor),
): Framed {
  const doc = editor.ownerDocument;
  const win = windowOf(editor);
  let on = false;
  let lastCaret = "";
  let lastScroll = "";
  let wheelDownHeld = false;
  let caretScheduled = false;
  let scrollScheduled = false;

  const reportCaret = (force: boolean) => {
    const caret = measures.caret();
    if (!caret) return;
    const top = Math.max(0, Math.floor(caret.top));
    const bottom = Math.max(top, Math.ceil(caret.bottom));
    const key = `${top}:${bottom}`;
    if (!force && key === lastCaret) return;
    lastCaret = key;
    reports.caret(top, bottom);
  };

  const reportScroll = (force: boolean) => {
    const px = Math.max(0, Math.round(measures.scrollTop()));
    const end = Math.max(0, Math.ceil(measures.end()));
    const key = `${px}:${end}`;
    if (!force && key === lastScroll) return;
    lastScroll = key;
    reports.scroll(px, end);
  };

  // A burst of mutations, a paste or a picture loading, is measured once, after layout. The caret
  // is reported when it or the text moves, never when the message is merely scrolled: a host that
  // followed it then would pull the column back to a caret the reader scrolled away from. A change
  // to the text reports it even on the same line, because the host's column may have moved since
  // the last report and left that line under its edge.
  let textChanged = false;
  const scheduleCaret = (edited: boolean) => {
    // A change to the text moves where it ends, too.
    scheduleScroll();
    if (!on) return;
    textChanged ||= edited;
    if (caretScheduled) return;
    caretScheduled = true;
    win.requestAnimationFrame(() => {
      caretScheduled = false;
      const force = textChanged;
      textChanged = false;
      reportCaret(force);
    });
  };

  const scheduleScroll = () => {
    if (!on || scrollScheduled) return;
    scrollScheduled = true;
    win.requestAnimationFrame(() => {
      scrollScheduled = false;
      reportScroll(false);
    });
  };

  return {
    useFramedComposer() {
      if (on) return;
      on = true;
      doc.documentElement.classList.add("framed");
      doc.body.classList.add("framed");
      new win.MutationObserver(() => scheduleCaret(true)).observe(editor, {
        childList: true,
        subtree: true,
        characterData: true,
        attributes: true,
      });
      doc.addEventListener("selectionchange", () => scheduleCaret(false));
      editor.addEventListener("scroll", scheduleScroll);
      // A narrower page wraps the text onto more lines, which no mutation reports.
      if ("ResizeObserver" in win) new win.ResizeObserver(scheduleScroll).observe(editor);
      doc.addEventListener(
        "wheel",
        (event) => {
          if (wheelDownHeld && event.deltaY > 0) event.preventDefault();
        },
        { passive: false },
      );
      scheduleScroll();
    },
    holdWheelDown(hold) {
      wheelDownHeld = hold;
    },
    resend() {
      if (!on) return;
      reportCaret(true);
      reportScroll(true);
    },
  };
}

/// The measurements, from the laid-out page. The page itself never scrolls in this mode, so a
/// rectangle's place in the viewport is its place in the page.
function domMeasures(editor: HTMLElement): FramedMeasures {
  const doc = editor.ownerDocument;
  const win = windowOf(editor);
  return {
    scrollTop: () => editor.scrollTop,
    end() {
      // The last line's bottom, not the box's: the editor's own padding is not message.
      const last = editor.lastElementChild;
      const rect = (last ?? editor).getBoundingClientRect();
      return last ? rect.bottom : rect.top;
    },
    caret() {
      const selection = win.getSelection();
      if (!selection || selection.rangeCount === 0 || !doc.hasFocus()) return null;
      const range = selection.getRangeAt(0);
      if (!editor.contains(range.endContainer)) return null;
      const end = range.cloneRange();
      end.collapse(false);
      const box = editor.getBoundingClientRect();
      const rect = end.getBoundingClientRect();
      if (rect.height > 0) return shownIn({ top: rect.top, bottom: rect.bottom }, box);
      // A caret on an empty line has no rect of its own; the block it sits in has. When the caret
      // is between two of the message's blocks, the block is the one at its offset, or the one
      // before it at the end. A block can be the whole message (an empty one is just the editor),
      // so only the line the caret is on counts: a caret as tall as the window is brought to the
      // top, which carries the host's header away.
      const node = end.endContainer;
      const after = node.nodeType === 1 ? node.childNodes[end.endOffset] : undefined;
      const before = node.nodeType === 1 ? node.childNodes[end.endOffset - 1] : undefined;
      const element = node.nodeType === 1 ? (after ?? before ?? node) : node.parentElement;
      if (!element || element.nodeType !== 1) return null;
      const block = element as Element;
      const line = lineIn(block.getBoundingClientRect(), win.getComputedStyle(block), !after && !!before);
      return shownIn(line, box);
    },
  };
}

/// The caret, or `null` when the message's own scroll has taken it out of its box: the box clips
/// it, so no scroll of the host's column can show it. A select-all's end far down a long message,
/// or a caret the reader scrolled away from, would otherwise pull the column to it.
export function shownIn(
  caret: { top: number; bottom: number },
  box: { top: number; bottom: number },
): { top: number; bottom: number } | null {
  return caret.bottom <= box.top || caret.top >= box.bottom ? null : caret;
}

/// The first line of a block, or its last when `last`: where a caret that has no rectangle of its
/// own is drawn. A line's height is its `line-height`, or the font's size and a fifth when that is
/// `normal`; it never runs past the block's edge.
export function lineIn(
  rect: { top: number; bottom: number },
  style: Pick<CSSStyleDeclaration, "paddingTop" | "paddingBottom" | "lineHeight" | "fontSize">,
  last: boolean,
): { top: number; bottom: number } {
  const px = (value: string) => {
    const parsed = parseFloat(value);
    return Number.isFinite(parsed) ? parsed : 0;
  };
  const line = px(style.lineHeight) || px(style.fontSize) * 1.2;
  if (last) {
    const bottom = rect.bottom - px(style.paddingBottom);
    return { top: Math.max(rect.top, bottom - line), bottom };
  }
  const top = rect.top + px(style.paddingTop);
  return { top, bottom: Math.min(rect.bottom, top + line) };
}
