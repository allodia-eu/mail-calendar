# The folder pane: cross-platform contract

**Scope.** The accounts-and-folders tree: which folders are on screen, which accounts are open,
what survives a restart, what the unread badge counts, and which icon a folder gets.

**Principle.** The pane is **furniture, not navigation**. It shows the user where their mail lives
and how much of it is waiting; it does not rearrange itself because they looked somewhere else. A
person who opens their Archive and then clicks All Inboxes has not asked for their folders to go
away.

## The rules

| # | Rule | Why |
|---|---|---|
| 1 | **Every account's folders are on screen at once.** A client renders `MailboxListSnapshot::account_folders`, which the core populates in **every** view: the unified inbox, one account's mailbox, search results, and while the calendar or contacts is showing. `folders` (the selected account's) is **not** the pane's source. | The pane used to be fed the selected account's folders alone, so choosing All Inboxes, or the account next door, emptied it. That is the bug this contract exists for. |
| 2 | **Expansion is independent of selection.** Any number of accounts may stand open. Selecting an account, a folder, All Inboxes, the calendar or contacts changes **nothing** about which trees are open. | Outlook's model, and the one people expect. Tying the two means every navigation is also a collapse. |
| 3 | **Expansion lives in the core and is persisted**, per account id, across launches. A client renders `AccountRow::expanded` and changes it with `Intent::SetAccountExpanded`; it keeps **no** expansion state of its own. | Client-held state disagrees between platforms and is lost on every restart, which is what the user reported. One owner, one answer, and the answer is still there tomorrow. |
| 4 | **A new account opens expanded**, and so does every account on the first launch after this shipped. The persisted set holds the accounts the user **collapsed**, not the ones they expanded. | The default has to be the useful one, and storing the exception is what makes "never touched it" mean *open* rather than `bool::default()`. |
| 5 | **The count is the server's, not the store's**, and counts **messages** (`Mailbox::unread_count`, engine). | A store holds only the synced window: three months by default. Counting rows would show 12 where the user's other client shows 545. Only JMAP offers the conversation form, so a portable field cannot mean that. |
| 6 | **No badge at zero.** `0` deliberately folds together "nothing unread" and "this provider reports no count", because both mean there is nothing truthful to show. | A `0` badge claims we looked and found nothing. On a provider that reports no count (Gmail today), that is a lie on every folder. |
| 7 | **All Inboxes shows every account's Inbox unread, summed** (`MailboxListSnapshot::unified_unread`): Inbox only, never every folder. | It badges the unified list, and that list holds inbox mail. Summing Junk and Archive into it would count mail those rows will never show. |
| 8 | **Account rows carry no count.** | The count belongs to the folders, as in Outlook; a roll-up sits directly above an identical number on the Inbox row beneath it. |
| 9 | **Icons come from the folder's role** (`FolderRow::role`, RFC 6154 SPECIAL-USE / JMAP), never from its name. A role with no distinct icon, and every custom folder, takes the plain folder. | The name is whatever the server calls it. A name test picks the wrong icon in six of the seven shipped languages, and on any server whose folders were renamed. |
| 10 | **Icons are native per platform**; the contract fixes the *meaning*, not the artwork. | A Lucide glyph beside Segoe Fluent in a WinUI pane, or beside SF Symbols in a macOS sidebar, reads as a bug. Semantic parity, not pixel parity. |
| 11 | **On a desktop the pane is horizontally resizable**, by dragging its trailing edge, and the width is remembered across launches. It has a floor, a ceiling, and it yields to the window: the mail beside it keeps a minimum width. | An account address is as long as it is. A fixed pane clips `eva.jansen@example.c…` mid-domain, and with several accounts that is precisely the row the user needs to read. Truncation is unavoidable at *some* width, so the row also carries its full address as a tooltip. |
| 12 | **A known folder is called what *we* call it**, from the app's catalog, keyed on `FolderRow::role`. Everything else keeps the server's name. `Other` keeps it too. | The server's name for a special folder is not a name the user chose: `INBOX` shouting in capitals (the one name IMAP mandates), `Deleted Items` from Exchange, `[Gmail]/Sent Mail`. Naming them ourselves is what every mail client does, and it is what makes the folder list follow the **app's** language rather than the server's. `Other` is exempt because the core collapses flagged, important and all-mail into it: one word for three folders would be a lie. |
| 13 | The rename applies **wherever a folder is named**, not only in the pane: the list header, the sync-settings folder list, the account settings dialog. | A folder called two things in one app is worse than one called something odd in both. |
| 14 | **A folder is opened by naming it and its account together**, in one intent: `Intent::SelectFolder { account, key }`. There is no folder-only form, and the account is the one whose tree the row sits under, never whichever is selected. An account's whole mailbox is `Intent::SelectAccount`, which is the pane's only other destination. | A folder key is unique only within its account, and every account's tree is on screen (rule 1), so a bare key gets resolved against whichever account happens to be selected. From All Inboxes there is none, and the core's unified scope ignores the folder outright: the click does nothing at all. Two intents cannot express it either: each `dispatch` spawns its own task on a multi-threaded runtime, so an account issued first only *starts* first, and when the folder's handler wins the account's clears the folder it just set. |
| 15 | **Every folder row exposes one named native primary action to assistive technology.** The disclosure control remains separate because expanding an account is not opening it. | Focus and Return are keyboard mechanics, not a semantic action a screen reader can invoke. A row with no action is visible but unreachable. |

## Where each rule lives

Rules 12 and 13 are client-side by construction: the core has no locale (`AGENTS.md` →
"Localisation is client-side"), so it emits the **role** and each client maps it to a word from its
own catalog: `FolderLabel.For` (Windows), `folderLabel(role:name:)` (Apple), `folderLabel`
(Android), `folder_pane::folder_label` (Linux), over `folder_inbox` … `folder_trash`. Because a
folder is named in more than one screen, each of those is a single function every site calls,
rather than a `switch` per screen.

**A renamed special folder takes the app's word anyway.** Someone whose Archive is called
"Archief 2024" on the server sees "Archive". That is deliberate and matches Outlook, Apple Mail and
Thunderbird (a special folder's identity is its role), but it *is* a trade, and the place it would
bite is a provider that mis-tags an ordinary folder with a role.

Rule 14 is core-side in the strongest sense: `Intent::SelectFolder` carries a `FolderRef`, an
account bound to a folder key, built once at the FFI boundary
([`reference.rs`](../crates/mailcal-app/src/reference.rs)), so a client cannot dispatch a folder
without its account, and [`Scope`](../crates/mailcal-app/src/scope.rs) cannot hold one either. The
rule is unrepresentable to break rather than merely tested against.

Rules 1, 5, 6, 7 and 9 are core-side and a client gets them by rendering the snapshot:
[`folders.rs`](../crates/mailcal-viewmodel/src/folders.rs) projects `FolderRow` (with `unread` and
`role`) and sums `inbox_unread`; [`view.rs`](../crates/mailcal-viewmodel/src/view.rs) carries
`unified_unread`. Rules 2–4 are [`folder_pane.rs`](../crates/mailcal-app/src/folder_pane.rs) plus
`Preferences::collapsed_accounts`. Rules 8 and 10 are the client's to keep.

The count reaches the core from the engine, which asks the server for it: JMAP `unreadEmails` and
Graph `unreadItemCount` ride along on the folder object; IMAP has no such field, so the folder-list
sync also asks: one round trip via `LIST … RETURN (STATUS (UNSEEN))` (RFC 5819) where the server
advertises LIST-STATUS, else one `STATUS` per mailbox.

## Per-platform

| Platform | Tree source | Expansion control | Badge | Role icons | Resizable | Opens with its account | Semantic primary action |
|---|---|---|---|---|---|---|---|
| Windows | `account_folders` → `SidebarTree.Reconcile` | `NavigationViewItem.IsExpanded`, two-way → `Intent.SetAccountExpanded` | accent `TextBlock`, trailing | Segoe Fluent (`MainWindow.Sidebar.cs`, `RoleGlyph`) | ✅ `SidebarSplitter` → `OpenPaneLength`, persisted (`PaneLayoutStore`) | `MailboxModel.SelectFolder` ← `SidebarItem.OwnerAccountId` | `NavigationViewItem.Invoke` |
| macOS | `model.accountFolders` → `sidebarList` | chevron `Button` → `setAccountExpanded` | accent `Text`, trailing | SF Symbols (`Mailcal.Layout.swift`, `folderIcon`) | ✅ 220–320 pt: the `HSplitView` pane in `macOSLayout`, autosaved by AppKit (`SplitViewAutosave`) | `selectFolder(in:key:)` | SwiftUI `Button` |
| iPadOS | `model.accountFolders` → `sidebarList` | chevron `Button` → `setAccountExpanded` | accent `Text`, trailing | SF Symbols | n/a: fixed column, per the platform | `selectFolder(in:key:)` | SwiftUI `Button` |
| iOS (iPhone) | `model.accountFolders` → `sidebarList` in a drawer | chevron `Button` → `setAccountExpanded` | accent `Text`, trailing | SF Symbols | n/a: a drawer is not resizable | `selectFolder(in:key:)` | SwiftUI `Button` |
| Android | `accountFolders` → `FolderDrawerScaffold` | chevron `IconButton` → `Intent.SetAccountExpanded` | `NavigationDrawerItem` badge slot | Material Symbols (`FolderDrawer.kt`, `folderIcon`) | n/a: a modal drawer is not resizable | `FolderDrawer.kt`, off the row's own account | `NavigationDrawerItem` click semantics |
| Linux | `account_folders` → `folder_pane::render` | chevron `GtkButton` → `Intent::SetAccountExpanded` | accent `GtkLabel` pill, trailing | symbolic icons (`folder_pane.rs`, `role_icon`): Adwaita's, except the bundled inbox and archive it has none of | ✅ 200–560 px: the `GtkPaned`, persisted (`HostPreferences::folder_pane_width`) | `activate_sidebar` → `SidebarTarget::Folder` | row-named `GtkButton`, also the `AdwActionRow` activatable widget |

The iPhone draws the same pane as the desktop, in a drawer over the whole screen (opened from the
toolbar or a drag off the leading edge). Calendar and Contacts are **not** on it there: they are
tab-bar destinations on a phone, and listing them in both places would be two routes to one screen
with one of them behind a gesture. Settings stays on the pane on every platform: it has no tab, and
it must not be something a user has to remember a gesture to find.

Rule 14 has a test at the layer every client shares:
`a_folder_opens_in_the_account_it_names_whatever_was_selected` (`mailcal-app`) opens the same
`archive` key in two accounts from the unified list and asserts each one's own mail. Every count,
name and icon is unit-tested rather than trusted: `folders_and_accounts.rs`
(`mailcal-viewmodel`), `SidebarTreeTests.cs` (Windows), `FolderDrawerTest.kt` (Android),
`FolderPaneTests.swift` (Apple), `folder_pane_tests.rs` (Linux), and `folder_pane.rs`'s own tests
for the persistence. Name a
role-bearing folder in a fixture and the app renames it: assert the app's word, or use a folder
with no role, or the assertion is one that cannot fail. The Windows pane's rendered shape (that each row
carries a name a screen reader can read, that the badge reads as a sentence, that a shut account
takes its folders off screen, and that clicking a folder opens **that** folder) is
`FolderPane.Tests.ps1`, because none of it is visible to an assembly that cannot link WinUI. Rule 14
is the reason the last of those is there: a unit test can see that a row knows its account, and only
a running window can see that the mail list then changes.

**The resize drag itself is not automatable on Windows.** Synthetic pointer input (`mouse_event`
*and* `SendInput`) does not reach this client's WinUI content: the list|reading splitter that has
shipped for months is exactly as undrivable, which is what rules out the handler. The clamp is
therefore split into `SidebarWidth` (WinUI-free, and covered by `SidebarWidthTests.cs`) and the
gesture needs a real mouse.

## Known gaps

- **Gmail folders report no count.** `users.labels.list` (the call the folder sync makes) does not
  return one; only `users.labels.get` does, which would be one request per label on every
  folder-list sync. The engine reads the field where the API supplies it, so the day that fan-out
  earns its round trips, nothing here changes. Until then Gmail folders show no badge, which rule 6
  makes indistinguishable from "nothing unread".
- **The count refreshes with the folder-list sync, not instantly.** Reading a message updates the
  badge when the sync that follows the action lands, the same moment the row's own unread dot
  clears, so the two never disagree on screen. There is no optimistic local delta.
- **The selected account and folder are still not persisted.** Every launch opens on All Inboxes,
  with the tree restored. Only expansion survives a restart; where you *were* does not.

## Enforcement

When you change the folder pane on any client:

1. Render `account_folders`, not `folders`, and take expansion from `AccountRow::expanded`. A
   client that stores its own has broken rules 2 and 3 in a way that looks perfectly fine until it
   is restarted.
2. Hide the badge at zero (rule 6) and keep the account rows bare (rule 8).
3. Map icons from the role (rule 9) in that platform's own icon set (rule 10), and **look at
   them**. A private-use codepoint that does not exist renders as an invisible glyph or a tofu box,
   and the pane keeps drawing as though nothing happened. A *named* icon fails the same way: GTK
   draws the broken-image icon for a name the theme lacks, and Adwaita (the theme the GNOME
   runtime provides, so the one the Flatpak runs against) has no inbox and no archive glyph,
   while the Ubuntu desktop this would be written on has both. Assert the name resolves.
4. **Identify a row by its account *and* its folder key.** A folder key is unique only within its
   account, every provider calls its inbox `inbox`, so a pane holding every account's tree holds
   several rows with the same key. A framework that keys rows by identity then treats them as one
   row: on macOS the second account's Inbox drew the *first* account's unread count, with the right
   folders under the right accounts and nothing else out of place. Android composes
   `folder-<account>-<key>`, Apple `SidebarFolder.id`, Windows nests children under the account.
   The row must carry its account for **opening** it too (rule 14): that half used to fail
   silently: the pane looked right and the click did nothing.
5. **Make the whole row the target, not the words on it.** The count sits at the trailing edge with
   space between, and on a wide pane most of a row is that space. A layout gap is not automatically
   hit-testable (a SwiftUI `Spacer` is not); clicks landed on nothing until the row got an explicit
   content shape.
6. Apply the change to **every** platform that ships a folder pane, update the matrix above, and
   record any shortfall under Known gaps rather than leaving it silent.
