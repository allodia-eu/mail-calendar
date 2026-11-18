# Message importance: cross-platform contract

**Scope.** The importance a sender gives a message, high, normal or low: reading it on every
message, drawing it on a list row and above an open message, and choosing it while writing. Read
receipts and delivery notifications are different features and are not covered here.

**Principle.** Importance is the sender's word about a message, so the core reads it, decides
what each surface shows, and holds what a composer will send. A client draws a glyph, a banner and
a menu, and words them.

## The standard

| Header | Defined by | Read | Written |
|---|---|---|---|
| `Importance: high \| normal \| low` | RFC 2156 §5.3, registered by RFC 4021 §2.1.52 | first | for high and low |
| `X-Priority: 1 … 5` | none; what Thunderbird and Apple Mail write alone | when `Importance` is absent or unreadable: 1 and 2 are high, 3 normal, 4 and 5 low | `1 (Highest)` and `5 (Lowest)` beside `Importance`, as Outlook writes it |
| `Priority: urgent \| normal \| non-urgent` | RFC 2156 §5.2 | never | never |

A message with neither header is **normal**: that is what their absence means, not a value the
app invents. RFC 2156's `Priority` is the delivery priority a mail server acts on, a different
property, so it is neither read as importance nor written.

## The rules

| # | Rule | Why |
|---|---|---|
| 1 | **The engine reads it on every transport**: the two headers over IMAP, JMAP (as `header:` properties) and Gmail, and the message's `importance` property over Microsoft Graph, which Exchange fills from the same headers. It is part of the message's content and of the list's stored row. | A row has to draw it without opening the message. |
| 2 | **A list row draws a mark for high and for low, and nothing for normal**: a red exclamation mark for high, a down arrow for low, before the flag ([`icons.md`](icons.md)). The mark is announced to assistive technology. | Outlook's arrangement, and the one people know. Low is drawn too: it is the sender saying "this can wait". |
| 3 | **A conversation row is high when any of its messages is, and low only when every one is.** The core decides it (`ThreadRow.importance`); each expanded message carries its own. | An urgent message must not hide behind the replies to it. |
| 4 | **An open message carries a banner for high and for low**: "This message was sent with high importance." under the header, from `ReadingSnapshot.importance_banner`, and none when that is absent. It has no button. | There is nothing to do about a sender's importance except know it. |
| 5 | **The composer's importance is the core's**, held with the composition like its files ([`composer-layout.md`](composer-layout.md), rule 3). A client reads `composer_importance(composition)`, changes it only with `set_composer_importance`, and every save and the send take it from the composition. No client passes an importance to a save or a send. | The importance on screen and the one that goes out cannot differ, on any of four clients. |
| 6 | **A new message, a reply and a forward open normal; a stored draft and a send moved back from the Outbox open with their own.** A moved or popped-out composer keeps its composition, so it keeps its importance. | A reply's importance is its sender's word about the reply, not an echo of the original's; a draft's is what its writer chose. |
| 7 | **A changed importance is a change to the draft**: `ComposerImportance.changed` joins `ComposerFiles.changed` in "was anything written", and a pick restarts the autosave interval ([`drafts.md`](drafts.md)). | Raising a resumed draft and leaving must not lose the raise. |
| 8 | **The picker sits in the composer's action row, beside Signature**, listing what the core offers (`options`, highest first) with the current one marked. On a phone it waits behind the overflow where the action row is the app bar. | It is something you do to the message, not a field you address it with ([`signatures.md`](signatures.md) places Signature for the same reason). |
| 9 | **An assistant reads it, and does not set it.** `list_messages`, `search_messages` and `get_message` carry `importance` ([`mcp.md`](mcp.md)); `create_draft` and `send_message` send normal. | Reading it costs nothing and helps triage. Setting it would need the agent-draft handoff of every client to carry a new field, and nothing has asked for it. |

## How the core says it

- **Rows**: `FlatRow.importance`, `ThreadRow.importance`, `ThreadMessage.importance`, each a
  `MessageImportance` (`High`, `Normal`, `Low`).
- **Reading**: `ReadingSnapshot.importance_banner: Option<MessageImportance>`, `None` for none.
- **Composer**: `composer_importance(composition) -> ComposerImportance` and
  `set_composer_importance(composition, importance) -> ComposerImportance`, where
  `ComposerImportance { selected, options, changed }`. Neither waits on anything but a lock, so a
  client may call them on its UI thread.

## Per-platform

| Platform | Row mark | Banner | Picker | Where |
|---|---|---|---|---|
| macOS | `exclamationmark` red, `arrow.down` blue | under the action row, above the files | a menu in the action row beside Signature | `ImportanceViews.swift`, `RichComposerView.Importance.swift` |
| iOS/iPadOS | as macOS | in the scrolling header, under the recipients | a menu in the action bar beside Signature | as macOS |
| Windows | `E8C9` Important red, `E74B` Down in the accent colour | an `InfoBar` under the action row | a `DropDownButton` beside Signature, radio items | `MailListView.xaml`, `ReadingView.Importance.cs`, `ComposerView.Importance.cs` |
| Android | `ic_priority_high` red, `ic_arrow_downward` | under the recipients | Importance in the app bar's overflow, opening a menu | `MessageImportanceViews.kt`, `ComposerImportance.kt` |
| Linux | `mail-mark-important-symbolic` red, `go-down-symbolic` dimmed | an `AdwBanner` under the header | a `GtkMenuButton` beside Signature, radio items | `ui/mailbox.rs`, `ui/reading/importance.rs`, `ui/composer_importance.rs` |

## Enforcement

When you change how importance is read, shown or chosen:

1. Change the core first: the engine's reading, the row and banner fields, or the composition's
   importance. A client never derives one from another field.
2. Apply it to every client in the matrix, and update the matrix and [`icons.md`](icons.md).
3. Update [`capabilities.md`](capabilities.md) when a platform's reach moves.
