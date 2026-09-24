# Reading-view actions: cross-platform contract

**Scope.** The action row above an open message: which actions sit on it as buttons, which sit
behind the **overflow menu** at its end, and what exporting a message writes. It binds every
client that draws a reading view.

**Principle.** The row carries the actions someone reaches for on most messages; the overflow
carries the ones a smaller number of people reach for often. Both are the same actions on every
platform, in the same order, because "the menu at the end of the row" has to mean one thing when
a user moves between their phone and their desk.

## The row

Reply, reply-all and forward at the **start**; archive and delete at the **end**; the overflow
button **last of all**, after both of them. Nothing is added between the mailbox actions and the
overflow: it is the end of the row on every platform, which is what makes it findable without a
label. (Which of archive and delete comes first is each platform's own; only the overflow's place
is fixed here.)

Labels collapse to icons on a narrow pane ([`../clients/apple/Packages/MailcalKit/Sources/MailcalUI/ReadingView.swift`](../clients/apple/Packages/MailcalKit/Sources/MailcalUI/ReadingView.swift)
and its Windows twin measure this); the overflow button is an icon at every width and carries the
same accessible name, `a11y_more_actions`, everywhere.

**The row fits the pane, whatever that costs it.** Labels go first, then the control size itself:
six touch-sized icon buttons are wider than any phone, and a row that outgrows its pane is not
clipped on its own, it *sets* the pane's width, so the header above it and the message below it
hang off both edges of the screen. Measuring is what decides, never a width written down here or
in a client: a longer language, a larger text size and a narrower phone each move the answer. What
may not move is the target a finger gets, so a smaller control stays a 44-pt button.

**The row stays put; the rest of the header does not.** Who sent the message, who else got it,
what it carries and any banner over it belong to the *message*, so wherever a client can do it they
scroll away with the body and the action row alone stays on screen. A reader who has scrolled to
the end of a newsletter is exactly the reader who wants to archive or reply to it, and having to
scroll back up first is a cost paid on every message to save one line of chrome.

Where a client cannot do it the header stands still above the body, which is the fallback, not a
second design; each such client says so in the matrix below.

**It is the same control as the buttons beside it**, at both of the row's widths. Each platform
uses whatever gets it there: Windows and Android hang a native menu off a button, and Apple builds
an ordinary button that presents a popover, because a SwiftUI `Menu` cannot be made to match a
bordered button's height and breaks the row's width measurement
([`client-traps.md`](client-traps.md)). What the contract fixes is the result, not the mechanism:
a reader should not be able to tell the overflow apart from its neighbours except by its glyph.

## What may go in the overflow

- **Actions on the message as a document**, rather than on its place in the mailbox: exporting it
  and printing it, and later reading its source.
- **Never a duplicate** of a button already on the row. A user who cannot find archive does not
  need a second archive; they need a wider pane, which the icon collapse already gives them.
- **Never something irreversible.** Delete stays a button someone can see before they press it.
  An action that cannot be undone does not belong behind a menu that has to be opened first.

The menu is present on every platform including phones, where it is the same icon at the end of
the same row. It is not a desktop-only affordance: the actions behind it are no less useful on a
phone, and a menu costs one icon.

## Exporting a message

**The file is the message that arrived, byte for byte.** The core writes the raw RFC 5322 source
the engine cached (`Engine::message_source`), never a document assembled from the reading view.
The reading view is a *rendering*: its HTML has been sanitised, its inline images rewritten to
`data:` URIs, its parts decoded. A file built from that would open in another mail client as a
plausible message that is not the one the sender sent, and no signature over the original would
still verify.

**The name comes from the core**, through `message_export_file_name(subject)`. A client passes the
subject **it displays**, so an untitled message exports under that client's own "(no subject)"
rather than a second, English name. The core flattens path separators, drops control and
bidirectional characters, replaces the punctuation a filesystem reserves, bounds the length and
appends `.eml`. No client writes its own version of that: a subject is free text, and five answers
to "what is a safe file name" are five different files, some of them written somewhere nobody
chose.

**The host chooses where.** A save panel on the desktops, the share sheet on iPhone, iPad and
Android, exactly as saving an attachment already works. The bytes never cross the FFI; the host
hands down a path and Rust writes it.

**A failure is always reported**, with `message_save_failed`, in whatever way that client already
reports an attachment save. A message that could not be exported never reads as one that was, and
a half-written file is removed rather than left as a damaged message. Success is announced only
where that client announces an attachment save (`message_saved`, on Android and Linux); on the
other three the file appearing where the user put it is the confirmation.

**It is not an `Intent`.** Exporting moves nothing, marks nothing and syncs nothing: it is a read
plus a host file write, so it is a direct FFI call like `save_attachment`, and the
dispatch → snapshot loop never sees it.

⚠️ **Fetches when the source is not cached**, so every client calls it off the main thread. A
message whose body has been read exports with no network at all; one that has not been opened
since a size cap dropped its source will fetch it.

## Printing a message

**The page is built once, in the core**, through `render_message_print_html`: the subject, then the
lines the reading header draws (From, To, Cc, Bcc and the date, under the label **Sent**), then the
body. A client passes each line under the label it already shows and the date as it already
formats it; a line with nothing in it is dropped by the core, so no client filters. Every header
value is escaped, so a subject or an address is text on the page and never markup.

**It is the reading document with a header on top**, not a second renderer: the body is the
snapshot's sanitised fragment (or its plain text, escaped and kept to its own line breaks), wrapped
by the same `render_document`, so the CSP, the base stylesheet and the reflow are the reading
view's. The reader's remote-images choice for this message is passed through: **a print never
loads what reading did not**, so it cannot be the way a tracking pixel fires.

**It is laid out in a web view nobody sees**, carrying every native gate of
[`rendering-security.md`](rendering-security.md) that the reading host carries, and handed to the
platform's own print dialog, which owns the printer, the paper and saving as PDF. The reading
pane's web view is not reused: it holds no header, and a plain-text body is not in a web view at
all on Apple and Android.

**Print is offered once the body has arrived.** Before that, while an open is still running, and
after a fetch that failed, the item is in the menu and disabled: a printout of a message with no
body is a header on a blank page. A client that could not lay the page out reports it with
`message_print_failed` where it reports a failed export.

It is not an `Intent`, for the reason exporting is not one.

## Per-platform

| | macOS | iOS/iPadOS | Windows | Android | Linux |
|---|:---:|:---:|:---:|:---:|:---:|
| Overflow menu at the end of the action row | ✅ | ✅ | ✅ | ✅ | ✅ |
| Header scrolls, row stays put | ✗ | ✅ | ✗ | ✗ | ✗ |
| Save as `.eml` | ✅ | ✅ | ✅ | ✅ | ✅ |
| Destination | save panel | share sheet | save picker | share sheet | save dialog |
| Result reported | inline error | inline error | inline error | toast | banner |
| Print | ✅ | ✅ | ✅ | ✅ | ✅ |
| Print dialog | `NSPrintOperation` sheet | `UIPrintInteractionController` | `ShowPrintUI` (system) | `PrintManager` | `WebKitPrintOperation` |

## Known gaps

- **View source is not built.** It is named here because the menu's rules were written for the
  set, not for the items it ships with: an item that is a duplicate of a row button, or that cannot
  be undone, is refused whichever arrives next.
- **No print shortcut.** On the desktops Print is reached through the menu only; neither Cmd+P
  nor Ctrl+P is bound to it yet.
- **No multi-message export.** Selecting several messages and exporting them is not offered
  anywhere; the export acts on the open message only.
- **The header scrolls with the message on iPhone and iPad only.** Everywhere else it stands still
  above the body. On macOS the obstacle is the platform's: the scroll the header would have to ride
  is the web view's own, and `WKWebView` exposes no scroll view there to read it from, which is the
  same limit [`reading-zoom.md`](reading-zoom.md) records for measuring a message's width. Windows,
  Android and Linux are simply not done yet, and none of them has that obstacle: WebView2, Android's
  `WebView` and `WebKitGTK` all report a scroll position.
