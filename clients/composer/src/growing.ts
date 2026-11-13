// Growing mode: the editor laid out at the height of its content, for a host whose composer is one
// column that scrolls as a whole (`docs/composer-layout.md`, rules 5 and 6).
//
// The page never scrolls in this mode. It reports the height its toolbar and text need, and the
// host sizes the WebView to `composer_editor_height` of it; it reports where the caret is, and the
// host scrolls its column to keep it in view. While the text is short the host's frame is the
// taller of the two, and the editor stretches to fill it, so the blank area under the last line
// still puts the caret in the message. A host that never calls `useGrowingComposer` keeps the
// layout where the editor box scrolls itself.

import { windowOf } from "./dom";

/// Where the reports go: the host channel's `height` and `caret` notices.
export interface GrowingReports {
  height(px: number): void;
  caret(top: number, bottom: number): void;
}

/// What growing mode measures, in CSS pixels from the top of the page.
export interface GrowingMeasures {
  /// The height the toolbar and the text need, however tall the host's frame is.
  contentHeight(): number;
  /// The caret's top and bottom, or `null` when the selection is not in the message.
  caret(): { top: number; bottom: number } | null;
}

export interface Growing {
  useGrowingComposer(): void;
  /// Sends both reports again, for a host that has just announced it takes them.
  resend(): void;
}

export function installGrowing(
  editor: HTMLElement,
  toolbar: HTMLElement,
  reports: GrowingReports,
  measures: GrowingMeasures = domMeasures(editor, toolbar),
): Growing {
  const doc = editor.ownerDocument;
  const win = windowOf(editor);
  let on = false;
  let lastHeight = -1;
  let lastCaret = "";
  let scheduled = false;

  const reportHeight = (force: boolean) => {
    const px = Math.ceil(measures.contentHeight());
    if (!force && px === lastHeight) return;
    lastHeight = px;
    reports.height(px);
  };

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

  // A burst of mutations, a paste or a picture loading, is measured once, after layout.
  const schedule = () => {
    if (!on || scheduled) return;
    scheduled = true;
    win.requestAnimationFrame(() => {
      scheduled = false;
      reportHeight(false);
      reportCaret(false);
    });
  };

  return {
    useGrowingComposer() {
      if (on) return;
      on = true;
      doc.documentElement.classList.add("growing");
      doc.body.classList.add("growing");
      // Written words, a list made or a table removed: every change to the message's tree.
      new win.MutationObserver(schedule).observe(editor, {
        childList: true,
        subtree: true,
        characterData: true,
        attributes: true,
      });
      // A picture's height is known only once it has loaded; `load` does not bubble, so capture.
      editor.addEventListener("load", schedule, true);
      doc.addEventListener("selectionchange", schedule);
      // A narrower window wraps the toolbar and the text onto more lines.
      win.addEventListener("resize", schedule);
      if (win.ResizeObserver) new win.ResizeObserver(schedule).observe(toolbar);
      schedule();
    },
    resend() {
      if (!on) return;
      reportHeight(true);
      reportCaret(true);
    },
  };
}

/// The measurements, from the laid-out page. The editor stretches to fill the host's frame, so its
/// own box says how tall the frame is, not how tall the text is: the text's height is read from the
/// range around its contents instead, which never includes the stretch.
function domMeasures(editor: HTMLElement, toolbar: HTMLElement): GrowingMeasures {
  const doc = editor.ownerDocument;
  const win = windowOf(editor);
  // The page's top, as the text would sit with nothing scrolled. `overflow: clip` keeps the body
  // from scrolling at all; its offset is added as well, so a page that did scroll still reports its
  // whole height rather than what is left in view.
  const pageTop = () => doc.documentElement.getBoundingClientRect().top - doc.body.scrollTop;
  const px = (value: string) => Number.parseFloat(value) || 0;
  return {
    contentHeight() {
      const style = win.getComputedStyle(editor);
      const box = editor.getBoundingClientRect();
      const range = doc.createRange();
      range.selectNodeContents(editor);
      const text = range.getBoundingClientRect();
      // An empty message has no box of its own; it is the placeholder's one line.
      const textBottom =
        text.height > 0 ? text.bottom : box.top + px(style.paddingTop) + px(style.lineHeight);
      const editorBottom = textBottom + px(style.paddingBottom) + px(style.borderBottomWidth);
      return Math.max(editorBottom, toolbar.getBoundingClientRect().bottom) - pageTop();
    },
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
      const top = pageTop();
      return { top: rect.top - top, bottom: rect.bottom - top };
    },
  };
}
