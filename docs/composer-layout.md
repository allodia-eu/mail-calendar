# The composer's layout: cross-platform contract

**Scope.** How a composer arranges what it holds: the fields, the files attached to the message and
the message itself, and how it scrolls. What the editor does with the text is
[`composer-security.md`](composer-security.md)'s and [`undo.md`](undo.md)'s; saving it is
[`drafts.md`](drafts.md)'s; a share's or a mail link's files arriving is
[`os-integration.md`](os-integration.md)'s.

**Principle.** A composer reads as one surface that scrolls the way a page does. The header and the
files sit above the message, every file and every line can be reached the same way at any window
size, and the formatting toolbar stays where it is drawn, in the same frame as the text it formats.
The header is native and the message is the shared editor's page, so a desktop hands one gesture
from the header's scroll to the message's (rule 2); a touch screen puts the header inside the page's
own scroll.

## The rules

| # | Rule | Why |
|---|---|---|
| 1 | **The order is the header, the files, then the message.** The header is the From, the recipients, the subject and the action row. The composer's notes (a refused send, the draft hint) sit under the action row. The message is the editor: its formatting toolbar, then its text. | A file belongs to the message as a whole, not to a place in its text, so it is listed with the fields rather than after the words. A note under the message would move down the page with every line typed. |
| 2 | **One gesture, the header first.** On a desktop the column holds the header, the files and the message, whose box is exactly as tall as the column's visible part. Down, the column carries the header away until the box fills the visible part, and the message then scrolls inside it; up, the message goes back to its top before the header returns. Which of the two a step moves is the core's answer, `composer_scroller`, and the client hands the step there. Only the message's box draws a scroll bar. On a touch screen the header is inside the page's own scroll, so there is one. Nothing else scrolls vertically: not the file list, not the header. A popup (recipient suggestions, a toolbar menu) is drawn over the column and pushes nothing. | The header is native and the message is a web page, and no desktop platform lets a native view live inside a web view's scroll. Handing the gesture over at the header's end reads as one scroll, as Outlook's does; two scroll bars, or a header that stays while the message moves, read as two. |
| 3 | **The files are the core's.** A composer reads them with `composer_files(composition)` and changes them only with `add_composer_files` and `remove_composer_file`, each of which answers with the list as it now stands. `open_composer` seeds the list with what the composer opens holding (a forward's originals, a stored draft's own), a move keeps it, and every save and the send take it from the core. No client keeps a list of its own. Files the person chose that the client still has to copy or read (a picker has closed, a drop has landed) are announced with `begin_adding_composer_files` as that work starts and handed over with `finish_adding_composer_files` once it ends, with nothing when nothing arrived, and the composer's **Attach** is drawn from the list's `attach` state through the client's control components, running until every one has ended ([`waiting.md`](waiting.md)). | Four clients each holding a list was four places for the file on screen and the file sent to differ, and four reckonings of whether a file had been added. A film from the photo library is converted after its picker has closed, and the seconds that takes showed nothing at all. |
| 4 | **A file is a tile of one line**: the file shrunk to a small square when the core offers a `preview`, and the attachment glyph otherwise ([`icons.md`](icons.md)); its name, cut in the middle with the end kept as long as the start, so both the extension and the part that tells one file from the next stay; its size in the client's own words; and a Remove control drawn from the tile's `remove` state. Tiles are laid out in rows of equal-width columns that wrap, in the core's order, and the block grows downward without a cap. Above them one line states how many files there are and their `total_size` (`compose_files_summary`). | The name and the size are what someone checks before sending, and a picture is checked faster by looking at it. Twenty screenshots share their first twenty characters, so a name cut at the end, or cut only short of its extension, makes them twenty identical tiles. One line a tile keeps a forward's twenty files to a few rows. |
| 5 | **The message is part of the column, not a box inside it.** On a desktop the editor runs in its framed mode: its page has no background of its own and the host's web view draws none, so the toolbar sits on the column's colour with no strip or frame, and the text lines up with the fields above. The message itself stays the light page it will be sent as; in a dark app it is a light sheet with its own margin. On a touch screen the page leaves the strip under the native header transparent, so the header is drawn on the app's own background in either appearance and the message's light page starts under it. | A frame and a grey strip around the editor read as a web page set into the window. The message keeps its light page because a colour picked on it is the colour that is sent. A header drawn on that light page put its light labels on white in a dark app. |
| 6 | **The caret stays in view.** The editor reports where its caret is, and a desktop client scrolls the column by what `composer_caret_scroll` answers, as far as the column's end. Inside the message's box the page keeps its caret in view itself. The caret is reported when it or the text moves, never when the message is only scrolled. | While part of the header is still shown, the bottom of the message's box is under the column's edge, and a line typed there is out of sight until the column moves. A caret reported on a scroll would pull the column back to a line the reader scrolled away from. |
| 7 | **The formatting toolbar is the message's own and stays in view.** It is at the message's top and is drawn by the page, so no client keeps a native copy of the editor's controls. On a desktop it is above the message's box, which scrolls under it, so it never moves while the message does; it keeps to **one row**, and the controls that do not fit move, last first, behind a **More** (⋯) at its end. A touch screen keeps its own: on Android, iOS and iPadOS the row sits above the keyboard, where the thumb is (`useNativeComposerChrome`), and scrolls sideways rather than hiding anything behind More. | It formats the text below it, and in a long message the text is far from the column's top. A toolbar the host moved as the column scrolled lagged the text by the round trip and flashed; one the page holds above its own scroll cannot. One row with More keeps the message's top where it is in a narrow window, as Outlook's does; a phone row scrolls because a thumb swipes it. |
| 8 | **Whether files were added is the core's answer**: `ComposerFiles::changed`, true once the list differs from what the composer opened holding. A share's files are added after the composer opens, so they count. It is the files' half of "was anything written" ([`drafts.md`](drafts.md)). | A stored draft's own files and a forward's originals are not something the user wrote; a file they picked or shared is. |
| 9 | **Actions under the column show that it runs on.** A client whose Send and Discard sit under the column draws a rule over them while the message has more below what is shown, and none at its end. | A line cut by the edge then reads as more to come rather than as the message's end. |

## How the core says it

- **`composer_files(composition) -> ComposerFiles`**, and `add_composer_files` /
  `remove_composer_file`, which answer with the same record. `ComposerFiles { tiles, changed,
  total_size, attach }`; each `ComposerFileTile { id, file_name, size, remove, preview }`. `file_name` is
  the name the message will carry (`safe_file_name`), `size` is in bytes and `None` for a file that
  could not be read, `remove` is a `ControlState` ([`waiting.md`](waiting.md)), and `total_size`
  adds up the sizes that could be read. `preview` is the file's own path when its first bytes are
  PNG, JPEG, GIF or WebP (the avatars' sniff, [`avatars.md`](avatars.md)) and it is no larger than
  `MAX_INLINE_IMAGE_BYTES`; a client decodes it at the tile's size, never at its own, and keeps the
  glyph when the decode fails.
- **`begin_adding_composer_files(composition)`** starts one addition and
  **`finish_adding_composer_files(composition, files)`** ends it, attaching what arrived as
  `add_composer_files` does; a client calls the second exactly once for each first, off the UI
  thread. `attach` is a `ControlState`, running (and so held) while any addition is under way. An
  end with none under way ends nothing, one for a composition that has gone starts no list, and a
  move leaves Attach free in the composer it moves to, since the one that started an addition is
  the one that sees it end; its files still land. `add_composer_files` stays for files that are
  ready at once, a share's as its composer opens.
- **The list lives as long as the composition**: from `open_composer` until `discard_draft`,
  `close_composition`, `save_draft_and_close` or an accepted `submit_composition`. A move opens on
  the same composition and keeps it.
- **`composer_scroller(column_offset_px, column_end_px, message_offset_px, towards_end) ->
  ComposerScroller`** answers `Column` or `Message` for one step of a gesture: down, the column
  until it is at its end (where the message's box fills the visible part); up, the message until it
  is back at its top. A client asks it for every vertical step over the message.
- **`composer_caret_scroll(offset_px, visible_px, caret_top_px, caret_bottom_px) -> Option<f64>`**,
  in the column's own coordinates: the offset to scroll to, or `None` when the caret is already in
  view. A client stops at the column's end.
- **The editor's framed mode** is switched on with `useFramedComposer()` before the page is seeded.
  The page never scrolls in it; the message scrolls in its own box. It sends two notices on the
  composer host channel ([`composer-security.md`](composer-security.md), Gate 2), each only to a
  host that announced it (`composer_host_requests_script`, which names both): `caret` with the
  caret's top and bottom in the page's coordinates, and `scroll` with how far the message is
  scrolled inside its box (`px`) and where its last line ends in the page's coordinates as it is
  scrolled now (`end`), which is past the page's height while there is more below its box. The core
  parses both.
- **`holdComposerWheelDown(hold)`** is for a host whose web view scrolls the page by a wheel step
  before the host sees it: while `composer_scroller` gives a step down to the column, the page lets
  such a step pass without scrolling the message, so the two do not both move by it. Windows calls
  it; macOS and Linux take the step before the page does and never need it.

## Per-platform

| Platform | The column | The files | The message | Caret | Toolbar |
|---|---|---|---|---|---|
| macOS | a `ScrollView` with no scroll bar of its own (`RichComposerView.Column.swift`); `WKWebView` takes every wheel event over it, so `EditorWebView` hands each vertical step to the column or the page by `composer_scroller` | a `LazyVGrid` of tiles, each picture decoded at the tile's size off the main thread (`ComposerFileThumbnail`); Attach a `ControlButton` running while the core reads what was picked or dropped | framed, as tall as the column's visible part; the web view draws no background | the column scrolled by `composer_caret_scroll`, as far as its end | one row above the message's box, the rest behind More |
| iOS/iPadOS | the editor's own page, with the header and the files hosted inside the `WKWebView`'s scroll view above the message (`TouchComposerChrome.swift`); the header takes a touch inside its own height and passes the rest to the message, and WebKit's form bar is taken off | as macOS, in the header; Attach a `PhotosOrFilesMenu` drawn through `ControlMenu`, running from the moment the photo picker or the document picker closes, or a drop lands, while each item is copied (`PhotoPick.copy`, `EditorDropTarget`) | the page's own, which is the one scroll | the WebView's own | above the keyboard, scrolling sideways (`useNativeComposerChrome`) |
| Windows | a `ScrollViewer` (`ComposerView.xaml`) with its own bar hidden, with Send and Discard under it and a rule over them while the message's `end` is below what is shown; the WebView2 scrolls its page by a wheel step before the column sees it and marks the event handled, so `ComposerView.Layout.cs` takes the handled event, hands it by `composer_scroller` and tells the page to hold a step down that is the column's; focus does not bring the message's box into view, which would carry the whole header away | an `ItemsRepeater` in a `UniformGridLayout`; the name cut by `MiddleTrimmedText`; Attach a `ControlComponents.Button` running while a pick or a drop is opened and read | as macOS | as macOS | as macOS |
| Linux | a `GtkScrolledWindow` (`ui/composer_layout.rs`), handing each step by `composer_scroller` | a `GtkFlowBox` of tiles; the picture decoded by gdk-pixbuf on a worker thread (`ui/composer_file_preview.rs`); Attach a `controls::button` drawn by the tiles, running while a worker reads what was picked, dropped or pasted | as macOS | as macOS | as macOS |
| Android | the editor's own page, with the header drawn over it (`ComposerHeaderScroll.kt`) | tiles in the header, one column on a phone; the picture decoded by `ImageDecoder` off the main thread (`ComposerFilePreview.kt`); Attach a `ControlIconButton` in the app bar, running while a pick or a drop is copied into the app's cache | the page's own, which is the one scroll | the WebView's own | above the keyboard, scrolling sideways (`useNativeComposerChrome`) |

## Enforcement

When you change a composer on any client:

1. Draw the files from `composer_files` and change them only through the core; begin an addition
   as soon as files the person chose are on their way, end it however the copy went, and draw
   Attach from `attach` through the client's control components.
2. Put nothing in the column that scrolls vertically except the message's box, and hand each step
   over the message by `composer_scroller`; draw no scroll bar for the column.
3. On a desktop, run the editor framed: its box as tall as the column's visible part, the web view
   with no background, and the caret followed with `composer_caret_scroll`.
4. Keep the editor's controls in the page; never draw a native copy of them.
5. Update the matrix above and [`capabilities.md`](capabilities.md) when a platform's reach moves.
