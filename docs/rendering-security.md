# Untrusted message rendering: cross-platform security contract

**Scope.** How every Allodia Mail & Calendar client renders a message body (the reading view).
Mail HTML is hostile input: it tries to run script, exfiltrate via remote loads, track the open
with remote images/pixels, navigate the host away, and escape its frame. This document is the
**single bar** all clients meet: Apple (macOS, iOS, iPadOS), Android, Windows, and any future
platform (Linux, web, …). It applies the security posture in
[`../AGENTS.md`](../AGENTS.md) → "Non-negotiables" to this product's rendering surfaces.

**Principle.** A privacy/security gate here is a **contract, not a per-client detail**. Adding or
raising a gate on one platform raises it for **all**; see the enforcement rule at the end and in
[`../AGENTS.md`](../AGENTS.md). Raising the bar anywhere raises it everywhere.

## The layers

Defence in depth: each layer holds even if another is weakened. Layers 1–2 are **shared Rust**
(written once, identical for every client); layer 3 is the **native host renderer** (each client
implements every gate).

### Layer 1: Core sanitisation (shared) · `crates/mailcal-app/src/html/mod.rs`

The engine returns the raw `text/html` part **unsanitized** by design (it is hostile input). The
core sanitises it **once** for every client (never re-implemented per platform):

- **Removed:** `<script>` + contents, event handlers (`on*`), `<iframe>`/`<object>`/`<embed>`/
  `<form>`, `<base>`/`<meta>` (so a message can't set its own refresh/CSP), and any URL scheme
  other than `http(s)`/`mailto`/`data`/`cid`.
- **Kept:** presentational HTML/CSS: inline `style`, `<style>` blocks, `class` (stripping CSS
  makes real mail illegible), and remote `<img>` sources (the load is gated downstream, not the
  markup).
- **Reported:** `has_remote_images`: whether the body *would* load a remote resource (remote
  `<img>`, CSS `url(...)`, `@import`, protocol-relative `//…`, refs inside `<style>`), so a client
  offers the "load remote images" confirmation only when there is something to gate.
- **Resolved (inline images):** an inbound `<img src="cid:…">` (an inline image whose bytes live
  in a sibling MIME part with a matching `Content-ID`) is rewritten to a self-contained `data:`
  URI (`inline_cid_images`, fed by the engine's `message_inline_parts`), so it renders under the
  existing CSP with no network load. This is done **once in shared Rust** for every client. Two
  hard limits keep it safe: only `image/*` parts are inlined (the rewrite can **never** emit an
  executable/document `data:` such as `data:text/html`), and only an `<img src>` is rewritten (the
  one place the sanitiser keeps `cid:`). Inline images are part of the message (**local**, not a
  remote fetch), so they do **not** set `has_remote_images` and are **not** gated behind the
  remote-image opt-in; an unresolved or non-image `cid:` is left as an inert broken image.
- **Downloadable attachments:** regular MIME attachments are projected as sanitised metadata
  (`AttachmentRow`) beside the body and are **never loaded into the WebView, rendered, or executed
  in-app**. Save and Open are both explicit, per-attachment user actions; in both, the shared core
  decodes that one part to disk through `save_attachment`, so attachment bytes do not cross FFI.
  **Open never opens the file inside our app**: the core writes the decoded part to an app-owned
  temp path and the host hands *that file* to the **OS default handler** (`NSWorkspace.open` /
  a `FileProvider` `content://` URI + `ACTION_VIEW` / `Launcher.LaunchFileAsync`), or, where the OS
  has no such handoff, to the **OS's own viewer** (iOS/iPadOS: Quick Look, which renders in the
  system's out-of-process preview extensions). Routing every open through the OS is deliberate: its
  own file scanning / antivirus runs before the file is opened, and we never build a viewer for
  hostile attachment content ourselves. **A share sheet is not an Open**: it is where a file goes
  next, so it stays on Save, and on the types the OS viewer declines outright (an installer). The
  core sanitises the suggested file name; hosts sanitise any name or extension they derive for a
  picker/temp path. Where the OS is not told the media type outright (Android passes it on the
  intent), it picks the viewer from the **extension**, so a part whose name carries none is typed
  from its media type; see "Known gaps" for where that is still outstanding. This is a
  cross-platform gate: a client may not ship the attachment surface until it meets every cell
  below.

### Layer 2: Strict-CSP document (shared) · `render_message_html` / `render_document`

The sanitised fragment is wrapped in a complete HTML document with a strict
`Content-Security-Policy` and base styling, built in shared Rust so **the security boundary and
the presentation are identical across clients**:

- `default-src 'none'`: nothing loads unless explicitly allowed below.
- `img-src data:` by default: only inline images render; **every remote image is blocked**, so a
  message cannot phone home or track the open. When the user opts in, remote `http(s)` is added to
  `img-src` and the document is re-rendered.
- No `script-src`: scripts never run, even if one survived sanitisation.
- The document has **no resolvable base origin**, so relative/remote URLs can't be rebased.

### Layer 3: Native host renderer (per platform, implement **every** gate)

The fragment is rendered in the platform's web view. The shared CSP is the primary boundary; these
native gates are mandatory defence in depth so a single weakness (e.g. a future CSP regression)
can't leak data:

1. **Scripting disabled** in the web view.
2. **No host/script bridge**: no JS↔native channel, no injected host objects.
3. **All in-view navigation blocked**: redirects, form posts, and link taps never load inside the
   web view; the rendered document is inert. The one exception is a **user-activated** link tap,
   which is handed to the OS default handler (see gate 4a) rather than navigated in place; the
   in-view navigation is still cancelled either way.
4. **No new windows / popups.**
   - **4a. External link handoff.** A link the user **clicks/taps** opens in the OS default
     browser/handler instead of the inert web view, but **only** for the safe scheme allowlist
     **`http` / `https` / `mailto`**; `data:`/`cid:`/`file:`/custom app schemes/etc. are never
     handed off. The allow-or-ignore decision is **shared Rust**: `should_open_external_link`
     (FFI), the single source of truth, so every client decides identically and **cannot
     drift**; it is the same scheme set the sanitiser keeps on `<a href>` (a link the sanitiser
     strips can never be clicked, so the two must agree). Only the actual launch is native
     (`NSWorkspace` / `Intent.ACTION_VIEW` / `Launcher.LaunchUriAsync`). The handoff is gated on
     **user activation** (`linkActivated` / `hasGesture()` / `IsUserInitiated`) so a redirect or
     programmatic navigation can't auto-launch a URL. This is an OS handoff of a URL the user
     chose to open in their own browser, not an app-initiated dispatch of user data, so it does
     not pass the `JurisdictionGate`.
5. **No file/content access** from the rendered document.
6. **Opaque document origin**: load the document in-memory with no base URL, so there's no origin
   to resolve remote/relative resources against.
7. **Remote sub-resources blocked by default**: images, fonts, CSS from `http(s)` do not load
   until the user opts into images **per message** (the choice resets per message). Prefer an
   **explicit native sub-resource interceptor** as a second barrier to the CSP, not the CSP alone.

### Layer 4: Untrusted sender text rendered **natively**, outside the web view

Not every piece of sender-controlled content goes through a web view. The **meeting-invitation card**
([`invitations.md`](invitations.md)) draws a message's `SUMMARY`, `LOCATION`, `DESCRIPTION` and the
organiser's display name as **native labels** above the body. Those four fields are attacker-controlled
and they bypass layers 1–3 entirely: no sanitiser, no CSP, no web view. So they get their own gate.

8. **Rendered as text, never as markup.** The core emits these values as plain text (control
   characters and the Unicode bidi overrides dropped, whitespace collapsed, truncated on a *character*
   boundary), and deliberately does **not** escape markup, because the contract is that they *are*
   text: escaping here would print a literal `&amp;` on every client that renders them correctly. The
   client's obligation is therefore the other half of that contract: pass the string to a **text**
   primitive, never to one that parses markup, markdown, or a localised format string.

   The trap is real and platform-specific, which is why it is written down rather than assumed:
   libadwaita rows parse their titles as **Pango markup by default** (the `use_markup(false)` rule in
   [`../AGENTS.md`](../AGENTS.md), which a visual pass missed because the test subjects had no
   ampersand), and SwiftUI's `Text` has a `LocalizedStringKey` overload that *does* parse markdown,
   reached by a literal, not by a `String` variable, but one refactor away. Where a language offers an
   unambiguous spelling, use it rather than relying on overload resolution: on Apple that is
   `Text(verbatim:)`, which has no markdown-parsing overload to fall into.

   **Test:** a title of `**bold** <b>x</b> & co` must appear on screen exactly as typed.

## Per-platform implementation matrix

Every row is mandatory on every column. A new platform may not ship the reading view until every
cell is filled.

| Gate | Apple · `WKWebView` | Android · `WebView` | Windows · `WebView2` | Linux · `WebKitGTK` |
|---|---|---|---|---|
| Scripting off | `allowsContentJavaScript = false` | `settings.javaScriptEnabled = false` | `Settings.IsScriptEnabled = false` | `Settings.enable_javascript = false` on the dedicated reading host |
| No host/script bridge | (no message handlers added) | (no `addJavascriptInterface`) | `AreHostObjectsAllowed = false`, `IsWebMessageEnabled = false` | fresh `UserContentManager`; no script-message handlers or host objects |
| In-view navigation blocked | `decidePolicyFor` cancels all but the initial in-memory load | `shouldOverrideUrlLoading → true` | `NavigationStarting` cancels all but our `NavigateToString` | `decide-policy` permits only the expected initial `about:blank` load; all later navigation is ignored |
| New windows blocked | navigation policy cancels; no `WKUIDelegate` opens windows | (navigation cancelled) | `NewWindowRequested.Handled = true` | `create → None`; `NewWindowAction` is ignored |
| External link handoff (user-activated; scheme decided by shared Rust `should_open_external_link`: `http`/`https`/`mailto` only) | `.linkActivated` → `NSWorkspace.shared.open` | `hasGesture()` → `Intent.ACTION_VIEW` | `IsUserInitiated` (nav + `NewWindowRequested`) → `Launcher.LaunchUriAsync` | `is_user_gesture` → shared gate → `GAppInfo` OS handler; WebKit navigation stays cancelled |
| No file/content access | n/a (no file URLs; nil base) | `allowFileAccess = false`, `allowContentAccess = false` | (no file access; in-memory doc) | file/universal access from file URLs disabled; no file URL is loaded |
| Opaque document origin | `loadHTMLString(_, baseURL: nil)` | `loadDataWithBaseURL(null, …)` | `NavigateToString(…)` | `load_html(_, None)` |
| Remote sub-resources blocked by default | document CSP + nil base origin **(CSP-only; see gaps)** | document CSP + **`shouldInterceptRequest`** hard-block unless opted in | document CSP + **`WebResourceRequested`** 403 hard-block unless opted in | document CSP + native `UserContentFilter` blocks `http(s)` unless opted in |
| Remote-images opt-in (per message, resets per message) | ✓ banner → re-render | ✓ banner → re-render | ✓ banner → re-render | ✓ banner → re-render; native filter removed only for that open message |
| Inline `cid:` images (resolved to `data:` in shared Rust: `image/*` only, `<img src>` only; **local, not gated**) | renders via document CSP `img-src data:` | renders via CSP; `shouldInterceptRequest` passes `data:` (blocks only `http(s)`) | renders via CSP; `WebResourceRequested` passes `data:` (403s only `http(s)`) | renders via CSP; native filter blocks only `http(s)` |
| Downloadable attachments: metadata only in view; **never rendered/executed in-app**; explicit per-attachment Save + Open | Save: save panel (macOS) / share sheet (iOS/iPadOS) → core decodes to chosen or temp path. Open: core decodes to a temp path (extension typed from the media type when the name carries none) → `NSWorkspace.open` (OS handler) on macOS, `QLPreviewController` (OS viewer, out-of-process) on iOS/iPadOS, falling back to the share sheet for a type Quick Look cannot preview | Save: document picker → core decodes to app-private temp → host copies to chosen URI. Open: core decodes to app-cache → `FileProvider` `content://` + `ACTION_VIEW` (OS handler) | Save: save picker → core decodes to temp → host copies to chosen `StorageFile`. Open: core decodes to temp → `Launcher.LaunchFileAsync` (OS handler) | Save: `GtkFileDialog` → core decodes to chosen path. Open: core decodes to an app-owned temp path → `GAppInfo` OS handler |
| Attachment decode/write off the UI thread (large parts must not freeze/ANR) | detached task → `save_attachment` | background thread → `save_attachment` | `Task.Run` → `save_attachment` | Rust worker thread → `save_attachment`; completion returns through the Relm4 sender |
| **Gate 8**: invitation-card fields as text, never markup (native, outside the web view) | `Text(verbatim:)`: takes no `LocalizedStringKey` overload, so a refactor cannot start parsing markdown | Compose `Text(String)`: styling requires an `AnnotatedString`, which a `String` cannot become implicitly; no `HtmlCompat.fromHtml`, no `buildAnnotatedString`, no WebView | `TextBlock.Text`: markup needs a `RichTextBlock` with authored `Inline`s or an explicit `XamlReader.Load`, neither reachable from a `String`; no `XamlReader`, no WebView2, no composer bridge | `GtkLabel.set_text`: takes the string as text *and* clears `use-markup`; `set_markup` is the one-word refactor that undoes it, so the widget test asserts on the **rendered** label and on the absence of a `from markup` log record. No `WebKitWebView`, no composer bridge |

Gate 8 is now held on every column. It carried a ⬜ for as long as one client did not host the
surface: the gate bound the moment that client drew the card, which is the ordering the
cross-platform security-parity rule requires: a platform may not ship a surface until it meets
every gate in the contract.

The four languages fail differently, which is why each cell names its own spelling rather than
"render as text". Swift's danger is an *overload* (`Text` parses markdown through
`LocalizedStringKey`, reachable by a literal); GTK's is a *default* (`use_markup` is on);
Kotlin/Compose's and C#/WinUI's are neither: a `String` has no styling path at all, and the risk is
only that someone later routes the value through `HtmlCompat.fromHtml`, an `AnnotatedString` builder
or a `XamlReader.Load` to make it prettier. So on Android and Windows the gate is stated as a
prohibition on those, not as a call to make.

**The Windows cell was closed by running the test, not by reading the framework docs.** An
invitation whose `SUMMARY` is `**bold** <b>x</b> & co`, with matching `LOCATION`, `DESCRIPTION` and
organiser `CN`, was appended to the harness INBOX over IMAP and opened in the client: every field
appeared exactly as typed: no bold, no tag interpreted, no doubled `&`. Note that **UI Automation
cannot prove this**: `AutomationElement.Name` reports the source `Text` property, so it reads back
the raw string whether or not the framework parsed it. The evidence has to be the rendered pixels.

Source of truth per client:
- The invitation card (Gate 8): `clients/apple/Packages/MailcalKit/Sources/MailcalUI/InvitationCardView.swift`,
  `clients/android/app/src/main/java/eu/allodia/mailcal/InvitationCard.kt`,
  `clients/windows/Mailcal/Views/InvitationCardView.cs`,
  `clients/linux/src/ui/invitation/card.rs`
- macOS + iOS/iPadOS (shared Apple client): `clients/apple/Packages/MailcalKit/Sources/MailcalUI/ReadingView.swift`,
  one `WKWebView` host serves every Apple platform. On iOS/iPadOS the external-link handoff uses
  `UIApplication.open`, attachment Open uses Quick Look and attachment Save the share sheet: the
  iOS analogues of macOS's `NSWorkspace.open` / save panel (`PlatformShims.swift`).
- Android: `clients/android/app/src/main/java/eu/allodia/mailcal/ReadingScreen.kt`
- Windows: `clients/windows/Mailcal/Views/ReadingView.xaml.cs`
- Linux: `clients/linux/src/ui/reading.rs` + `clients/linux/src/ui/webview.rs`

## Known gaps / follow-ups

- **Inline-image height pin is overridden globally.** The reading document's base CSS uses
  `img { max-width: 100%; height: auto !important }` (`html/mod.rs`, `base_css`) so a width-pinned image
  can't keep a fixed height while `max-width` shrinks its width: the squashed-aspect-ratio fix. The
  `!important` is deliberately broad, so it *also* overrides a message that pins **only** the height
  (e.g. a signature `<img style="height:32px">` with auto width): such an image renders at its full
  intrinsic height instead of the requested size. A narrower rule (force `height:auto` only when the
  width is also constrained) would fix this, but **this CSS is load-bearing for already-validated
  rendering** (the corporate-logo aspect-ratio case): any change here carries a real regression risk
  and MUST land with extensive tests across all four pin combinations (width-only, height-only,
  both, neither) before it ships. Tracked as a follow-up.
- **An extension-less attachment opens as `.bin` on Windows and Linux.** Both write the temp copy
  with the extension from the sender's file name and fall back to `.bin` when it has none
  (`ExtensionFor`, `safe_extension`), so a PDF sent as a bare `invoice` reaches the OS untyped:
  Windows raises "how do you want to open this file?", and Linux depends on GIO sniffing the
  content. Apple derives the extension from the media type (`attachmentFileName`); the other two can
  do the same from `media_type`, which both already have on the row.
- **Apple explicit sub-resource barrier.** Apple currently blocks remote sub-resources via the
  document CSP + a nil base origin (WebKit enforces the CSP), but, unlike Android and Windows,
  has no explicit native interceptor as a second barrier. To bring it to full parity with this
  contract, add a `WKContentRuleList` that blocks network loads unless the user opted in. Tracked
  as a follow-up for macOS, iOS, and iPadOS.
- **iOS/iPadOS reading view newly ported (Apple multiplatform migration).** It shares the
  macOS `WKWebView` host **verbatim** (scripting off, in-view navigation blocked, new windows blocked,
  opaque `baseURL: nil` origin, document CSP), so every gate is met by construction; only the
  external-link handoff (`UIApplication.open`) and attachment Open/Save (Quick Look / share sheet)
  use the iOS analogues noted above. **Build-green and real-account verified on the iPhone/iPad
  simulators;** background delivery remains the coordinated iOS/Android follow-up before iOS/iPadOS
  ship to users.

## Enforcement

This contract is binding via [`../AGENTS.md`](../AGENTS.md) ("Cross-platform security parity"). When
you add or raise a gate:

1. Update this document (the rule **and** the matrix) in the same change.
2. Apply it to **every** existing platform in that change, not platform-by-platform later.
3. A new platform implements **every** gate before its reading view ships.

When a platform's mechanism for a gate differs (as the interceptors do), that's fine, but the
**outcome** in every column must be the same, and any shortfall goes under "Known gaps" with a
follow-up, never left silent.
