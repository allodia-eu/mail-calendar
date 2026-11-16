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
| 4 | **A file is a tile of one line**: the file shrunk to a small square when the core offers a `preview`, and the attachment glyph otherwise ([`icons.md`](icons.md)); its name, cut in the middle with the end kept as long as the start, so both the extension and the part that tells one file from the next stay; its size in the client's own words; and a Remove control drawn from the tile's `remove` state. Tiles are laid out in rows of equal-width columns that wrap, in the core's order, and the block grows downward without a cap. Above them one line states how many files there are and their `total_size` (`compose_files_summary`). | The name and the size are what someone checks before sending, and a picture is checked faster by looking at it. Twenty screenshots share their first twenty characters, so a name cut at the end, or cut only short of its extension, makes them twenty identical tiles. One line a tile keeps a forward's twenty files to a few rows. |
| 5 | **The message grows with its text.** The editor runs in its growing mode and reports the height its content needs; the client gives it `composer_editor_height(content, room)`, where `room` is what the column's visible height leaves after the header and the files. The editor never scrolls itself. | While the text is short the message fills the window, so there is somewhere to click; once it is long, it is the column that scrolls, so one gesture reaches the subject and the last line alike. |
| 6 | **The caret stays in view.** The editor reports where its caret is, and the client scrolls the column by what `composer_caret_scroll` answers. A target past the column's end is applied as far as it goes and **kept**, and asked again after each layout pass until the column reaches it. Under the editor the column keeps a bottom padding of at least 16, so the editor's own padding (14) and border with it are more than the core's margin (24). | The editor reports its new height and its caret in the same frame, so the caret of a line typed past the end arrives before the column has grown to hold it, and a scroll asked for then stops at the old end with the line half under the edge. Without the padding the margin could never be reached at the last line. |
| 7 | **The formatting toolbar is the message's own and stays in view.** It is at the message's top, and on a desktop (macOS, Windows, Linux) it is drawn where the column's visible part starts while the message runs on under it, stopping where the text ends: the client tells the page where its visible part starts, in the page's coordinates, whenever the column scrolls or the header changes height, and the page draws its toolbar there. A phone keeps its own (Android's sits above the keyboard, where the thumb is: `useNativeComposerChrome`). | It formats the text below it, and in a long message the text is far from the column's top. The page draws it, so no client keeps a native copy of the editor's controls, which would be a second implementation of every one of them. |
| 8 | **Whether files were added is the core's answer**: `ComposerFiles::changed`, true once the list differs from what the composer opened holding. A share's files are added after the composer opens, so they count. It is the files' half of "was anything written" ([`drafts.md`](drafts.md)). | A stored draft's own files and a forward's originals are not something the user wrote; a file they picked or shared is. |
| 9 | **Actions under the column show that it runs on.** A client whose Send and Discard sit under the column draws a rule over them while the column has more below its visible part, and none at its end. | A line cut by the column's edge then reads as more to come rather than as the message's end. |

## How the core says it

- **`composer_files(composition) -> ComposerFiles`**, and `add_composer_files` /
  `remove_composer_file`, which answer with the same record. `ComposerFiles { tiles, changed,
  total_size }`; each `ComposerFileTile { id, file_name, size, remove, preview }`. `file_name` is
  the name the message will carry (`safe_file_name`), `size` is in bytes and `None` for a file that
  could not be read, `remove` is a `ControlState` ([`waiting.md`](waiting.md)), and `total_size`
  adds up the sizes that could be read. `preview` is the file's own path when its first bytes are
  PNG, JPEG, GIF or WebP (the avatars' sniff, [`avatars.md`](avatars.md)) and it is no larger than
  `MAX_INLINE_IMAGE_BYTES`; a client decodes it at the tile's size, never at its own, and keeps the
  glyph when the decode fails.
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
  the page's own coordinates. The core parses both. The caret's top is reported less the toolbar's
  height, because a toolbar kept in view may lie over the line above it (rule 7).
- **`setComposerVisibleTop(px)`** tells the page, in growing mode, where the column's visible part
  starts, in CSS pixels from the page's top: the column's scroll offset less where the page starts
  in the column, zero or less while the page's top is in view. The page draws its toolbar there,
  never above its own place and never past the end of the text. A client sends it when the column
  scrolls and when the height above the editor changes, one call at a time, carrying the latest
  value.

## Per-platform

| Platform | The column | The files | The message's height | Caret | Toolbar |
|---|---|---|---|---|---|
| macOS | a `ScrollView` (`RichComposerView.swift`); `WKWebView` keeps every wheel event over it, so `EditorWebView` hands the vertical part to the column | a `LazyVGrid` of tiles | `composer_editor_height`, from the `height` notice | scrolled to by `composer_caret_scroll`, kept until reached | kept in view by `setComposerVisibleTop` |
| iOS/iPadOS | a `ScrollView`, inside the keyboard's safe area | a `LazyVGrid` of tiles | as macOS | as macOS | at the message's top, scrolling with it |
| Windows | a `ScrollViewer` (`ComposerView.xaml`), with Send and Discard under it and a rule over them while it runs on; the WebView2 marks every wheel event over it handled, so `ComposerView.Layout.cs` takes the handled event and scrolls the column | an `ItemsRepeater` in a `UniformGridLayout`; the name cut by `MiddleTrimmedText` | as macOS | as macOS | as macOS |
| Linux | a `GtkScrolledWindow` (`ui/composer_layout.rs`); WebKitGTK passes a wheel event the page does not use on to it | a `GtkFlowBox` of tiles | as macOS | as macOS | as macOS |
| Android | the editor's own page, with the header drawn over it (`ComposerHeaderScroll.kt`) | tiles in the header | the page's own, which is the one scroll | the WebView's own | above the keyboard (`useNativeComposerChrome`) |

## Enforcement

When you change a composer on any client:

1. Draw the files from `composer_files` and change them only through the core.
2. Put nothing inside the column that scrolls vertically.
3. Size the editor with `composer_editor_height` and follow the caret with
   `composer_caret_scroll`, keeping a target the column cannot reach yet; never give the editor a
   height of its own choosing.
4. On a desktop, tell the page where the column shows it from with `setComposerVisibleTop`.
5. Update the matrix above and [`capabilities.md`](capabilities.md) when a platform's reach moves.
