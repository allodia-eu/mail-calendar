// The colour scheme the editor's chrome is drawn in, as the host names it.
//
// A web view's `prefers-color-scheme` answers what its engine decides, and the engines decide
// differently: WebKit on Apple follows the window, while WebView2, Android's WebView and WebKitGTK
// can follow the system and miss a Light or Dark chosen in the app. So each host says which scheme
// the app is in, on load and whenever it changes, and the stylesheet resolves every chrome colour
// against it (`editor.css`). The page itself stays light either way.
//
// Set on the root, never on the editor: the revision counter watches the editor's attributes, and a
// scheme change is not an edit.

export type Scheme = "light" | "dark";

/// Names the scheme the chrome follows. Anything but "light" or "dark" hands it back to the
/// system's, which is also what a page no host has spoken to yet uses.
export function setScheme(root: HTMLElement, scheme: unknown): void {
  if (scheme === "light" || scheme === "dark") {
    root.dataset.scheme = scheme;
  } else {
    delete root.dataset.scheme;
  }
}
