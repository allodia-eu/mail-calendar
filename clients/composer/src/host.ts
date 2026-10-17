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

export function installNativeChrome(editor: HTMLElement, toolbar: HTMLElement): NativeChrome {
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
  // page that has it: the native header taking focus blurs the page and leaves `activeElement` on
  // the editor, so `hasFocus` is what tells the two apart. A toolbar control holding focus counts
  // whatever the page says, because a `select`'s picker is a native dialog that blurs the page while
  // it is open, and hiding the control under it would lose the choice.
  const syncWriting = () => {
    const active = doc.activeElement;
    const writing = (doc.hasFocus() && editor.contains(active)) || toolbar.contains(active);
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
