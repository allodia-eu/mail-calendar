# The mail command bar: cross-platform contract

**Scope.** The row of mail actions over the mailbox on a desktop: where it stands, which actions it
carries in which order, what happens when the window is too narrow for them, and what Delete does.
It binds macOS, Windows and Linux, and through rule 9 the selection-mode bar the phones and the iPad
raise. The selection the bar acts on is [`list-selection.md`](list-selection.md)'s; what each
action reaches on a conversation is [`row-actions.md`](row-actions.md)'s.

**Principle.** The bar is the row menu's actions over the selection, laid out for a pointer: the same
items, the same labels and the same answers, so a right-click and the bar above it can never
disagree. It is shaped like Outlook's, because that is the bar the people this app is for already
know.

## The rules

1. **The core decides the bar; a client draws it.** A client asks `command_bar(rows)` whenever the
   selection or the mailbox snapshot changes and draws the answer: the sections in order with a
   divider between them, each item under its action's label and icon, greyed out where it is not
   enabled. It adds no item, removes none and enables none. The core reads the bar off `row_menu`
   over the same rows, so which half of a toggle is on offer, which items respond and which message
   an item is about are the menu's answers, never a second set.

   A pick runs as the same item in a row menu does: the single-message form for an item with a
   `target`, the batch over the selection for one without (`Intent::ActOnSelection`, or the folder
   picker for Move), and a question first wherever `confirm` is set.

2. **One full-width row under the title bar, on every desktop.** The title bar keeps the window's
   own controls and the search field. Below it, one row spans the window:

   - **New Mail** at its leading edge, over the folder pane, as the bar's one framed button in
     the platform's own style for a standalone button: Liquid Glass on macOS, the accent button on
     Windows, a suggested action on Linux. Its leading edge is the line the window's own controls
     start on, where a platform has them there. It is the client's own item, not the core's: it
     names no selection.
   - **The actions** from the message list's leading edge onward, so they stand over the two panes
     they act on and move with the folder pane's divider, as Outlook's do.
   - When the folder pane is collapsed, New Mail stays where it is and the actions follow it. That
     is why New Mail lives on the bar and not in the folder pane: a pane that can be shut cannot
     hold the one control that writes a message.

   **The title row and the bar are one band of window chrome**, drawn in the folder pane's own
   surface, so the band and the folder pane frame the list and the reading pane, which keep the
   content's surface. Each platform uses the surface its toolkit gives the sidebar (on macOS the
   sidebar material run up under the title row), never a colour of our own, so dark mode and the
   desktop's tint follow.

   The bar is the mail surface's. The calendar and contacts draw their own.

3. **The bar carries, in order: Delete, Archive, Move, Flag, the read toggle; then Sync, past a
   divider.** Filing comes first because it is what a bar over an inbox is reached for most, and it
   is Outlook's order. Flag and the read toggle are single toggles naming the half that changes
   something (`list-selection.md`, rule 5). Sync acts on the mailbox rather than on the selection,
   so it is never disabled. **Report as junk** (or **not junk**, in Junk) always stands in the
   overflow menu, so the bar offers every action the row menu does except those the reading pane
   carries (open, reply, forward).

   Select all and Clear selection are not on the bar. Ctrl/⌘+A and Escape do both where the list
   has focus (`list-selection.md`, rule 9), and the Mac's Edit menu carries Select All.

4. **Labels never wrap, truncate or scroll.** Each item is its icon and its word on one line. When
   the row cannot hold every item, items leave from the **end**, Sync first, into an overflow menu
   behind a **⋯** button at the bar's end, in the order they had on the bar and with the bar's
   dividers between groups; the items that always stand there follow past a separator. The items
   still on the bar never move, so a hand that learnt where Delete is finds it at any width.

   The ⋯ button is drawn only while its menu holds something, as an icon at every width with the
   accessible name `a11y_more_actions`. Measuring decides what fits, never a width written here or
   in a client: a longer language and a larger text size each move the answer.

5. **The bar is standing chrome.** It is on screen whether or not anything is selected; with nothing
   selected every action is disabled and each toggle names its affirmative half
   (`list-selection.md`, rule 5). A disabled item keeps its place.

6. **Delete moves to Trash; only in Trash does it delete for good.** No surface offers a permanent
   delete beside Delete. When everything the selection would take is already in Trash, or its
   account has no Trash at all, the item is **Delete permanently** in Delete's place, and its
   `confirm` is set: every platform asks first, naming the count, with `delete_permanently_*`. A
   selection that is partly outside Trash is Delete, and moves what is outside. An account whose
   folders are not known yet offers neither.

   The write holds the same line: a permanent delete takes only messages filed in Trash, or any on
   an account with no Trash, whichever path asked for it. So no route, now or later, loses mail that
   was never thrown away.

7. **Move opens the folder picker the row menu opens** ([`folder-pane.md`](folder-pane.md), rule
   24), over the selection. It is disabled when the selection spans accounts, since mail never
   crosses accounts, and keeps its place.

8. **The reading pane's Delete is the bar's Delete for that message.** A client asks
   `command_bar` over the open message's row and, where that answer is Delete permanently, asks
   first and deletes for good, so Delete in the reading pane of a message in Trash empties it rather
   than doing nothing ([`reading-actions.md`](reading-actions.md)).

9. **A selection mode's bar is the same answer for a smaller screen.** The phones and the iPad raise
   a bar with their selection mode (`list-selection.md`, rule 5); it draws `command_bar` too, which
   on those devices carries no Sync (the list is pulled to sync) and no overflow extras. The mode's
   own Select all and Done stay the client's, as the mode itself is.

## Per-platform

| | macOS | Windows | Linux | iOS/iPadOS | Android |
|---|:---:|:---:|:---:|:---:|:---:|
| Bar drawn from `command_bar` (rule 1) | ✅ | ⬜ | ⬜ | ✅ | ✅ |
| New Mail on the full-width bar, actions over the list column (rule 2) | ✅ | ⬜ | ⬜ | n/a: no standing bar | n/a: no standing bar |
| Overflow into ⋯ from the end, no wrapping or scrolling (rule 4) | ✅ | ⬜ | ⬜ | n/a: icons only | n/a: icons only |
| Delete permanently only in Trash, always confirmed (rule 6) | ✅ | ⬜ | ⬜ | ✅ | ✅ |
| Reading pane's Delete follows the bar's (rule 8) | ✅ | ⬜ | ⬜ | ✅ | ✅ |

Each desktop measures with its own toolkit: a `ViewThatFits` over the possible splits on macOS,
`CommandBar`'s dynamic overflow on Windows, and a container that measures its children on Linux.

## Enforcement

- [`command_bar.rs`](../crates/mailcal-viewmodel/src/command_bar.rs) is the bar;
  [`command_bar_tests.rs`](../crates/mailcal-viewmodel/src/command_bar_tests.rs) asserts its order,
  that each action is the row menu's own item over the same rows, and what Delete is in Trash.
- The purge's own line is held at the write, in `bulk_move` and `permanently_delete`
  (`crates/mailcal-app`), and asserted in `tests_selection.rs`.
- A new bar action is a `RowAction` the row menu offers and a place in rule 3, or it is neither.
