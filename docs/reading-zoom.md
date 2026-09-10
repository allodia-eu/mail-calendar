# Fitting and zooming a message: cross-platform contract

**Scope.** How a message body is *sized* in the reading view on every client: the width it lays out
against, what happens when it is wider than the pane anyway, and how the reader changes the scale.
What may be rendered at all, and under which gates, is a different contract and stays in
[`rendering-security.md`](rendering-security.md); this one starts after those gates are met.

**Why it is a contract and not a per-client detail.** The width a message lays out against decides
which of its own `@media` rules apply, so it decides what the message *is*. Two clients that
disagree about it show the reader two different newsletters from the same bytes.

## The rules

1. **A message lays out against the pane's own width.** The shared document
   (`render_document`, `crates/mailcal-app/src/html/mod.rs`) declares
   `<meta name="viewport" content="width=device-width">`, and `device-width` is the **web view's**
   CSS width, not the display's. So a message's `@media (max-width: 480px)` rules see the reading
   pane the reader actually has: a phone, a narrow iPad column and a desktop window dragged narrow
   all get the layout the sender wrote for that width.

   A client must therefore let the viewport declaration reach its engine. Android's is the trap:
   `useWideViewPort` is **false** by default and that ignores the tag outright.

2. **The declaration names a width and no scale.** No `initial-scale`, no `user-scalable`, no
   `maximum-scale`. An explicit `initial-scale` pins the page at 1:1, and that is the condition
   under which both touch engines decline to shrink an over-wide message (rule 3); the other two
   would take the reader's pinch away (rule 4).

3. **A message wider than the pane is scaled down to fit, not clipped**, wherever the engine can do
   it. This is the half of real mail that has no `@media` rules at all: a table pinned to 600px in
   the markup and again inline. Rule 1 cannot help it, because there is nothing in it to reflow.

   Only the touch engines offer this, and each needs rule 2 to be in force. **On the desktop hosts
   there is no such setting**, and the only way to compute the scale would be to measure the
   document's width from inside it, which needs script in the message: `rendering-security.md`
   gates 1 and 2 forbid that, and no rendering convenience is worth reopening them. So a desktop
   pane narrower than the message scrolls horizontally, and the reader zooms out under rule 4 if
   they would rather see it whole. That is also what Thunderbird does on a desktop.

4. **The reader can zoom, with the gesture the platform already taught them**: pinch on a
   touchscreen, pinch on a trackpad, and the host's own zoom keys where it binds them. The range is
   a browser's, 0.25× to 5×.

   A zoom control is never a page element and never an injected script: it is the host's, outside
   the document, or the contract in `rendering-security.md` is weakened to make a message easier to
   read.

5. **A zoom belongs to the message it was made on.** Opening another message starts again at that
   message's own fit. Rule 3 gives each message the scale that fits *it*, so a carried-over zoom
   would mean the next message opens at a scale chosen for the last one, and a reader who zoomed in
   would still get a wide message clipped.

   Where the scale is the *page's* it resets on load and the client does nothing. Where it is the
   *view's* it survives the load and the client MUST reset it: macOS's `magnification`, WebView2's
   `ZoomFactor` and WebKitGTK's `zoom-level` are all of the second kind.

6. **A message cannot change any of this.** The sanitiser drops `<meta>`
   ([`rendering-security.md`](rendering-security.md) → Layer 1), so the only viewport in the
   document is the one rule 2 wrote. This is what makes it safe for a client to honour a viewport
   tag at all.

## Per-platform implementation matrix

| Rule | Apple · `WKWebView` | Android · `WebView` | Windows · `WebView2` | Linux · `WebKitGTK` |
|---|---|---|---|---|
| 1 · lays out at the pane's width | honours the tag by default | `settings.useWideViewPort = true` (**false by default: the tag is ignored without it**) | honours the tag by default | honours the tag by default |
| 3 · over-wide is scaled to fit | iOS/iPadOS: WebKit's own shrink-to-fit. macOS: n/a (see rule 3) | `settings.loadWithOverviewMode = true` | n/a (see rule 3) | n/a (see rule 3) |
| 4 · reader zoom | macOS `allowsMagnification = true` (trackpad pinch); iOS/iPadOS the scroll view's own pinch, from the viewport | `builtInZoomControls = true` + `displayZoomControls = false` (pinch, without the legacy floating buttons) | `IsPinchZoomEnabled` + `IsZoomControlEnabled` (touch pinch, Ctrl+scroll, Ctrl +/−) | `GtkGestureZoom` + Ctrl+scroll → `zoom-level`, both controllers in the **`Capture`** phase |
| 5 · resets per message | macOS `magnification = 1` in `loadReadingDocument`; iOS/iPadOS the page scale resets itself | page scale resets on load | **not reachable** (see gaps) | `set_zoom_level(1.0)` in `SecureWebView::load` / `clear` |

Source of truth per client: the same four files
[`rendering-security.md`](rendering-security.md) names.

## Known gaps

- **No automatic fit on the desktop hosts.** Rule 3 holds on Android, iOS and iPadOS only. The
  reason is in the rule and it is a real constraint, not an omission of effort, but the outcome is
  still that a 600px message in a 400px-wide macOS, Windows or Linux pane scrolls sideways until
  the reader zooms out. If it ever becomes worth closing, the honest route is a host-side measure of
  the laid-out document, which none of the three engines currently exposes without script.
- **A zoom outlives its message on Windows.** Rule 5 is unreachable there: the zoom factor lives on
  `CoreWebView2Controller.ZoomFactor`, and `Microsoft.UI.Xaml.Controls.WebView2` (Windows App SDK
  2.4) surfaces neither that property nor the controller that owns it. `CoreWebView2` and
  `CoreWebView2Settings` carry no zoom member at all, and WebView2 resets the factor only on a
  navigation to a *different* origin, which every `NavigateToString` document shares. The reader
  keeps their zoom until they change it, so a Windows reader who zoomed for one message meets the
  next one at that scale. The honest fix is an SDK that exposes the controller; the alternative,
  scaling the element with a `ScaleTransform`, would resample the rendered surface rather than
  re-lay-out the page and is worse than the gap.
- **On Linux, WebKitGTK carries a pinch gesture of its own, and whether the two compound is
  untested.** Every `WebKitWebView` installs a `GtkGestureZoom`, which is why our controllers are
  identified by name rather than by type (a lookup by type finds the toolkit's and proves nothing).
  Ours sits ahead of it in the `Capture` phase and drives `zoom-level`; if WebKit's also acts on the
  same touch sequence, a pinch would move the scale twice. Nothing here can answer that: the widget
  suite delivers no touch, and the AT-SPI run drives semantic actions. It needs one pinch on a Linux
  machine with a touchscreen or a precision touchpad. If they do compound, the fix is to have our
  gesture claim the sequence rather than to drop it, since `zoom-level` is what rules 4 and 5 are
  written against.
- **No keyboard zoom on macOS and Linux.** Windows gets Ctrl +/− from WebView2 and Linux gets
  Ctrl+scroll, but neither macOS nor Linux binds the `+`/`−`/`0` keys, because that is a menu
  command on macOS and an application shortcut on Linux rather than a web-view setting. Both have
  the pinch.
- **Rule 4 is verified by hand.** A pinch is a gesture on a real compositor: the Android JVM suite
  has no renderer (`docs/client-traps.md`), there is no Apple UI-test target, and the Linux AT-SPI
  run drives semantic actions rather than touch. What each client's suite *can* hold is rule 5's
  reset and rule 2's declaration, which is where the assertions are.

## Enforcement

Same as [`rendering-security.md`](rendering-security.md): the rule and the matrix move together, in
the same change, on every platform that ships the reading view, and a shortfall goes under "Known
gaps" rather than staying silent.
