// Framed mode: the editor as a desktop composer column holds it (`docs/composer-layout.md`, rules 5
// to 7). The host gives the page the height of the column's visible part and scrolls its header
// away first; the message then scrolls inside its own box, under a toolbar the page never moves.
// So the toolbar stays where it is drawn in the same frame as the text, which nothing the host
// could tell the page as it scrolls would manage.
//
// The page reports two things on the host channel. Where the caret is, in the page's own
// coordinates, so a host whose column still shows part of its header can bring a line typed under
// the column's edge into view; and how far the message is scrolled, so a host can hand a gesture
// back to its column once the message is at its top.

import { windowOf } from "./dom";

/// Where the reports go: the host channel's `caret` and `scroll` notices.
export interface FramedReports {
  caret(top: number, bottom: number): void;
  scroll(px: number): void;
}

/// What framed mode measures, in CSS pixels from the top of the page.
export interface FramedMeasures {
  /// The caret's top and bottom, or `null` when the selection is not in the message.
  caret(): { top: number; bottom: number } | null;
  /// How far the message is scrolled inside its box.
  scrollTop(): number;
}

export interface Framed {
  useFramedComposer(): void;
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
  let lastScroll = -1;
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
    if (!force && px === lastScroll) return;
    lastScroll = px;
    reports.scroll(px);
  };

  // A burst of mutations, a paste or a picture loading, is measured once, after layout. The caret
  // is reported when it or the text moves, never when the message is merely scrolled: a host that
  // followed it then would pull the column back to a caret the reader scrolled away from.
  const scheduleCaret = () => {
    if (!on || caretScheduled) return;
    caretScheduled = true;
    win.requestAnimationFrame(() => {
      caretScheduled = false;
      reportCaret(false);
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
      new win.MutationObserver(scheduleCaret).observe(editor, {
        childList: true,
        subtree: true,
        characterData: true,
        attributes: true,
      });
      doc.addEventListener("selectionchange", scheduleCaret);
      editor.addEventListener("scroll", scheduleScroll);
      scheduleScroll();
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
    caret() {
      const selection = win.getSelection();
      if (!selection || selection.rangeCount === 0 || !doc.hasFocus()) return null;
      const range = selection.getRangeAt(0);
      if (!editor.contains(range.endContainer)) return null;
      const end = range.cloneRange();
      end.collapse(false);
      let rect = end.getBoundingClientRect();
      // A caret on an empty line has no rect of its own; the line it sits in has. When the caret
      // is between two of the message's blocks, the line is the block at its offset.
      if (rect.height === 0) {
        const node = end.endContainer;
        const element =
          node.nodeType === 1
            ? ((node.childNodes[end.endOffset] ?? node.childNodes[end.endOffset - 1] ?? node) as Node)
            : node.parentElement;
        if (!element || element.nodeType !== 1) return null;
        rect = (element as Element).getBoundingClientRect();
      }
      return { top: rect.top, bottom: rect.bottom };
    },
  };
}
