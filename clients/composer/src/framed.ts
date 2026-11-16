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
      const rect = end.getBoundingClientRect();
      if (rect.height > 0) return { top: rect.top, bottom: rect.bottom };
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
      return lineIn(block.getBoundingClientRect(), win.getComputedStyle(block), !after && !!before);
    },
  };
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
