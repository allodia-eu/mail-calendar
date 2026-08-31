// Native-chrome mode: the single-scroll layout Android hosts the editor in.
//
// The native surface owns one scroll; the compose header sits in a native overlay above this
// WebView and scrolls away as the message grows, and the toolbar pins just above the keyboard. So
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
  const fillViewport = () => {
    editor.style.minHeight = `${doc.documentElement.clientHeight}px`;
  };

  return {
    useNativeComposerChrome() {
      doc.documentElement.classList.add("native-chrome");
      doc.body.classList.add("native-chrome");
      setBottomInset();
      fillViewport();
      // The toolbar can wrap to a second row on a narrow screen; keep the inset in step.
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
