// The script in the document's head: the app's scheme and the page a host handed this document
// before it ran (`composer_page_preload_script`), drawn ahead of any of the editor's markup.
//
// The editor's own script sits at the end of the body, and an engine may paint the markup above it
// while that script is still arriving: WebView2 does, streaming a document handed over as a string.
// That frame would show the chrome in the system's scheme and the message on the light page. The
// editor's script applies both again as it starts (`main.ts`), which is what installs the page.

import { pageOf, paintPage } from "./page_paint";
import { setScheme } from "./scheme";

/// Applies what `win`'s host preset, if anything, to its document's root.
export function applyPreset(win: Window & { composerScheme?: unknown; composerPage?: unknown }): void {
  const root = win.document.documentElement;
  if (win.composerScheme !== undefined) setScheme(root, win.composerScheme);
  const page = pageOf(win.composerPage);
  if (page) paintPage(root, page.scheme, page.colours);
}
