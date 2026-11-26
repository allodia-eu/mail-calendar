# Reading in a dark app: cross-platform contract

**Scope.** What a message's body looks like while the app is dark: which page it is drawn on, how
its own colours are moved to suit that page, and the control that sends it back to the page it was
written for. It binds every client that draws a reading view, its separate reading windows included
([`reading-window.md`](reading-window.md)), and every client that hosts the shared editor, the
composer and the signature editor alike, whose message is written on the page it will be read on
(rules 10 to 14).

**Principle.** A dark app with a white rectangle in the middle of it is the app disagreeing with
itself, so a message follows the app. But a sender wrote for a white page, and no transform reads
every message correctly, so the way back to that page is one press away, and stays where the reader
left it.

## Rules

1. **A message is drawn dark exactly when the app is dark and the reader has left dark messages
   on.** "The app is dark" is the appearance the app actually resolved to: Dark, or System while
   the host is dark, including a host that switches at sunset while a message is open. "Left dark
   messages on" is `DisplaySettings::dark_message_background`, persisted in the core and **on by
   default**. Nothing else decides it: not the message, not its sender, not a declaration in it.
   A client therefore knows the scheme before the body arrives, which is what lets it paint the
   right canvas from the first frame (rule 5).

   A sender's `<meta name="color-scheme" content="light only">` is not honoured. Honouring it would
   put a light page under a toggle that says dark, for one message and no reason the reader can see;
   Outlook and Thunderbird do not honour it either.

2. **One preference, two doors.** The reading pane's toggle and **Settings → Reading → Dark
   background for messages** set the same value, and it persists. A reader who sends one message
   back to light is reading the next one in light too, until they press it again: that is Outlook's
   behaviour, and a per-message choice would mean pressing it on every newsletter. Changing it from
   either door re-renders every open message, in every window.

3. **The toggle exists only while the app is dark.** In a light app there is nothing to switch, so
   no client draws it there, and it appears and disappears live as the app's appearance changes. It
   sits on the action row, an icon at every width, **immediately before archive and delete**, so the
   row reads reply, reply all, forward, then the toggle, archive, delete and the overflow
   ([`reading-actions.md`](reading-actions.md)). Its glyph is what pressing it does: a **sun** on a
   dark message, a **moon** on a light one. Its accessible name says the same in words
   (`reading_show_light_background`, `reading_show_dark_background`), and so does its tooltip
   where the platform's row shows one.

4. **The transform is shared, and moves colours and nothing else.** A dark render
   (`render_message_html(…, MessageScheme::Dark)`, built in the engine's `engine-html`:
   [`html.md`](https://github.com/allodia-eu/email-calendar-sync-engine/blob/main/docs/agent-guidance/html.md),
   "Reading in a dark theme") moves each colour the message sets in its `<style>` sheets, its
   `style` attributes, `bgcolor` and `<font color>`, keeping its hue and changing only its
   perceived lightness (OKLab), by what the colour paints:

   | Role | Properties | Moves to |
   |---|---|---|
   | Fill | `background*`, `bgcolor` | A light fill becomes dark; white becomes the canvas exactly. A dark fill stays. |
   | Ink | `color`, `<font color>`, `-webkit-text-fill-color`, `caret-color` | Dark ink becomes light; black becomes the canvas's ink. Light ink stays, raised to a floor that reads on the canvas. |
   | Line | `border*`, `outline*`, `text-decoration*`, `column-rule*` | Lightness swaps end for end, so a pale divider stays subtle and a black frame stays visible. |

   What it never moves: **images**, which are drawn as sent; a declaration block whose background
   is an image (`url()`, `image-set()`), all of whose colours stay, because text over a picture was
   chosen for the picture; a value it cannot read (`var()`, `currentColor`, a malformed colour),
   which reaches the page as written; and the rules inside a sender's own
   `@media (prefers-color-scheme: dark)` block, which were written for a dark page (rule 6). A
   fill or ink already right for a dark page is not rewritten either, so a sender's own dark design
   passes through unchanged. A line always swaps: what a sender chose for it is its contrast with
   the page, which the swap keeps, and a pale divider left as written would be a bright stripe.

   Every pair the sender made legible on white (4.5:1) stays legible after the move;
   `a_pair_that_read_on_white_still_reads_on_the_dark_page` holds that for the fills and inks mail
   actually uses.

   **Why here, and why this.** A web view's own darkening is one platform's feature (Android's
   algorithmic darkening) and absent on the others, so it would be four answers. Inverting the page
   with a CSS filter is what Thunderbird tried first and abandoned: photographs come out as
   negatives and a transparent logo drawn in dark ink vanishes. Stripping the sender's colours,
   which Thunderbird ships now, loses every designed background and is the source of most of its
   dark-mode bug reports. Moving lightness and keeping hue is what Outlook and Apple Mail do.

5. **The canvas follows the scheme.** `message_canvas(scheme)` is the page each client paints behind
   the body area from the moment a message opens, and it is the same constant the document paints
   ([`sync-progress.md`](sync-progress.md), "Empty is a blank page"). The dark canvas is `#1e1e1e`
   under `#e3e3e3` ink; the ink over the area (the spinner, a plain-text body, a load error) follows
   the canvas, not the app. A plain-text body is drawn on the canvas like any other, so it follows
   the scheme with nothing to transform.

6. **A sender's own colour-scheme queries follow the page, in both schemes.** Every
   `@media (prefers-color-scheme: dark)` and `(… light)` feature is replaced with one that is always
   or never true, to match the scheme being drawn. A web view decides `prefers-color-scheme` from the
   host, and each decides differently, so left alone a light render on a dark host would apply the
   sender's dark rules to a page the core drew light. The document's own `color-scheme` names the
   scheme, so scrollbars and form controls match the page.

7. **A printout is light, always.** Paper is white, and the message was written for it:
   `render_message_print_html` takes no scheme. Exporting writes the delivered bytes
   ([`reading-actions.md`](reading-actions.md)), and a reply or forward quotes the snapshot's
   fragment, which a dark render never changes, so neither carries a moved colour.

8. **The transform cannot widen what the page may do.** It runs after the sanitiser, on the
   sanitiser's own serialisation, and writes only colour values and media features: no element, no
   attribute, no URL ([`rendering-security.md`](rendering-security.md), Layer 2).
   `a_dark_render_adds_no_markup` holds that.

9. **A scheme change keeps the reader's place.** The colours are all that changed, so the layout is
   the same: a re-render of the same message for a new scheme scrolls back to where the reader
   was, before its first paint, and keeps the message's zoom where the zoom is the view's rather
   than the page's (macOS, Windows, Linux). A different message still
   opens at its top and at its own scale ([`reading-zoom.md`](reading-zoom.md)). Where the web view
   offers no native scroll position, the app reads and sets it by script in a world of its own
   that page script cannot reach ([`rendering-security.md`](rendering-security.md), gate 1).

**What the toggle is for.** Images are drawn as sent, so a logo drawn in dark ink on a transparent
background, or text baked into a picture, can sit badly on a dark page; so can a colour set on an
element that frames a background picture further down. No client solves these, and this one does
not pretend to: the toggle is the answer, which is why it is on the row rather than in a menu.

## Writing in a dark app

The composer shows its message as the reading view will show it once it is sent. A writer who sees
one page while writing and another when reading the message back, in their Sent folder or in a
reply, has been shown two different messages.

10. **A message is written on the page it will be read on.** The editor's page is dark exactly when
    a reading view's would be (rule 1), on the same canvas (rule 5), with every colour moved by the
    same transform (rule 4). The decision is rule 1's and is the client's to make as it makes it for
    a reading view; the page and each moved colour are the core's. The page is there from the
    editor's first frame, as the reading view's canvas is (rule 5): a client hands it to the web
    view before the editor's document runs (`composer_page_preload_script`, beside the app's own
    scheme: a script injected at the document's start, or on Android, which has no such hook, put
    into the document ahead of the editor's own scripts), and the editor draws it from its head
    before any of its markup is painted. The client names it again with
    `composer_page_script(scheme)` once the document has loaded and whenever the scheme changes,
    and the editor asks how
    each colour in its message looks on it (the `colours` request, answered by
    `composer_colours_answer_script`, which the client runs as it is). The editor applies answers
    and decides nothing. The lines the core's renderer draws itself (an indented quote's rule, the
    line-and-header divider, a table's borders) are on the page from the start, moved as the reading
    view moves them; `the_page_shows_each_piece_as_the_reading_view_draws_the_sent_message` holds
    the editor's pieces to the reading view's dark render of the sent message.

11. **What is sent is what was written.** The page changes what is drawn, never what the editor
    hands out to be saved, sent or stored. A colour the writer picks is kept as a stamp and shown
    moved by a stylesheet of the page's own, so a colour picked on a dark page is sent as picked and
    shown as a reader in a dark app will see it. A quoted original and a signature are markup kept
    as it arrived, whose colours can be anywhere and `!important`, so they are moved in place, the
    value as written kept beside the value shown, and every path that takes their markup out of
    the editor puts the written value back (`writtenHtml`). A region's plain text is its words and
    never its style sheets, so a moved sheet cannot reach the `text/plain` part either.

12. **An edit runs on the colours as written.** An engine writes the computed style of what it
    works on into new markup: the content a deletion across two paragraphs moves, the typing style
    Enter carries to the next line, the selection a copy or a cut puts on the clipboard, and on a
    dark page that is the colour shown. For the length of every such edit, an undo and a redo
    included, the page is the light one with every moved colour put back, and it returns before
    anything is painted, so what the engine writes is what it writes on a light page. A character
    typed at a caret, one deleted inside the text it sits in, and an input method's composition
    carry no style and are left alone.

13. **The page's own changes are not edits.** Showing a colour moved, or putting it back, is never a
    step of the undo history, does not end the step being typed, and does not count as a change to
    the draft ([`undo.md`](undo.md), [`drafts.md`](drafts.md)).

14. **A third door to the one preference.** The editor's toolbar carries the sun or moon of rule 3,
    with its glyphs and names, drawn only while the app is dark and only by a client that names the
    page. Pressing it sets the dark-messages preference (rule 2) through
    `composer_background_choice`, and every open message and editor follows. It is the last control
    on the row, so a narrow composer keeps it behind ⋯
    ([`composer-layout.md`](composer-layout.md), rule 7).

## Per-platform

Legend: ✅ shipped · ⬜ planned · — not applicable.

| | macOS | iOS/iPadOS | Windows | Android | Linux |
|---|:---:|:---:|:---:|:---:|:---:|
| Dark render while the app is dark | ✅ | ✅ | ✅ | ✅ | ✅ |
| Canvas follows the scheme from the first frame | ✅ | ✅ | ✅ | ✅ | ✅ |
| Sun/moon toggle on the action row, dark app only | ✅ | ✅ | ✅ | ✅ | ✅ |
| Settings → Reading switch | ✅ | ✅ | ✅ | ✅ | ✅ |
| Re-renders live on an appearance or preference change | ✅ | ✅ | ✅ | ✅ | ✅ |
| A scheme change keeps the reader's place | ✅ | ✅ | ✅ | ✅ | ✅ |
| Reading windows follow | ✅ | — | ✅ | — | ✅ |
| The composer writes on the reading page (rules 10 to 13) | ⬜ | ⬜ | ⬜ | ⬜ | ✅ |
| The editor's page is right from its first frame | ⬜ | ⬜ | ⬜ | ⬜ | ✅ |
| The signature editor writes on it too | ⬜ | ⬜ | ⬜ | ⬜ | ✅ |
| Sun/moon on the editor's toolbar, dark app only (rule 14) | ⬜ | ⬜ | ⬜ | ⬜ | ✅ |
| The editor follows an appearance or preference change live | ⬜ | ⬜ | ⬜ | ⬜ | ✅ |
