# The composer's layout: cross-platform contract

**Scope.** How a composer arranges what it holds: the fields, the files attached to the message and
the message itself, and how it scrolls. What the editor does with the text is
[`composer-security.md`](composer-security.md)'s and [`undo.md`](undo.md)'s; saving it is
[`drafts.md`](drafts.md)'s; a share's or a mail link's files arriving is
[`os-integration.md`](os-integration.md)'s.

**Principle.** A composer is one column that scrolls as a whole, the way a page does. The files sit
above the message and the message grows with its text, so nothing inside the column has a scroll of
its own, and every file and every line can be reached the same way at any window size.

## The rules

| # | Rule | Why |
|---|---|---|
| 1 | **The order is the header, the files, then the message.** The header is the From, the recipients, the subject and the action row. The composer's notes (a refused send, the draft hint) sit under the action row. The message is the editor: its formatting toolbar, then its text. | A file belongs to the message as a whole, not to a place in its text, so it is listed with the fields rather than after the words. A note under the message would move down the page with every line typed. |
| 2 | **One scroll.** The column scrolls, and nothing inside it scrolls vertically on its own: not the file list, not the message. A popup (recipient suggestions, a toolbar menu) is drawn over the column and pushes nothing. | A scroll inside a scroll of the same direction takes the wheel or the finger from the one around it as the pointer crosses, which is why Apple's guidelines advise against it. A list with a cap of its own also hides files at exactly the moment there are many to check. |
| 3 | **The files are the core's.** A composer reads them with `composer_files(composition)` and changes them only with `add_composer_files` and `remove_composer_file`, each of which answers with the list as it now stands. `open_composer` seeds the list with what the composer opens holding (a forward's originals, a stored draft's own), a move keeps it, and every save and the send take it from the core. No client keeps a list of its own. | Four clients each holding a list was four places for the file on screen and the file sent to differ, and four reckonings of whether a file had been added. |
| 4 | **A file is a tile**: its name (middle truncation, so the extension stays), its size in the client's own words, and a Remove control drawn from the tile's `remove` state. Tiles are laid out in rows of equal-width columns that wrap, in the core's order, and the block grows downward without a cap. | The name and the size are what someone checks before sending. A grid fits a forward's dozen files in a few rows. |
| 5 | **The message grows with its text.** The editor runs in its growing mode and reports the height its content needs; the client gives it `composer_editor_height(content, room)`, where `room` is what the column's visible height leaves after the header and the files. The editor never scrolls itself. | While the text is short the message fills the window, so there is somewhere to click; once it is long, it is the column that scrolls, so one gesture reaches the subject and the last line alike. |
| 6 | **The caret stays in view.** The editor reports where its caret is, and the client scrolls the column by what `composer_caret_scroll` answers. | Typing past the bottom of the window grows the message under the column's visible part, and nothing else brings the new line into view. |
| 7 | **The formatting toolbar is the message's own**, at its top, and scrolls with it. Android's sits above the keyboard instead, where the thumb is (`useNativeComposerChrome`). | It formats the text below it. A toolbar fixed to the window would need a native copy of the editor's, which is a second implementation of every control on it. |
| 8 | **Whether files were added is the core's answer**: `ComposerFiles::changed`, true once the list differs from what the composer opened holding. A share's files are added after the composer opens, so they count. It is the files' half of "was anything written" ([`drafts.md`](drafts.md)). | A stored draft's own files and a forward's originals are not something the user wrote; a file they picked or shared is. |

## How the core says it

- **`composer_files(composition) -> ComposerFiles`**, and `add_composer_files` /
  `remove_composer_file`, which answer with the same record. `ComposerFiles { tiles, changed }`;
  each `ComposerFileTile { id, file_name, size, remove }`. `file_name` is the name the message will
  carry (`safe_file_name`), `size` is in bytes and `None` for a file that could not be read, and
  `remove` is a `ControlState` ([`waiting.md`](waiting.md)).
- **The list lives as long as the composition**: from `open_composer` until `discard_draft`,
  `close_composition`, `save_draft_and_close` or an accepted `submit_composition`. A move opens on
  the same composition and keeps it.
- **`composer_editor_height(content_px, room_px) -> f64`** and
  **`composer_caret_scroll(offset_px, visible_px, caret_top_px, caret_bottom_px) -> Option<f64>`**,
  the second in the column's own coordinates: the offset to scroll to, or `None` when the caret is
  already in view.
- **The editor's growing mode** is switched on with `useGrowingComposer()` before the page is
  seeded. In it the page never scrolls, and it sends two notices on the composer host channel
  ([`composer-security.md`](composer-security.md), Gate 2), each only when the host announced it:
  `height` with the content's height in CSS pixels, and `caret` with the caret's top and bottom in
  the page's own coordinates. The core parses both.

## Per-platform

| Platform | The column | The files | The message's height | Caret |
|---|---|---|---|---|
| macOS | a `ScrollView` (`RichComposerView.swift`); `WKWebView` keeps every wheel event over it, so `EditorWebView` hands the vertical part to the column | a `LazyVGrid` of tiles | `composer_editor_height`, from the `height` notice | scrolled to by `composer_caret_scroll` |
| iOS/iPadOS | a `ScrollView`, inside the keyboard's safe area | a `LazyVGrid` of tiles | as macOS | as macOS |
| Windows | a `ScrollViewer` (`ComposerView.xaml`) | a wrapping grid of tiles | as macOS | as macOS |
| Linux | a `GtkScrolledWindow` (`ui/composer_layout.rs`); WebKitGTK passes a wheel event the page does not use on to it | a `GtkFlowBox` of tiles | as macOS | as macOS |
| Android | the editor's own page, with the header drawn over it (`ComposerHeaderScroll.kt`) | tiles in the header | the page's own, which is the one scroll | the WebView's own |

## Enforcement

When you change a composer on any client:

1. Draw the files from `composer_files` and change them only through the core.
2. Put nothing inside the column that scrolls vertically.
3. Size the editor with `composer_editor_height` and follow the caret with
   `composer_caret_scroll`; never give the editor a height of its own choosing.
4. Update the matrix above and [`capabilities.md`](capabilities.md) when a platform's reach moves.
