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
   `maximum-scale`, and no `minimum-scale`. An explicit `initial-scale` pins the page at 1:1, and
   that is the condition under which Blink declines to shrink an over-wide message (rule 3); the
   next two would take the reader's pinch away (rule 4); and the last WebKit ignores while the
   layout width is the pane's, so it would read as a promise the tag does not keep.

3. **A message written for a wider pane is reflowed to fit, not clipped.** This is the half of
   real mail that has no `@media` rules at all: a table pinned to 600px in the markup and again
   inline, its cells pinned to a third of that each. Rule 1 cannot help it, because nothing in it
   adapts.

   **The reflow is in the shared document**, `reflow_css` beside `render_document`
   (`crates/mailcal-app/src/html/reflow.rs`), so it is one implementation and every client has it.
   A client adds nothing to it and cannot opt out of it.

   **The approach is the mail clients' own**, the "munger" that came from AOSP Email through K-9
   Mail and Thunderbird for Android and that Infomaniak's iOS client still runs: make the content
   *reflow* before scaling anything, because a reflowed message is readable at the reader's own
   text size while a scaled one is a small picture of a message.

   **Where ours differs is that it is CSS, so it needs no script and no measurement.** Those clients
   run the munger as JavaScript inside the message, which `rendering-security.md` gates 1 and 2
   forbid here, and hand it the pane's width, which the core does not have. Both go away:

   - `max-width:100%` is the whole of "shrink this if the pane is narrower", at every width, with
     no number in it. It is unconditional, because it can only ever shrink something.
   - Clearing the **fixed widths on a table's cells** cannot be unconditional. A table is as wide as
     its columns whatever its `max-width` says, so the widths have to go; but clearing them in a
     pane that can hold the message would flatten a three-column newsletter on a desktop. They go
     behind a media query whose breakpoint is the widest fixed width the message itself asks for,
     **plus the document's own inset on both edges**.

   So the engine decides, at every layout, whether this message fits this pane, and re-decides it on
   a resize or a rotation with nothing re-rendered and no host involved.

   ⚠️ **The breakpoint is the message's width plus the page's own padding**, and the second half is
   not a detail: a 600px message in a 612pt pane has 584pt to lay out in and is clipped, so a
   breakpoint at 600 leaves a 28px band of pane widths where nothing fits and nothing fires. A
   13-inch iPad's reading pane lands in that band, and no narrower device shows it.

   ⚠️ **The selector asks for a width that cannot adapt on its own**, an attribute that is not a
   percentage, and that is load-bearing rather than fussy. `width:auto` on a table shrinks it to its
   contents, and the full-bleed band, `<table width="100%" bgcolor>` wrapping a column, is in half
   the newsletters ever sent: clear that one and the colour stops reaching the edges while the text
   inside it still looks right. Both shapes are seeded, `11-fixed-width-newsletter.eml` and
   `12-banded-newsletter.eml`, because each is the other's regression test.

   **What is left for an engine is the remainder**: a message the reflow cannot narrow far enough,
   because a cell holding a 300px word is 300px wide. Blink scales that down
   (`loadWithOverviewMode`, which needs rule 2 in force). WebKit has no route to it, and neither do
   WebView2 or WebKitGTK, so that pane scrolls sideways; the three WebKit routes that look like one
   are under "Known gaps", because each fails in a way worth not rediscovering.

4. **The reader can zoom, with the gesture the platform already taught them**: pinch on a
   touchscreen, pinch on a trackpad, and the host's own zoom keys where it binds them. The range is
   a browser's, 0.25× to 5×.

   ⚠️ **On iOS and iPadOS it starts at 1, not at 0.25**, and no client decides that: rule 1's
   `width=device-width` makes the layout viewport the pane, and WebKit will not scale a page below
   the size at which its layout viewport fits, so `minimumZoomScale` is 1 whatever the tag says.
   The reader can pinch **in** to 5× and cannot pinch **out**. It is the constraint that closes
   rule 3 there, seen from the reader's side, and it is why a wide message on a phone is scrolled
   rather than zoomed out of.

   A zoom control is never a page element and never an injected script: it is the host's, outside
   the document, or the contract in `rendering-security.md` is weakened to make a message easier to
   read.

   ⚠️ **On Linux the pinch has to claim its sequence.** Every `WebKitWebView` installs a
   `GtkGestureZoom` of its own, and it is not inert: it drives WebKit's magnification, which scales
   the rendered page on the compositor while the gesture is held and commits that as a page scale
   when it ends. A gesture that does not claim reports the event unhandled, so it carries on past
   the capture phase to WebKit's, which claims it in its own `scale-changed` and denies ours in the
   same stroke. Two things follow, on a touchscreen and a precision touchpad alike: after an event
   or two the scale the reader is moving is WebKit's rather than `zoom-level`, and the picture ends
   up at the **product** of the two, a pinch that laid out at 1.6× showing 3×; and the commit leaves
   the page layer scaled with no offset for one frame, which the reader sees as the message jumping
   to its top-left corner and back on release. Ours claims in `begin`, which stops the event at the
   capture phase and leaves `zoom-level` the only scale that moves.

   What the claim costs is the toolkit's cheap compositor scale, and `zoom-level` is not cheap: it
   re-lays-out the document, which no engine can do at a touchscreen's report rate. So a held pinch
   buys a layout only where it has moved 5%, and the scale it ended on is applied in full on
   release. The message steps rather than glides, and what steps is the layout rather than a picture
   of it.

5. **A zoom belongs to the message it was made on.** Opening another message starts again at 1:1,
   or, where rule 3 reaches, at that message's own fit. A carried-over zoom would mean the next
   message opens at a scale chosen for the last one, and a reader who zoomed in on a short note
   would meet the one after it magnified past its own margins.

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
| 3 · over-wide is reflowed to fit | the shared document's `reflow_css`, on all three | the same, plus `settings.loadWithOverviewMode = true` for the remainder | the shared document's `reflow_css` | the shared document's `reflow_css` |
| 4 · reader zoom | macOS `allowsMagnification = true` (trackpad pinch); iOS/iPadOS the scroll view's own pinch, from the viewport, **1× to 5×** (see rule 4) | `builtInZoomControls = true` + `displayZoomControls = false` (pinch, without the legacy floating buttons) | `IsPinchZoomEnabled` + `IsZoomControlEnabled` (touch pinch, Ctrl+scroll, Ctrl +/−) | `GtkGestureZoom` + Ctrl+scroll → `zoom-level`, both controllers in the **`Capture`** phase, the pinch **claiming** its sequence (see rule 4) |
| 5 · resets per message | macOS: `loadReadingDocument` sets `magnification = 1`, the **view's** scale, which survives a load. iOS/iPadOS: nothing to reset, the only scale there is the page's and WebKit drops it | page scale resets on load | **not reachable** (see gaps) | `set_zoom_level(1.0)` in `SecureWebView::load` / `clear` |

Source of truth per client: the same four files
[`rendering-security.md`](rendering-security.md) names.

## Known gaps

- **What the reflow cannot narrow, only Blink scales.** Rule 3's reflow is every client's, but a
  message it cannot narrow far enough still overflows: a cell holding a 300px word, or a fixed width
  written only in an inline style (below). Android scales that remainder down; everywhere else the
  pane scrolls sideways, and on iPhone and iPad with no zooming out to be had either (rule 4).

  **WebKit has no route to the scaling, and the three that look like one each fail differently**,
  measured on an iPhone rather than read off the documentation, because the documentation is what
  suggested it was already handled:

  - `WKWebView.pageZoom` does not shrink the page. It widens the layout viewport to `pane ÷ zoom`
    and leaves every box the size it was, and a document laid out wider than the window is exactly
    what WebKit **inflates text** in, to keep it legible. The boost scales `font-size` and not a
    `line-height` given in `px`, which is how mail sets line heights, so a fitted newsletter
    rendered with its lines piled on top of each other, at a size the fit had made *larger* rather
    than smaller. `-webkit-text-size-adjust` does not turn that boost off, neither `none` nor a
    percentage, on the document or on the element.
  - `scrollView.zoomScale` is clamped back. WebKit recomputes `minimumZoomScale` from the viewport
    on its own next layout pass, and the assignment is gone a frame later with nothing reported.
  - `minimum-scale` in the viewport does not unlock that clamp: with `width=device-width` the layout
    viewport already **is** the pane, so WebKit's own minimum is the scale at which it fits, which
    is 1, and a smaller `minimum-scale` in the tag is ignored.

  What is left is scaling the host view with a transform, which resamples the rendered surface
  instead of laying the page out again, and is the trade this page already declines for Windows.

  ⚠️ **A measurement of a document is worth only as much as the layout it came from.** WebKit lays
  one out more than once on the way in, and the **first** `contentSize` it reports is from a pass
  against its 980px fallback viewport rather than against the pane, so a ratio taken there is the
  pane over a width nothing was ever drawn in. That is how the `pageZoom` above came to be fed 0.45
  where the message needed 0.73. Measuring at all is the host asking **its own view** a question,
  never the message one: measuring from inside the document would need script in it, which
  `rendering-security.md` gates 1 and 2 forbid.
- **A fixed width written only in an inline style is left to `max-width` alone.** The selector that
  clears widths asks for the attribute first (`table[width]`), because that is what tells a relative
  width from a fixed one without guessing at CSS text. A `<table style="width:600px">` with no
  `width` attribute therefore keeps its width and can still overflow. In practice a table width in
  mail is written as the attribute, with or without a style beside it, because Outlook has never
  honoured anything else.
- **Rule 3 is verified by eye, per engine.** What a real message does with a media query is a
  question about an engine, so the Rust suites hold what the sheet *says* and the rendering is
  checked on each client against the two seeded newsletters, on both sides of the breakpoint. The
  pane widths that matter are a phone (≈390), an 11-inch iPad (≈495), a 13-inch iPad (≈610, the one
  that found the document's own inset missing from the breakpoint) and a desktop window dragged from
  wide to narrow.
- **A zoom outlives its message on Windows.** Rule 5 is unreachable there: the zoom factor lives on
  `CoreWebView2Controller.ZoomFactor`, and `Microsoft.UI.Xaml.Controls.WebView2` (Windows App SDK
  2.4) surfaces neither that property nor the controller that owns it. `CoreWebView2` and
  `CoreWebView2Settings` carry no zoom member at all, and WebView2 resets the factor only on a
  navigation to a *different* origin, which every `NavigateToString` document shares. The reader
  keeps their zoom until they change it, so a Windows reader who zoomed for one message meets the
  next one at that scale. The honest fix is an SDK that exposes the controller; the alternative,
  scaling the element with a `ScaleTransform`, would resample the rendered surface rather than
  re-lay-out the page and is worse than the gap.
- **The Linux claim is not in any suite.** Rule 4's ⚠️ is the one thing on this page that no
  assertion reaches: whether the two gestures still both act is a question about event delivery, and
  the widget suite delivers no touch while the AT-SPI run drives semantic actions. What a widget
  test does hold is that our controllers exist, are named and are in the `Capture` phase. The claim
  itself is checked by pinching a message on a Linux machine with a touchscreen or a precision
  touchpad and watching the release: a message that jumps to its top-left corner and back has lost
  it.
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
