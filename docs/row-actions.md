# Row actions: cross-platform contract

**Scope.** The menu a mailbox-list row opens (right-click, long press, the row's ⋯ button):
which items it holds, in which order, which of them respond, and which messages each one reaches.
It binds every client's flat rows, conversation rows and the messages listed inside an expanded
conversation, and it decides how a conversation expands and collapses as the selection moves.
The selection itself, and a menu opened over several rows, are
[`list-selection.md`](list-selection.md)'s.

**Principle.** A conversation row is a message too. Its menu offers what a message's does, and
each action reaches the messages Outlook's does: Reply answers the newest message, Flag marks the
newest one received, marking read touches what was received, a move takes the conversation. The
user should never have to expand a conversation to do something to it.

## The rules

1. **The core decides the menu; a client draws it.** A client asks `row_menu(rows)` as the menu
   opens, with the row under the pointer or the selection it belongs to, and draws the answer:
   the sections in order with a separator between them, each item under its action's label and
   icon (on Linux the label alone, as GNOME's menus carry no icons), greyed out where it is not
   enabled. It adds no item, removes none and enables none. Which
   message a conversation's item is about depends on which member is newest, which was received
   and where each is filed, and five clients working that out would give five answers.

   An item with a **`target`** is about that one message: a client opens it, answers it, flags it
   or reports it with the single-message form. An item without one acts on the rows the menu was
   asked about: a client dispatches `Intent::ActOnSelection` over them (or
   `FolderIntent::MoveMessages` for Move to folder…), and the core works out the members when the
   write lands, from the store's thread index, with the same table the menu was labelled from.

2. **"Newest" means the newest message on the conversation, whatever its kind.** A received
   message, the owner's reply filed only in Sent, or a draft. It is not the row's **face**: a
   conversation is listed, ordered and summarised by its newest message *in the folder being
   viewed*, so a reply sent from elsewhere never lifts it up the Inbox, while Open and Reply act
   on the conversation as it stands. Outlook draws the same distinction.

3. **Each action reaches one of five sets.**

   | Reach | Actions | Not built yet, and reaching this set when they are |
   |---|---|---|
   | **The newest message** | Open, Open in new window, Reply, Reply all, Forward | Print, View source, Forward as attachment; Recall and Resend; Create appointment, Create meeting |
   | **The newest received message**, else the newest message | Flag | |
   | **The newest received message** | Report as junk, Report as not junk | Block sender |
   | **Every received message** | Mark read, Mark unread | |
   | **The whole conversation** | Move to folder…, Archive, Delete, Delete permanently; Unflag reaches every flagged message | Categorise |
   | **The row itself, never a message inside it** | | Pin |

   Marking read reaches only what was received because the owner's own messages carry no read
   state anyone asked about, and marking a Sent copy unread would bold a row for mail nobody has to
   read. A conversation's unread count follows the same line, so a Sent copy another client filed
   unread never bolds a row that nothing on it can clear. Unflag reaches every flagged member, so nothing stays flagged behind a collapsed row and
   the row's label turns back to Flag. A conversation row draws a flag while any message on it is
   flagged (`ThreadRow.flagged`), which is exactly when its menu offers Unflag, and each message
   inside it draws its own.

4. **A conversation's move leaves Sent copies and drafts where they are.** A copy filed in Sent
   stays in Sent, so a conversation reopened from wherever it went still shows both sides. A draft
   stays a draft where its composer will look for it, so deleting a conversation never throws
   away unfinished writing. A draft is told by the engine's draft keyword, which every transport's
   own tell is normalised onto, never by a folder's name.

5. **A message row reaches itself.** A flat row, or a message inside an expanded conversation, is
   one message the user named, and every action reaches that message whatever its kind: a Sent
   copy picked out by itself moves out of Sent, a draft picked out by itself moves, a reply of the
   owner's can be marked unread. Rules 3 and 4 decide only for the members a conversation row
   stands for.

6. **What cannot be done is drawn disabled, not left out.** Reply, Reply all and Forward are
   disabled when the newest message is a draft, which is not yet a message anyone sent; selecting
   another message in the conversation offers them for that one. Report as junk is disabled with
   nothing received to report. A move is disabled when it would take nothing: Archive in Archive,
   Delete in Trash, either on an account with no such folder, and every move on a conversation
   whose members all stay put. A disabled item keeps its place, so the menu has the same shape
   from one row to the next.

   An item for something the conversation is not is left out rather than disabled: Report as not
   junk replaces Report as junk once the message it is about is in Junk, and a toggle (read,
   flag) shows the one state it would change to. Recall and Resend, when built, appear only when
   the newest message is Sent.

7. **The order is fixed, and the same everywhere.** Open and Open in new window; Reply, Reply all,
   Forward; the read toggle and the flag toggle; the report; Move to folder…, Archive, Delete,
   Delete permanently. Open in new window is a desktop's only
   ([`reading-window.md`](reading-window.md)). Delete permanently keeps whatever confirmation that
   platform's single-row path already has ([`list-selection.md`](list-selection.md), rule 6).

8. **One conversation is expanded: the one holding the selection.** Selecting a collapsed
   conversation expands it and opens its newest message (rule 2), the way
   [`drafts.md`](drafts.md) says that message opens. Selecting any row outside it collapses it, including a conversation
   the user expanded by its chevron, so leaving a conversation always folds it away. Adding a
   conversation to a selection of several rows expands nothing. A phone that reads a conversation
   on a screen of its own rather than expanding it in the list (Android) keeps that screen; its
   conversation row still opens the menu above.

9. **A menu over several rows is the selection's.** Its items are the batch the selection bar
   offers, labelled as the bar labels them and acting on every row
   ([`list-selection.md`](list-selection.md), rules 5 and 12); none has a target. Move to folder…
   is offered only when every row is in one account.

## Per-platform

| | macOS | iOS/iPadOS | Windows | Android | Linux |
|---|:---:|:---:|:---:|:---:|:---:|
| Row menus drawn from `row_menu` | ✅ | ✅ | ⬜ | ✅ | ✅ |
| A conversation row's menu offers every message action (rule 3) | ✅ | ✅ | ⬜ | ✅ | ✅ |
| A message inside a conversation offers its own full menu (rule 5) | ✅ | ✅ | ⬜ | n/a: no in-list expansion | ✅ |
| One conversation expanded, folding as the selection leaves it (rule 8) | ✅ | ✅ | ⬜ | n/a: a screen of its own | ✅ |
| A conversation row, and each message in it, draws its flag (rule 3) | ✅ | ✅ | ⬜ | ✅ | ✅ |
| The batch reaches what rules 3 and 4 say | ✅ | ✅ | ✅ | ✅ | ✅ |

The last row is the core's, and every client already dispatches into it. On Apple the selection
bar reads its toggles and what responds from the same answer (`RowMenuPresentation.swift`), and
`RowMenuTests` in the UI suite long-presses the showcase's conversation and leaves it. On Linux
the bar does the same (`row_menu.rs`), and `test-linux-ui.sh` flags and unflags the harness's
conversation from its menu and resumes its draft from the Inbox. On Android the bar does the same
(`RowMenuPresentation.kt`), and a message's flag is drawn on the conversation's own screen, in the
strip of its other messages.

## Enforcement

- The reach table is `conversation_reach`, `flag_target` and `stays_put` in
  [`row_reach.rs`](../crates/mailcal-viewmodel/src/row_reach.rs). The menu
  ([`row_menu.rs`](../crates/mailcal-viewmodel/src/row_menu.rs)) and the batch write
  ([`bulk.rs`](../crates/mailcal-app/src/mail_ops/bulk.rs)) both call it; neither restates it.
- Which folder a move lands in is resolved once per account, by the function the write uses
  (`account_places` in [`folders.rs`](../crates/mailcal-app/src/mail_ops/folders.rs)), so an item
  the menu enables is a write that acts.
- `row_menu_tests.rs` holds the three conversations the table was written against (the newest
  received, sent, or a draft); `tests_row_actions.rs` asserts what the batch reaches; and
  [`live_row_menu.rs`](../crates/mailcal-bindings/tests/live_row_menu.rs) asks the harness's own
  three conversations over JMAP and IMAP, so a fixture that stops threading fails there.
- A new action is a `RowAction` variant, a row in the table above with its reach, and an item on
  every client, or it is none of them. A variant nothing draws is a surface that drifts.
