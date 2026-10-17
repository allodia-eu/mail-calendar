// Native-chrome mode: the single-scroll layout Android hosts the editor in.
//
// The native surface owns one scroll; the compose header sits in a native overlay above this
// WebView and scrolls away as the message grows, and the toolbar pins just above the keyboard, one
// row that scrolls sideways, while the message is being written. So
// the *page* scrolls (not an inner editor box), the toolbar is fixed to the viewport bottom, and the
// editor reserves room for the native header (top) and the fixed toolbar (bottom) via insets the
// host sets. A host that never calls `useNativeComposerChrome` keeps the desktop flex layout.

import { windowOf } from "./dom";

export interface NativeChrome {
  useNativeComposerChrome(): void;
  setComposerTopInset(cssPx: unknown): void;
}

/// `onStopWriting` runs when the message stops being written, for the toolbar to close its panels.
export function installNativeChrome(
  editor: HTMLElement,
  toolbar: HTMLElement,
  onStopWriting: () => void = () => {},
): NativeChrome {
  const doc = editor.ownerDocument;
  const win = windowOf(editor);

  // The fixed toolbar's height, so the last line of the message clears it.
  const setBottomInset = () => {
    doc.documentElement.style.setProperty(
      "--composer-bottom-inset",
      `${toolbar.offsetHeight}px`,
    );
  };

  // Make the empty editor fill the viewport, so tapping anywhere in the blank area below the first
  // line puts the caret in the body rather than doing nothing.
  //
  // This has to be measured in JS: **viewport units do not work in this WebView.** Compose lays the
  // WebView out *after* the page has been loaded into it, so Chromium establishes the layout
  // viewport at zero height and every viewport-percentage length is stuck there: `100vh` and
  // `height: 100%` both compute to `0px`, silently. A CSS `min-height: 100vh` is therefore not
  // merely fragile here, it is dead, and the editor collapses to the height of its one empty line
  // while the rest of the composer looks like a body you cannot tap. `clientHeight` reports the true
  // height, so size from that, and re-measure on resize (the keyboard opening changes it).
  //
  // Set on the root, never on the editor: the revision counter watches the editor's attributes,
  // so a height written there would read as an edit every time the keyboard moved, and a stored
  // draft would be saved over with nothing typed.
  const fillViewport = () => {
    doc.documentElement.style.setProperty(
      "--composer-fill-height",
      `${doc.documentElement.clientHeight}px`,
    );
  };

  // The toolbar formats the message, so it is drawn while the message is being written and gone
  // while an address field in the native header has the keyboard, where it would only take room
  // from the field being typed in. "Being written" is the message or the toolbar holding focus in a
  // page that has it: the native header taking focus blurs the page and leaves `activeElement` where
  // it was, so `hasFocus` is what tells the two apart. A panel left open would come back with the
  // toolbar, over a message the user has since moved on from, so it is closed as the toolbar goes.
  let writing = false;
  const syncWriting = () => {
    const active = doc.activeElement;
    const now = doc.hasFocus() && (editor.contains(active) || toolbar.contains(active));
    if (writing && !now) onStopWriting();
    writing = now;
    doc.body.classList.toggle("composer-writing", writing);
  };

  return {
    useNativeComposerChrome() {
      doc.documentElement.classList.add("native-chrome");
      doc.body.classList.add("native-chrome");
      syncWriting();
      doc.addEventListener("focusin", syncWriting);
      // After the move, so `activeElement` already names where focus went.
      doc.addEventListener("focusout", () => win.setTimeout(syncWriting, 0));
      win.addEventListener("focus", syncWriting);
      win.addEventListener("blur", syncWriting);
      // A picked font, size or style goes back to the message, which is where the next keystroke
      // belongs. Left on the `select`, focus would keep the toolbar up as "writing" with nothing to
      // type into. Here rather than in the toolbar, because on a desktop a keyboard user changes a
      // `select` with the arrow keys, and each arrow is a `change`.
      toolbar.addEventListener("change", (event) => {
        if ((event.target as Element | null)?.tagName === "SELECT") editor.focus();
      });
      setBottomInset();
      fillViewport();
      // The toolbar comes and goes with focus; keep the inset in step, so the message's last line
      // clears it while it is there and the page does not end in a blank band while it is not.
      if (win.ResizeObserver) new win.ResizeObserver(setBottomInset).observe(toolbar);
      // The keyboard opening/closing resizes the viewport; keep the fill in step with it.
      win.addEventListener("resize", fillViewport);
    },

    /// The height of the native header overlaid on the WebView, in CSS px, so the editor's text
    /// starts just below it and the two scroll in lockstep.
    setComposerTopInset(cssPx: unknown) {
      const px = Number(cssPx);
      doc.documentElement.style.setProperty(
        "--composer-top-inset",
        `${Number.isFinite(px) && px > 0 ? px : 14}px`,
      );
    },
  };
}
