# Rich composer security: cross-platform contract

**Scope.** How every Allodia Mail & Calendar client hosts the rich HTML composer. Unlike the
reading view, the composer must run editor JavaScript for selection, IME, undo, paste, table editing,
and image handling. That makes its boundary different: the editor document is trusted product code,
but pasted/imported HTML, file names, image metadata, and draft body text are still hostile input.

**Principle.** The composer may use platform WebViews, but the sendable body contract is shared Rust:
`mailcal-composer` validates the editor document and emits canonical HTML, `text/plain`, inline CID
parts, and regular attachment manifests. Native clients never invent their own send HTML.

## The layers

### Layer 1: Local editor bundle

The editor runtime is bundled with the app and loaded from app assets:

- No CDN scripts, stylesheets, fonts, images, or extension packages.
- No runtime package downloads.
- No network access from the composer document.
- The editor schema is the product schema: bold, italic, underline, constrained font sizes, **text
  and highlight colour**, bullets **nested to any depth**, tables, inline images, and regular
  attachments. Unsupported pasted constructs are dropped or normalised before they enter the shared
  document.
- **Colour is `#rrggbb` and nothing else.** The editor normalises what the DOM hands it (engines
  disagree: `#rgb`, `rgb()`, a legacy `<font color>`) and `mailcal_composer::TextColor` re-validates
  on submit, because the value is placed straight into a `style="…"` attribute. A value that fails is
  dropped, not rejected: the run renders in the inherited colour rather than the message refusing to
  send.

**The bundle is built, not hand-written.** Sources are TypeScript ESM modules under
[`clients/composer/src/`](../clients/composer/src); `bun run build` inlines them into the single
self-contained `clients/composer/dist/editor.html` every host loads. That artifact is committed (see its
header for why), and `bun run check`, wired into [`gate.sh`](../scripts/dev/gate.sh), fails when it
does not match its sources, so a client cannot ship a bundle nobody rebuilt. Every client's
`build-and-run` script rebuilds it first ([`composer-bundle.sh`](../scripts/dev/composer-bundle.sh)),
so a source edit reaches the app you launch rather than the one you last built.

### Layer 2: Shared Rust output contract · `crates/mailcal-composer`

Every client sends the editor document to Rust before mail submission:

- Rust validates attachment ids, inline-image references, rectangular tables, and required metadata.
- Rust emits a deterministic, self-contained HTML document (`<!DOCTYPE html>` with a charset/viewport
  head and inline styles, since mail clients strip `<head>`/`<style>`) plus deterministic plain text.
- Inline images are `cid:` references only; remote image URLs are not composer body content.
- Attachment-only images/files stay in the attachment manifest and are not rendered into the body.
- The output is a MIME-ready manifest, not byte content. Inline-image blobs remain behind
  host-owned handles until the host calls `submit_rich_mail` with resolved bytes. Regular native
  file attachments use `submit_rich_*_with_files`: hosts pass selected file paths/metadata and Rust
  reads the bytes locally while preparing the MIME draft, so file content does not cross FFI.

### Layer 3: Native WebView host gates

Every platform implements these gates for the composer WebView.

**Where the composer is mounted is not a gate.** macOS and Windows render it *inline in the reading
pane* (it replaces the reading view in the detail column); iPhone, iPad, and Android host it
full-screen/modal. That is a layout choice, and it moves none of the rules below: the composer
WebView is configured identically wherever it is mounted, and the gates travel with the view, not
with the window that held it. Two consequences worth stating, because they are easy to lose in a
port:

- The composer WebView is still a **separate** WebView from the reading view's (gate 1). Hosting
  them in the same slot does not merge them: the desktop clients tear the composer's editor down on
  Send/Cancel rather than reusing it across drafts, and unload the reading body while the composer
  has the column, so no message document sits loaded (or remote-content-enabled) behind a composer
  the user cannot see it through.
- Draft body text stays unlogged (gate 8) whichever host it is in. The "has the user written
  anything?" check behind the **discard prompt** compares the editor document against the seed
  it opened with **inside the client** and reports a single boolean; it never logs, stores, or ships
  the document. The prompt itself is not a security gate but a data-loss one, and what makes it
  *reachable* differs per host: on macOS, Windows and Linux the composer is an inline pane, so a
  click on another message could drop the draft; on **Android** the composer is full-screen, but the
  system **back button and back gesture** could (an edge swipe being easier to hit by accident than
  any click). iPhone and iPad have neither, which is why they alone still short-circuit it. Wherever
  it is raised the rule is identical: header fields compared against what the composer *opened*
  with (a pre-filled reply is not a draft), body compared against the seed captured after the quote
  and signature were injected and **before** the caret was moved into it.

1. **JavaScript enabled only for the local composer document.** It is never reused for untrusted mail
   rendering.
2. **No general host bridge.** Expose only explicit commands: get/set document JSON, pick file, commit
   blob handle, render/validate via Rust, and report editor state.
3. **No network egress.** Block `http(s)` and protocol-relative sub-resources. The composer cannot
   fetch remote images, fonts, CSS, scripts, or uploads directly.
4. **All top-level navigation blocked.** Link clicks and attempted redirects are cancelled.
5. **No new windows / popups.**
6. **No arbitrary file/content access from web code.** File picking is native; web code receives only
   the resulting attachment id/blob handle/metadata.
7. **Pasted HTML is sanitised before insertion.** The client/editor strips scripts, event handlers,
   frames, forms, remote resources, and unsupported CSS; Rust validation is the final authority.
8. **Draft body text is sensitive.** Do not log editor JSON, rendered HTML, plain text, or pasted
   content. Logs may include lengths, counts, ids, and validation error categories.
9. **External dispatches stay gated.** If a future feature fetches a remote image, uploads to a cloud
   attachment provider, or calls AI drafting, it passes the `JurisdictionGate` before data leaves.
10. **Raw-HTML body blocks are sanitised in the core: quoted originals *and* signatures.** Two blocks
    carry HTML the composer emits verbatim rather than building from nodes, and both are hostile input
    the editor round-trips:
    - A reply/forward may embed the original message's HTML as a quoted block (`mailcal-composer`'s
      `Block::Quote`). It is sanitised when the core seeds the quote **and re-sanitised on submit**.
    - A message may carry the sender's signature as a `Block::Signature`. It is sanitised **when the
      core stores it** (a client assigns it into the editor with `innerHTML`, and `signatures.toml` is
      a plain file anything with disk access can edit) **and re-sanitised on submit**.

    `rich_draft` runs the reading-view sanitiser over every quote body and every signature body before
    the document is rendered into the outgoing message. The editor is never trusted to keep either
    inert. Because this runs in the shared use-case layer, it holds on every platform regardless of
    what a client's editor emits, so the gate cannot be missed by a new client. See
    [`signatures.md`](signatures.md).
11. **The quote seed's `initial_text` is text, never markup.** `setComposerQuote` accepts an optional
    `initial_text` that pre-fills the lead paragraph above the quote (the showcase/screenshot mode is
    its only caller; see `docs/debugging.md`). The editor assigns it via `textContent`, so it is never
    parsed as HTML and opens no injection path the empty lead paragraph didn't already have. A client
    may pass only plain text here; anything richer must go through the sanitised quote body (Gate 10).
12. **A `mailto:` link pre-fills a composer; it never composes one.** The OS hands a client an opaque
    URI that came from a web page, a document, or any other app: hostile input by definition. It is
    decoded **in the shared core** (`mailcal_composer::mailto`, over the FFI as `parse_mailto_uri`), so
    every platform inherits one implementation of these rules rather than re-deriving them:
    - **Only `to`, `cc`, `bcc`, `subject`, and `body` are honoured.** RFC 6068 §6.1 warns that a URI may
      name *any* header; everything else is dropped silently. `from` is the one that matters most
      (a link must never dictate who a message appears to come from), followed by `reply-to` (redirects the
      answer), the threading headers (grafts the message onto someone else's conversation), and
      `content-type` (decides how the body is interpreted).
    - **Nothing is ever sent.** The result only pre-fills an editable composer that the user must still
      send themselves. There is no `mailto:` code path that puts mail on the wire, and no client may add
      one.
    - **A pre-filled `Cc`/`Bcc` must be visible.** `bcc` *is* one of the honoured fields, so a link can
      add a recipient, and every client collapses the Cc/Bcc row by default. Any client that pre-fills
      either opens that row, because a recipient the user cannot see is one they cannot remove. The rule
      is not the mail link's alone: a reply-all fills `Cc` and an assistant's draft may fill either, so
      each client decides it once, from the seeded strings, in a pure function its own suite drives
      (`RecipientTokens.RevealsCcBcc`, `revealsCcBcc`, `reveals_cc_bcc`).
    - **Header injection is closed at the parse.** The query is split into fields *before*
      percent-decoding (so an encoded `&` cannot introduce a `bcc` the user never saw), control
      characters are stripped from the subject, and an address containing CR/LF/angle brackets/quotes is
      discarded rather than repaired.
    - **The body is seeded as text.** It reaches the editor through `setPlainText`, which assigns one
      paragraph per line via `textContent`: the Gate 11 rule, applied to the other seeding path.
    - **The URI is never logged.** It is message content end to end (recipients, subject, body), so a
      client records only that a mail link arrived (Gate 8 again, and `docs/logging.md`).
13. **A share pre-fills a composer with files the core has named and typed.** Another application
    hands us files and text and asks for a mail client; both are hostile input, chosen by that app
    rather than by us or by the user. The decode is **in the shared core**
    (`mailcal_composer::share`, over the FFI as `prefill_from_share`), and the product half of the
    contract, which OS entry point each client registers and how it asks to become the default mail
    app, is [`os-integration.md`](os-integration.md). The security half:
    - **Attachments never come from a URI.** `mailto:?attach=` is not RFC 6068 and is never
      honoured: a handler cannot tell a URI that came from `xdg-email` from one that came from a web
      page, so honouring it would let a page attach any local file it can name. Files reach a
      composer only from a channel that is itself a user action (a share sheet, an "Open With", an
      explicit `--attach`).
    - **A share may not address a message.** It can suggest a subject and body text and nothing
      else. The only route to `To`/`Cc`/`Bcc` is shared *text* that is itself a mail link, decoded
      by Gate 12's allowlist, so there is no second parser with its own idea of what a link may say.
    - **Names and media types are normalised in the core**, by the same
      `mailcal_composer::{safe_file_name, safe_media_type}` the composer's own file picker uses. A
      name keeps only its final path component, loses control characters (a CR or LF would end the
      `Content-Disposition` line) and bidirectional overrides (`holiday<U+202E>gpj.exe` renders as
      `holidayexe.jpg`), and is capped through its stem so the extension survives. A declared media
      type is used only when well formed; `*/*` is not one.
    - **Bytes are staged by the host and read by Rust.** A client holding an OS handle copies it
      into its own private storage and passes a path; the core reads it at submit
      (`submit_rich_*_with_files`), so file content still does not cross FFI, exactly as for a
      picked file.
    - **Nothing is ever sent**, and nothing is dropped silently: a file the core refuses comes back
      with a reason, because one the user watched disappear from a share sheet is one they will
      assume was attached.
    - **Neither the files nor the text is logged.** Names are the user's own; counts only (Gate 8).

## Per-platform implementation matrix

Every row is mandatory on every column. A new platform may not ship the rich composer until every
cell is filled.

The "editor chrome localised" row is the one whose failure is invisible: the bundle ships English
defaults, so a host that sends nothing, sends a key the bundle does not know, or omits one it does,
just shows English for that control. `scripts/ci/check_composer_labels.py` holds it: every client
must send exactly the keys `clients/composer/src/labels.ts` declares, and must actually call the
hook. Add a toolbar control and the label goes in all four clients in the same change.

| Gate | Apple · `WKWebView` | Android · `WebView` | Windows · `WebView2` | Linux · `WebKitGTK` |
|---|---|---|---|---|
| Host (layout only, not a gate) | macOS: inline in the detail column, in place of the reading pane. iPhone/iPad: full-screen cover | full-screen composer screen | inline in the reading-pane slot, behind the list\|reading splitter | inline in the detail column, in place of the reading pane |
| Editor torn down per draft | SwiftUI drops the composer view (and its `WKWebView`) when `compose` clears | composer screen is popped | `ComposerView` is built per request and its `WebView2` closed on Send/Cancel | `ComposerPane` builds a fresh `SecureWebView` per request and removes it on Send/Cancel |
| Reading body not left loaded behind the composer | the reading view is out of the view tree while composing | composer is a separate screen | `ReadingView.SuspendBody()` unloads the message document; it re-renders on return | `ReadingPane::suspend` replaces the message with an empty filtered document; it re-renders on return |
| Local editor assets only | load packaged bundle with an app-owned base URL | load packaged `android_asset` bundle | load packaged app asset / virtual host mapping | shared `clients/composer/dist/editor.html` is compiled with `include_str!` and loaded in memory |
| JS only for composer | separate composer `WKWebView`; never used by reading view | separate composer `WebView`; reading view keeps JS off | separate composer `WebView2`; reading view keeps JS off | separate composer `WebView` has JS on; dedicated reading `WebView` keeps it off |
| Narrow bridge | no bridge for the initial body-formatting editor; future file/image commands must be named `WKScriptMessageHandler`s | no bridge for the initial body-formatting editor; future file/image commands must be a minimal `addJavascriptInterface` object | no bridge/host objects for the initial body-formatting editor; future file/image commands must use an explicit `WebMessageReceived` protocol | fresh `UserContentManager` with no script-message handlers or host objects |
| No network egress | `WKContentRuleList` blocks remote loads | `shouldInterceptRequest` blocks remote loads | `WebResourceRequested` returns 403 for remote loads | native `UserContentFilter` blocks every `http(s)` request |
| Navigation blocked | navigation delegate cancels every non-initial navigation | `shouldOverrideUrlLoading` returns true | `NavigationStarting` cancels non-initial navigation | `decide-policy` permits only the expected initial `about:blank` load |
| New windows blocked | no popup-opening `WKUIDelegate` path | no popup path; navigation cancelled | `NewWindowRequested.Handled = true` | `create → None`; `NewWindowAction` is ignored |
| No arbitrary file/content access | file picking via native panel/picker only; no web file URLs; Rust reads selected paths on submit | `allowFileAccess = false`, `allowContentAccess = false`; native picker stages content to app cache; Rust reads staged paths on submit | no arbitrary file access; native picker only; Rust reads selected paths on submit | file/universal access disabled; native `GtkFileDialog` paths are passed to Rust only on submit |
| Paste/import sanitisation | editor paste rules plus Rust validation | editor paste rules plus Rust validation | editor paste rules plus Rust validation | shared editor paste rules plus Rust validation |
| Editor chrome localised | shared catalog via `setComposerLabels` (`ComposerLabels.swift`), in the composer **and** the signature editor | shared catalog via `setComposerLabels` | shared catalog via `setComposerLabels` (`ComposerLabels.cs`), in the composer **and** the signature editor | shared catalog via `setComposerLabels` |
| Rust canonical output | call `submit_rich_*_with_files` for regular file attachments; use `render_composer_document_json` for preview when needed | call `submit_rich_*_with_files` for regular file attachments; use `render_composer_document_json` for preview when needed | call `submit_rich_*_with_files` for regular file attachments; use `render_composer_document_json` for preview when needed | calls `submit_rich_*_with_files`; selected files are native metadata, never WebKit uploads |
| No body-content logging | lengths/counts only | lengths/counts only | lengths/counts only | no composer body is logged |
| Quoted-original sanitisation | shared core re-sanitises every quote body on submit (`rich_draft`); the host editor is never trusted | (same: shared core) | (same: shared core) | same shared-core `rich_draft` re-sanitisation |
| Signature sanitisation + `data:`→`cid:` | shared core sanitises on store **and** on submit, then rewrites inline `data:` images to `cid:` parts | (same: shared core) | (same: shared core) | (same: shared core) |
| `initial_text` is plain text | host passes only `showcase_reply(locale).text`; the shared editor assigns it as `textContent` | (same: shared editor) | (same: shared editor) | (same: shared editor) |
| `mailto:` link decoded by the core, never the client | `CFBundleURLTypes` scheme `mailto` → the shell's `onOpenURL` → `parseMailtoUri`. No scheme gate is needed: the OAuth redirects are captured inside their own `ASWebAuthenticationSession` and never reach it. **macOS only in practice**: iOS routes a mail link to the *default* mail app alone, which needs an Apple-granted entitlement ([`os-integration.md`](os-integration.md)) | `ACTION_VIEW` + `ACTION_SENDTO` on scheme `mailto` → `parseMailtoUri` (`MailtoLaunch` gates action + scheme so an OAuth redirect is never mistaken for a link) | MSIX `windows.protocol` `mailto` → `ParseMailtoUri` (`MailLink` gates the scheme, for the same reason; the URI reaches the core as `OriginalString`, still percent-encoded) | desktop `MimeType=x-scheme-handler/mailto` + raw GApplication command-line activation → `parse_mailto_uri`; cold and redirected warm activations share one broker path |
| `Cc`/`Bcc` collapsed by default, revealed when pre-filled | `revealsCcBcc(cc:bcc:)` seeds `showsCcBcc`; held by `RecipientTokenTests`, and on a mail link's own shape by `MailLinkTests` | `revealsCcBcc(cc, bcc)` opens the row; held by a JVM test | `RecipientTokens.RevealsCcBcc` sets the chevron's `IsChecked`; the rule is held by `Mailcal.Tests` and the collapse itself by a UI test that asks the running window for the fields, plus the mail-link suite that reads the pre-filled pills off it | `reveals_cc_bcc` sets the chevron; held by the crate's GTK test, which reads the row's visibility off the widget |
| `mailto:` body seeded as text | `initialBody` → the shared editor's `setPlainText`, as `textContent` | `window.setPlainText` (shared editor) assigns one paragraph per line via `textContent` | (same: shared editor, the same call the assistant-draft path uses) | (same: shared editor; the body is JSON-encoded data, never script) |
| Shared files named and typed by the core (Gate 13) | ⬜ no share target registered | `ACTION_SEND` / `ACTION_SEND_MULTIPLE` on `*/*` → `prefillFromShare`; `ShareLaunch` gates the action, the bytes are copied out of the sender's provider into app cache first | MSIX `windows.shareTarget` (any file type, plus Text and WebLink) → `PrefillFromShare`; the shared bytes are staged out of the `ShareOperation` before it reports complete, since its access ends there | desktop `MimeType=` + `%U` and `--attach` → `prefill_from_share`; the composer opens holding `ComposeRequest::files` |

## Known gaps / follow-ups

- **The single-scroll editor chrome is Android-only.** `useNativeComposerChrome`
  switches the bundle to page-scroll with the toolbar pinned to the bottom and the address header
  scrolling away, designed for a phone, where the keyboard takes half the screen. A resizable
  desktop window has room to keep the header pinned, so macOS, Windows and Linux deliberately keep
  the flex/inner-scroll layout. Revisit only if a desktop composer starts feeling cramped; porting
  it is host work (a header overlay synced to the WebView scroll), not a bundle change.
- **Pasted text stays plain text.** Formatting from Word, Outlook or a browser is dropped on paste,
  which is the strict reading of Gate 7 rather than an oversight. Mapping pasted HTML onto the closed
  document schema is its own piece of work.
- **Colour cannot be cleared from a partial selection.** "Automatic" / "No highlight" clears the mark
  from every element the selection touches, so selecting half a coloured run clears all of it.
  Removing a mark from part of a run means splitting it, which is what `execCommand` exists to do and
  which has no command for *removing* one mark.

- **iOS/iPadOS composer newly ported (Apple multiplatform migration).** It hosts the **same**
  hardened editor `WKWebView` as macOS (JS for the composer only, navigation + new windows blocked, Rust
  canonical output via `submit_rich_*`, no body-content logging); only native file picking differs:
  `UIDocumentPicker` in place of `NSOpenPanel`. **Build-green and real-account verified on the
  iPhone/iPad simulators;** background delivery remains the coordinated iOS/Android follow-up before
  iOS/iPadOS ship to users.
- Apple, Android, and Windows host the shared local editor bundle for new message, **reply, and
  forward** body formatting; all three open the **same** hardened editor host (no new surface) and
  call `submit_rich_mail` / `submit_rich_reply` / `submit_rich_forward` with the editor document; the
  Rust use-case layer derives the reply recipient/`Re:` subject/threading and the forward `Fwd:`
  subject.
- **Auto-quoting the original** into the reply/forward body now ships on **all three clients** (the
  reading-view reply/forward seeds a `Block::Quote` with the original's already-sanitised body and
  either an indented one-line attribution or a line-and-header block, chosen by a persisted default
  and, when the user opts into it in Settings, overridable per message via a composer toggle).
  The quote body is sanitised at seed and re-sanitised on submit (Gate 10): the
  re-sanitisation runs in the shared core, so it is enforced identically on Apple, Windows, and
  Android. macOS is runtime-confirmed; iOS/iPadOS, Windows, and Android are code-complete pending a
  runtime smoke test. A quoted original's inline `cid:` images now render: the reading-view body the
  quote seeds from already has its `cid:` references resolved to inline `data:` URIs
  ([`docs/rendering-security.md`](rendering-security.md) Layer 1), and the submit-time re-sanitizer
  (Gate 10) preserves `data:` images. On submit the shared core then turns each quoted `data:` image
  back into a `cid:` reference to a re-attached `multipart/related` part, **preserving the original
  inbound `Content-ID`** (`html::restore_cid_images` + `reattach_quote_cids`, keyed on the engine's
  inbound `InlinePart`s via `Engine::message_inline_parts`). This matches what Outlook/Thunderbird
  emit and what an Outlook reader renders (its reader blocks `data:` images but renders `cid:`), and
  keeping the original ids lets long-thread spam filters reuse a part's known hash instead of
  re-scanning it. The rewrite is shared-core, so it holds identically on Apple, Windows, and Android.
- **Signatures** ([`signatures.md`](signatures.md)) add a second raw-HTML block, `Block::Signature`,
  under Gate 10. It is sanitised **on store as well as on submit**: a client seeds it into the
  editor with `innerHTML`, and the editor page's CSP permits inline handlers, so the library must
  only ever hold inert HTML. On send the core rewrites the signature's inline `data:` images to
  `cid:` `multipart/related` parts with **minted** Content-IDs (the quote path preserves the
  sender's original ids instead, since those bytes were already MIME parts). The Settings signature
  editor hosts the **same** bundle with the **same** WebView gates (authoring a signature is
  authoring mail content) and adds three body-only seams (`setSignatureBody` / `signatureBody` /
  `insertSignatureImage`). The composer's seam is `setComposerSignature`, which replaces only **this message's**
  `.allodia-signature` region, the one that is a direct child of the editor, never a quoted
  original's (see `signatures.md`). Both gates are shared-core, so they hold on every platform; the
  authoring surface ships on Apple (macOS + iOS/iPadOS), Android and Windows today (see that doc's
  Known gaps). Android hosts it through the same `applyEditorSecuritySettings` +
  `EditorWebViewClient` the composer uses, and Windows through the same `EditorWebViewHost`
  (`Services/EditorWebView.cs`), one definition per client, so the two hosts cannot drift. On
  Windows that was a deliberate refactor: the gates had been inlined in `ComposerView.xaml.cs`, and
  a second host would have meant a second copy of them.
- Native regular-file pickers and byte wiring now ship on Apple, Android, and Windows:
  `submit_rich_*_with_files` validates the document, appends the selected files as regular MIME
  attachments, and reads bytes in Rust from host-selected/staged paths. Inline image insertion
  remains a follow-up; the editor schema and original `ComposerBlob` path still support inline CID
  images, but clients do not yet expose an inline-image picker.
- **`mailto:` handling ships on macOS, Windows, Android and Linux (Gate 12); iOS/iPadOS is
  registered but gated.** Linux registers
  `MimeType=x-scheme-handler/mailto` in its generated desktop entry and handles both a cold launch
  and a URI forwarded to the existing GApplication instance. A link received before the first
  account is kept through setup; a link arriving over Settings, Calendar or Contacts brings the
  mail composer to the front. Apple registers `CFBundleURLTypes` and takes the link at the
  shell's `onOpenURL`, holding one that arrives before the first account until there is somewhere
  to send it from. That path is live on macOS and inert on iOS/iPadOS, which hands `mailto:` to the
  *default* mail app and nothing else: until Apple grants
  `com.apple.developer.mail-client`, the app cannot be that, so `onOpenURL` never fires there. The
  declaration is the prerequisite, kept so the entitlement is the only thing left to add. Being
  *offered* as a handler is not being chosen as one either: what each platform can do about that is
  [`os-integration.md`](os-integration.md).
- **A link arriving over an open draft is answered differently across the shipped platforms, and Android's
  answer is the one to revisit.** Windows asks: the same "Discard draft? / Keep editing" prompt an
  assistant's draft raises, which is the identical situation: an unprompted request to open a
  prefilled composer. Linux asks through the same guard as message navigation. Android drops the
  link and leaves the draft alone. That divergence is a
  leftover, not a decision: Android's drop was chosen when it had no discard prompt to hang the
  question on, and it has had one since the composer's back-button guard landed. Neither behaviour
  can lose written work, which is why this is a follow-up rather than a defect, but a click that
  silently does nothing is the weaker of the two, so Android should adopt the prompt.
- **Sharing *into* the app (Gate 13) ships on Android, Linux and Windows; Apple is ⬜.** `prefill_from_share` decodes a share,
  names and types its files and reports what it refused, with suites in `mailcal-composer`,
  `mailcal-bindings`, the Linux crate, the Android JVM suite and `Mailcal.Tests`. What Apple
  still needs is a Share Extension target and an `initialAttachments` seed on its composer, the
  structural change the other three have made: without it a composer's attachment list can only be
  filled by its own picker. [`os-integration.md`](os-integration.md) has the per-platform state.
- Linux hosts the same editor inline in its detail column for new mail, reply, reply-all,
  and forward, with native attachment picking and the shared Rust submission methods. The
  optional per-message quote-style picker is still outstanding; until then Linux seeds the style
  chosen in Settings → Composing.

## Enforcement

This contract is binding via [`../AGENTS.md`](../AGENTS.md) ("Cross-platform security parity"). When
you add or raise a composer gate:

1. Update this document (the rule **and** the matrix) in the same change.
2. Apply it to **every** existing platform in that change.
3. Log any unavoidable shortfall under "Known gaps" with a clear follow-up.
