# Selecting several messages: cross-platform contract

The mailbox list lets the user pick more than one row and act on the lot in one go, the way
Outlook does. This document decides what a selection *is*, what a row stands for, which actions
the bar offers, and what the keyboard does; each client then renders it in its own idiom.

The write is the core's (`Intent::ActOnSelection`, `crates/mailcal-app/src/mail_ops/bulk.rs`). The
selection itself is the client's, and rule 1 says why.

## The rules

1. **The selection lives in the client, the batch lives in the core.** Every platform's list
   control already has a selection model with the modifier, focus and assistive-technology
   behaviour its users expect, and reimplementing that on top of a core state machine would fight
   the toolkit for no gain: a selection is transient, is never persisted, and no other surface
   reads it. What a client cannot do correctly is the *write*, which is why that is one intent
   rather than a loop (rule 8).

2. **A selection is a set of rows, not of messages.** In the flat list a row is one message; in
   the threaded list it is a whole conversation. `SelectedRow` carries which, so the core expands
   a conversation itself: its members come from the store's thread index, which holds messages a
   windowed list never listed, and no client can know them.

3. **A move never takes a Sent copy out of Sent.** Archive, Trash and permanent delete over a
   conversation row leave the owner's own replies filed in Sent, so reopening the thread still
   shows both sides. This is the rule `Intent::ArchiveThread` already followed; both now run the
   same code. Marking read and flagging reach every message on the thread, Sent copies included:
   neither takes anything out of a folder.

   A **message** row the user picked out by itself always moves, Sent or not. They named that one
   message; the rule above is about the members a conversation row stands for, which they did not.

4. **The selection is scoped to the list on screen.** Changing folder or account, starting or
   clearing a search, switching between flat and threaded, and leaving mail all clear it. A row
   that leaves the list (archived, deleted, filtered away by a sync) leaves the selection with it.
   Nothing survives a relaunch.

5. **The bar states the count and offers exactly six actions**: mark read/unread, flag/unflag,
   archive, delete, delete permanently, plus **Select all** and a way out. Read and flag are
   **single toggles**, not pairs: the label comes from the selection, so any unread row makes the
   button "Mark as read", and any unflagged row makes it "Flag". A selection that is already read
   throughout therefore offers the action that changes something.

6. **Archive and delete ask nothing.** Both are recoverable, and a confirmation over fifty rows
   the user deliberately picked is a dialog they will learn to dismiss unread. **Delete
   permanently keeps whatever confirmation its single-row path already has on that platform**, so
   the bar never makes an irreversible action easier to reach than the row menu does: Windows and
   Linux confirm, Apple and Android do not, which is exactly what their row menus do today.

7. **A move clears the selection; a keyword edit keeps it.** After archive or delete the rows are
   gone, so there is nothing to keep selected. After mark-read or flag the same rows are still
   listed, and the user is usually part-way through working on them. If the message open in the
   reading pane was in the acted-on set, the pane clears; it does **not** advance the way a
   single-row archive does, because the row it would advance to may be in the same batch.

8. **One selection is one command.** `Intent::ActOnSelection` hides every affected row in one go,
   applies the writes, and syncs each account **once**; the single-row intents re-sync per write,
   which is right for one swipe and would be a hundred account-wide syncs for a hundred rows.
   Rows may span accounts (the unified list allows it) and each is written within its own account,
   since a provider key is unique only there. A row the provider refuses comes back on its own,
   leaving the rest of the batch applied; an account with no Archive folder is skipped and the
   others still act.

9. **The keyboard is the desktop's, and it is scoped to the list.** Where the message list has
   focus: **Delete** (and Backspace) moves the selection to Trash, **Escape** clears it. Both are
   bound on the list itself, never app-wide, so a focused search field or composer keeps its own
   keys, which is the whole reason for the scope.

   **Ctrl/⌘+A is the toolkit's where the toolkit has it** (`GtkListBox` in multiple-selection mode,
   a WinUI `ListView` in `Extended`), and is bound nowhere it is not: taking that key from a
   focused search field to select mail would be worse than not having it. Every platform therefore
   also offers **Select all** as a button on the bar, which is the affordance the contract counts.

10. **Select all covers the rows that are loaded**, which is the window the list is showing, not
    every message in the folder. The count says how many, so what was selected is never in doubt.

11. **Selection is exposed to assistive technology as the platform's own selected state**, not as
    a visual highlight alone: the native selected/checked property on the row, and a bar whose
    count is a readable label rather than a bare number.

## Per-platform

| Platform | Enter | Extend | Bar | Delete key | Escape | Where |
|---|---|---|---|:---:|:---:|---|
| macOS | ⌘-click a row | ⇧-click a range | over the list | ✅ | ✅ | `Mailcal.Selection.swift` |
| iPhone / iPadOS | **Select** in the toolbar | tap toggles | over the list | — | — | `Mailcal.Selection.swift` |
| Windows | Ctrl-click a row | Shift-click, Ctrl+A | over the list | ✅ | ✅ | `Views/MailListView.Selection.cs` |
| Android | long-press a row | tap toggles | contextual top bar | — | — | `MailSelectionBar.kt` |
| Linux | Ctrl-click a row | Shift-click, Ctrl+A | over the list | ✅ | ✅ | `ui/selection_input.rs` |

Mobile has no modifier keys, so both phones enter a selection mode and leave it again (Back on
Android, **Done** on iPhone/iPad); a plain tap there still opens a message, as it always did.
The desktops need no mode: a plain click selects the one row and opens it, exactly as before.

The rules themselves are one small state machine per client, unit-tested without a window:
`MailSelection.swift`, `SelectionActions.cs`, `MailSelection.kt`, `ui/selection.rs`. The batch they
all dispatch into is `crates/mailcal-app/src/mail_ops/bulk.rs`.

## Known gaps

- **Dragging rows into a folder is not implemented anywhere.** The core has no move-to-a-named-
  folder action either: `BulkAction` resolves its destination by role (Archive, Trash), which is
  all the bar offers. Adding the gesture means a `MoveToFolder` variant carrying a `FolderRef`,
  plus a drop target on every folder-pane row, in four toolkits.
- **A bulk action has no undo.** The swipe undo window (`docs/settings.md`, swipe actions) covers
  a gesture that is easy to trigger by accident; a deliberate select-then-click is not that. The
  actions the bar offers are recoverable in the mailbox instead.
- **No "Move to folder…" item on the bar**, for the same reason as the first gap.
- **Spam and not-spam are not bulk actions.** They are reports, not moves
  ([`reporting.md`](reporting.md)), only two clients offer them at all, and which verdicts exist
  is read per account from `Capabilities::mail_report`; a bar button would have to be gated on the
  narrowest capability across a selection spanning accounts. Left until the single-row affordance
  is on every platform.
- **iPad hardware keyboards get no shortcuts**, though the platform supports them: the rule above
  binds the three desktops, and adding iPad means deciding what Escape does to a selection mode
  that has its own Done button.
- **Nothing verifies the desktop behaviour end to end.** Each client's rules have unit tests, and
  the batch has its own in `tests_selection.rs`, but what a ⇧-click does to a real list is a
  toolkit behaviour: `clients/windows/uitests` is the one suite that could assert it, and it does
  not yet. Until it does, a modifier click is checked by hand.

## Enforcement

When you change what a selection does:

1. Keep the write in one place. `mail_ops/bulk.rs` owns the batch, and `archive_thread` runs
   through it rather than keeping a second copy of the Sent rule; `tests_selection.rs` asserts the
   Sent protection, the per-account routing, the one-sync-per-account cost and the individual
   restore of a refused row.
2. A new action is a `BulkAction` variant **and** a button on all five clients' bars, or it is
   neither. A variant nothing dispatches is a surface that drifts.
3. Keep rule 4. A selection that outlives the list it was made in acts on rows the user can no
   longer see, and the first thing they will know about it is the mail leaving their inbox.
