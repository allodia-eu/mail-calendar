# Email signatures: cross-platform contract

**Scope.** What a signature *is* in Allodia Mail & Calendar, where it is stored, which one a
composer opens with, and what happens to it on the way out. The point is that a signature is one
thing the user writes once: a support answer like "Settings → Signatures → For replies or forwards"
must be true on every platform, and a message sent from macOS must carry the same signature, in the
same shape, as the same account sending from Android.

**Principle.** A signature is a **standalone, reusable entity**, not a per-account string. It is
authored once, lives in a named library, and any account may point at it. The core owns the
library, the assignment, and the security gates; each client renders them and formats its own copy
(localisation is client-side).

## The model

- **A library of named signatures.** Create, rename, edit and delete them in one place. Deleting one
  removes it from every account that used it (see "No dangling assignment" below).
- **Two slots per account, independently set**: **For new messages** and **For replies or forwards**.
  Each is a signature or **None**.
  - A reply, a reply-all and a forward share one slot. All three continue an existing message, and
    splitting them produces a setting nobody sets. This is Outlook's grouping and what people expect.
  - **There is no separate "signatures on" switch.** *None in both slots* already says "this account
    sends no signature", and a second control that could disagree with the pickers is a bug waiting
    to happen.
- **Seeded, then editable.** The composer opens with the resolved signature already in the body,
  where the user may trim or extend it before sending: Outlook's model, not Apple Mail's untouchable
  block.
- **It follows the sender.** Changing the composer's **From** account re-resolves the signature, so a
  work signature never goes out under a personal address. This is the failure the setting exists to
  prevent, so it is automatic rather than a reminder.
- **Per-message override.** The composer offers a **Signature** control listing the library plus
  **None**, so one message can carry a different signature, or none, without touching the account's
  default. It belongs in the composer's **action bar** (with Send, Discard and Attach), not among the
  From/To/Cc fields: it is an action you take on the message, not a field you address it with. The
  current choice shows as a checkmark inside the menu rather than on the button, so the bar stays a
  row of verbs.
  **An explicit choice survives a From change**: the user picked it *for this message*, and silently
  replacing it would undo a deliberate act. (Outlook re-swaps regardless; it is its most complained-
  about composer behaviour.)
- **Rich content, including an inline image.** A signature may carry formatting, links and an
  embedded logo. The logo is stored inline and sent as a `cid:` part (below).

## Storage

Two files, in the app data directory, for one reason:

| What | Where | Why there |
|---|---|---|
| The library (names + bodies + order) | **`signatures.toml`** | A signature body carries its images inline as base64, so it is the largest thing the app persists. Every preference write is a read-modify-write of the whole file: if bodies lived in `preferences.toml`, toggling a swipe action would rewrite a logo's bytes. |
| Each account's two assignments | `preferences.toml` (`signature_assignments`) | A small per-account pointer, which is exactly what the other per-account preferences are. |

- A signature's id is **opaque CSPRNG output**, never derived from its name: renaming must not break
  the accounts pointing at it.
- The order the user arranged the list in is stored separately from the map: a `BTreeMap` sorts by
  the (random) id, which is no order at all to a reader. An entry missing from the order still
  lists, so a hand-edited file can never hide a signature the user can no longer delete.

## Security

Both of these run in the shared core, so they hold on every platform whatever a client's editor
emits. See [`composer-security.md`](composer-security.md), Gate 10.

1. **Sanitised on store, and again on submit.** A stored body is sanitised when it is saved, because
   a client assigns it into the composer's editor with `innerHTML` and that page's CSP permits inline
   handlers, and because `signatures.toml` is a plain file anything with disk access can edit. It is
   re-sanitised on submit, because the editor round-trips it and is not trusted.
2. **`data:` images become `cid:` parts on send.** A signature stores its images inline as `data:`
   URIs; right for the library (one self-contained file), wrong for the wire: **Outlook's reader
   blocks `data:` images**, so a logo sent that way is an empty box for a large share of recipients.
   On send the core rewrites each to a `cid:` reference and attaches the bytes as a
   `multipart/related` part with a **minted** `Content-ID`. (A quoted original's images take the same
   route but *keep* the sender's original ids: they were already MIME parts; a signature's bytes
   never were.) A `data:` URI that cannot be safely decoded, or is not an `image/*`, is left
   untouched rather than dropped: losing the user's logo is worse than an interoperability shortfall.
3. **Never logged.** A signature body is user content: a name, a phone number, a logo. The core logs
   counts, ids and lengths only ([`logging.md`](logging.md)); `StoredSignature`'s `Debug` prints
   lengths for exactly this reason, and that is pinned by a test.
4. **No remote fetch.** Images are inert `data:`/`cid:` only. The composer's no-network-egress gate
   is unchanged.

## What goes on the wire

- **HTML**: the body verbatim inside `<div class="allodia-signature">`, in document order, so a
  reply reads *message → signature → quoted original*, the Outlook default.
- **Plain text**: the RFC 3676 separator line: `--` followed by **a space**, then the signature's
  plain-text rendering. The trailing space is significant (readers and list software key
  trailing-signature detection off it) and is pinned by a test.
- **An images-only signature still emits the delimiter**: it marks where the message ended, which a
  text part that merely stops does not.

## No dangling assignment

Three teardown paths, all in the core, so no client can forget one:

- **Deleting a signature** clears it from every account slot that pointed at it, across accounts.
- **Removing an account** drops its assignment: a later re-add starts with no signature rather than
  inheriting a pointer to something the user may have deleted meanwhile. The **library is untouched**:
  signatures are standalone, and another account may be using the same one.
- **Assigning an id that names nothing** (a host racing a delete, a stale picker) clears the slot
  instead of storing a pointer that resolves to nothing.

## Per-platform matrix

Every row is the same contract; only the presentation differs.

| Capability | Shared core | macOS | iOS/iPadOS | Windows | Android | Linux |
|---|:---:|:---:|:---:|:---:|:---:|:---:|
| Library CRUD (create / rename / edit body / delete) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Per-account **New messages** / **Replies or forwards** slots (each incl. None) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Rich body authoring (formatting, lists, tables, links) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Inline image in a signature (embedded logo) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Seeded into the composer, editable before send | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Auto-swap when the composer's From account changes | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Per-message override picker (incl. None), explicit choice sticks | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Re-sanitised on submit (Gate 10) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `data:` → `cid:` rewrite on send | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |

iOS/iPadOS is ✅ because the Apple client is one multiplatform target: the Settings category, the
signature editor and the composer are literally the same SwiftUI code as macOS, and only the native
file picker differs (`UIDocumentPicker` in place of `NSOpenPanel`). See Known gaps for exactly which
cells were driven on a simulator and which are ✅ by shared code.

Android renders the same contract in its own idiom: the settings hub gains a **Signatures** row, the
editor is a full-screen dialog (a rich-text body in an alert box would be a few lines tall), and the
per-message picker is an **app-bar action**: the app bar *is* this platform's action bar, which is
where the control belongs.

Windows likewise: the Signatures category sits in the Settings dialog's source list, and the body
editor renders **inside that dialog's detail panel** rather than in a window of its own, because
WinUI forbids a nested `ContentDialog`: the same constraint that already makes the destructive
database reset confirm in place, and the reason deleting a signature confirms inline too. The
per-message picker is a `DropDownButton` in the composer's action row, beside Attach: things you do
*to* the message, as against the From/To/Cc fields you address it with. Its entries are
`RadioMenuFlyoutItem`s: one choice from a set, drawn with the platform's own checkmark and
announced as "selected" to a screen reader, where a check glyph on a plain item would be decoration
nobody using one could perceive.

Linux renders it in the GTK/libadwaita idiom: the Signatures category sits in the Settings
window's sidebar, and the body editor replaces that window's detail inside a `GtkStack`. It does
not create a second toplevel: GTK's AT-SPI bridge can enumerate a newly visible toplevel before its
accessible object path exists. Each slot is an `AdwComboRow` rather than a row with a
`GtkDropDown` in it, because a `GtkDropDown` is labelled by its **selected item** through a
relation, and by the ARIA rules GTK follows a relation beats an explicit label: an account's two
pickers would both announce "None, combo box" with nothing to tell them apart. The per-message
picker is a `GtkMenuButton` beside Attach in the composer's action row, and its entries target one
stateful action, so GTK draws the current choice with its own radio mark and announces it as
selected, the same reasoning as Windows's `RadioMenuFlyoutItem`.

The last two rows are ✅ everywhere because they run in the shared use-case layer: any client that
submits a `Block::Signature` gets them, including one that cannot yet *author* a signature.

## Client seams

The shared editor bundle ([`clients/composer/dist/editor.html`](../clients/composer/dist/editor.html)) carries
both halves, so a new platform wires functions rather than inventing behaviour:

| Seam | Used by | What it does |
|---|---|---|
| `setComposerSignature(signature \| null)` | the composer | Inserts, replaces, or removes **this message's** `.allodia-signature` region, the one that is a **direct child of the editor** (see below). Touches only that region: the user's typed text, their trimming of the quote, and the caret stay where they are. Placement is decided on first insert (above the quote if there is one, else at the end) and reused on every swap, so the signature does not hop around. |
| `setSignatureBody(html, placeholder)` / `signatureBody()` / `insertSignatureImage(…)` | the Settings editor | The same bundle hosted body-only: load a signature, read back `{ body_html, body_plain }`, and insert an image as a `data:` URI. Deliberately **not** the composer's `addComposerInlineImage`, which mints an attachment id backed by a host blob handle: a stored signature has no way to keep one. |

**Both hosts get the composer's gates, from one definition.** The Settings signature editor loads
the same bundle as the composer, so it must carry the same WebView hardening: authoring a signature
*is* authoring mail content. Two hosts with two copies of those settings is two chances for one to
drift, so each client collapses them into a single definition its two hosts call: Android's
`EditorWebView.kt`, Windows's `EditorWebViewHost` (`Services/EditorWebView.cs`), Linux's
`SecureWebView` (`ui/webview.rs`, `DocumentKind::Composer`). A new client should do the same rather
than repeat the gate list. The **labels** the bundle's own chrome draws are one definition too, for
the same reason: a host that sends a partial map leaves those controls in English with nothing to
say so (`scripts/ci/check_composer_labels.py`).

**A quoted original can contain a signature too, and the composer must never touch it.** Our own
outgoing mail wraps the sender's signature in `.allodia-signature` (the Rust renderer emits that
class), so replying to a message we sent puts a *second* one inside the quote's `.aq-body`. A plain
`querySelector(".allodia-signature")` finds the **quoted** one first (it comes earlier in document
order once the quote is seeded), so every seed and every swap would rewrite the original author's
signature instead of the reply's, destroying quoted content the user never touched. The rule that
separates them exactly: **this message's signature is always a direct child of the editor, a quoted
one is always nested inside `.allodia-quote`**, so every lookup is scoped to direct children. A new
client wiring these seams must honour this; it is invisible until someone replies to mail sent from
this app, and then it silently eats the quote.

**A click in the editor's empty area below everything continues the *message*, never the signature.**
This is load-bearing, not polish: the signature is the last block, so `contenteditable`'s default
puts the caret inside it for every click in the large blank area underneath. The user then types
their message *into* their signature, and the next swap replaces that region and takes their text
with it: silent data loss from the most natural click in the composer. The shared editor intercepts
it (only when a signature is present, so behaviour without one is unchanged).

## Known gaps

- **What was verified where.** On **macOS**, against the seeded Stalwart harness: authoring a
  signature with an embedded logo, assigning both slots, the composer seed, editing it before
  sending, the override picker (including None and back), auto-swap in **both** directions across
  two accounts, an explicit choice surviving a From change, a reply placing the signature above the
  quote, and the delivered message's MIME (`multipart/related`, a `cid:` reference, the inline PNG
  part carrying the logo's exact bytes, and the `-- ` delimiter in the text part). On the **iPhone
  simulator**: the Settings category and its slot in the hub, library create/edit/delete, the
  per-account pickers, and the composer seed + picker. Two Apple cells are ✅ by **shared code
  rather than a simulator run**: the inline-image picker (iOS uses `UIDocumentPicker`, which the
  simulator tooling cannot drive to a file) and auto-swap (the simulator boot has one account). The
  **iPad**'s two-pane settings split was not separately driven; it is the same
  `SettingsCategoryDetail` the other eleven categories already render there.
  On an **Android emulator**, against the same harness: writing a signature with an embedded logo
  through the document picker, both slot assignments (and what the core persisted: the library in
  `signatures.toml`, the pointers in `preferences.toml`), the composer seed, the override picker
  both ways, auto-swap in **both** directions across two accounts, an explicit choice surviving a
  From round-trip, the caret rule (a tap in the blank area below the signature continued the
  *message*), and the delivered MIME over JMAP: `multipart/related`, `cid:sig0.…@allodia.local`,
  the inline `image/png` part carrying the logo's exact bytes, and the `-- ` delimiter.
  On **Windows**, against the same harness booted as two accounts (`--account stalwart-multi`):
  the category in its taxonomy slot and the library's empty state; writing a signature and what the
  core persisted (the body in `signatures.toml`, the two pointers in `preferences.toml`); editing
  one **in place** (same id, so it is an update rather than a second signature); adding a logo
  through the file picker and confirming the stored `data:` URI decodes to the picked file's exact
  bytes; assigning both slots on both accounts; the composer seed; the override picker both ways;
  auto-swap in **both** directions; an explicit choice, `None` **and** named, surviving a From
  change; a reply placing the signature above the quote; **replying to a message this app sent**,
  where both signatures survive and the quoted one is untouched (the `e516a16` bug, absent); the
  caret rule; deleting a signature clearing the slots that pointed at it while another account's
  assignment stayed; and the delivered MIME over JMAP: `multipart/related`, a minted
  `cid:sig0.…@allodia.local`, **no `data:` image left in the HTML**, the inline `image/png`
  carrying the logo's exact bytes, and the `-- ` delimiter.
  On **Linux**, confirmed end to end **against a real account** (2026-08-20) and, for the detail
  below, against the same harness: the category in its taxonomy slot and the library's empty
  state; writing a signature and what the core persisted (the body in `signatures.toml`, sanitised,
  under an opaque id, with the order kept separately, and the two pointers in `preferences.toml`);
  adding a logo through the file picker and confirming the stored bytes; assigning both slots; the
  composer seed, with the empty lead line above it that a new message needs; the Signature control
  in the action row beside Attach; the override picker both ways (**None** emptied the body, the
  named signature put it back in the same place); a reply placing the signature above the quote;
  and the delivered MIME over JMAP: for a text signature `multipart/alternative` with
  `<div class="allodia-signature">` and a text part opening `-- ` (trailing space intact), and for
  the logo signature `multipart/related`, a minted `cid:sig0.…@allodia.local`, **no `data:` image
  left in the HTML**, an inline `image/png` byte-identical to the picked file, and the `-- `
  delimiter still emitted for a signature that is nothing but an image. Two cells are ✅ by
  **shared code rather than a driven run**: auto-swap (the harness seeds one account, so there is
  no second sender to change to) and delete-clears-every-assignment (a core teardown the core's own
  tests pin). Two traps found in the driving, worth knowing before writing another such test: the
  The packaged-runtime acceptance run keeps an AT-SPI client walking the tree through twelve
  consecutive edit/save cycles. Opening the composer's signature menu puts focus **on the first
  entry**, so keyboard-selecting index *N* takes *N* presses, not *N+1*, the same off-by-one
  Windows records below, which silently selects the next signature along.
  Two of those checks passed only after the *test* was fixed, and both are worth knowing before
  writing another: a synthetic click needs its process to be **per-monitor DPI-aware** or the
  coordinates are virtualized and it lands somewhere else entirely (here, inside the signature,
  which reads exactly like the caret rule failing); and opening the picker's flyout puts focus **on
  the first item**, so keyboard-selecting index *N* takes *N* presses, not *N+1*, or every
  assertion reports the next signature along.
- **No JS test for the editor seams, and it has now cost a real bug.** Replying to a message with
  no HTML part rendered an *empty* quote (`setComposerQuote` assigned `body_html` and never fell
  back to `body_plain`), so the original vanished from the composer. It survived this feature's own
  verification because the quoted text was still on the wire (the plain half rides into the
  outgoing `text/plain` part), so the delivered MIME, which is what was asserted on, looked
  correct. **A MIME check cannot see what the composer shows.** Fixed in 0.8.1; when a JS runner
  arrives, that case is the first test to write.
- **No JS test for the editor seams.** The repo has no JavaScript test runner, and Robolectric has no
  renderer, so `setComposerSignature`, the caret rule above, and the signature editor's three seams
  are verified by driving a real client (macOS and Android, `scripts/dev/control.sh`; Windows, UI
  Automation via `clients/windows/uia.ps1`) rather than by a unit test. What *is* unit-tested either side of them: the Rust halves: the block, the render,
  the sanitise, the `data:`→`cid:` rewrite, the CRUD and the resolution, plus, on each client, the
  slot rule, the resolution precedence, the seed payload, (Android) the order of the
  page-finished script batch, which is what puts the signature above the quote, and (Windows) the
  same three rules plus the image cap and media-type refusal (`Mailcal.Tests`).
- **GTK's AT-SPI toplevel race remains upstream; Linux no longer enters it.** The old signature
  editor was a modal toplevel. While assistive technology enumerated visible toplevels, GTK could
  expose that window before or after its accessibility context had an object path and crash in
  `g_variant_builder_add_value`. GTK 4.14.5 and the shipped 4.22.4 runtime both reproduced it within
  one to three edit/save cycles. The editor now changes detail inside the existing Settings
  toplevel, so that object-path transition no longer exists. `test-linux-ui.sh` holds an AT-SPI
  session open through twelve consecutive cycles on GTK 4.22.4; the original implementation fails
  that run.
- **Embedded images are capped at 512 KB each, client-side.** A signature rides in *every* message an
  account sends, and base64 adds a third on top. The cap is enforced where the file is picked (so the
  user is told), not in the core.
- **No signature reordering UI.** The stored order is honoured; nothing yet lets the user change it.

## Enforcement

This contract is binding via [`../AGENTS.md`](../AGENTS.md). When you change how signatures are
stored, resolved, seeded, or sent:

1. Update this document (**the rule and the matrix**) in the same change.
2. Apply the change to **every** platform that has the surface, or record the shortfall under Known
   gaps, never silently.
3. A user-facing change also updates [`../README.md`](../README.md)'s capability matrix and adds
   a fragment under `docs/changelog/unreleased/` (every catalog locale, with its `Platforms:` and
   `Bump:`); see [`changelog.md`](changelog.md). It does **not** touch [`../VERSION`](../VERSION):
   only a release PR moves that.
4. Anything that widens what is stored or sent updates [`privacy-policy.md`](privacy-policy.md)
   **and both its locales**, with the version/date line bumped.
