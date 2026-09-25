# The folder pane: cross-platform contract

**Scope.** The accounts-and-folders tree: which folders are on screen, which accounts are open,
what survives a restart, what the unread badge counts, and which icon a folder gets.

**Principle.** The pane is **furniture, not navigation**. It shows the user where their mail lives
and how much of it is waiting; it does not rearrange itself because they looked somewhere else. A
person who opens their Archive and then clicks the unified Inbox has not asked for their folders to
go away.

## The rules

| # | Rule | Why |
|---|---|---|
| 1 | **Every account's folders are on screen at once.** A client renders `MailboxListSnapshot::account_folders`, which the core populates in **every** view: the unified inbox, one account's mailbox, search results, and while the calendar or contacts is showing. `folders` (the selected account's) is **not** the pane's source. | The pane used to be fed the selected account's folders alone, so choosing All Inboxes, or the account next door, emptied it. That is the bug this contract exists for. |
| 2 | **Expansion is independent of selection.** Any number of trees may stand open. Selecting an account, a folder, the unified Inbox, the calendar or contacts changes **nothing** about which trees are open. | Outlook's model, and the one people expect. Tying the two means every navigation is also a collapse. |
| 3 | **Expansion lives in the core and is persisted**, per account id, across launches. A client renders `AccountRow::expanded` and changes it with `Intent::SetAccountExpanded`; it keeps **no** expansion state of its own. | Client-held state disagrees between platforms and is lost on every restart, which is what the user reported. One owner, one answer, and the answer is still there tomorrow. |
| 4 | **A new account opens expanded**, and so does every account on the first launch after this shipped. The persisted set holds the accounts the user **collapsed**, not the ones they expanded. | The default has to be the useful one, and storing the exception is what makes "never touched it" mean *open* rather than `bool::default()`. |
| 5 | **The count is the server's, not the store's**, and counts **messages** (`Mailbox::unread_count`, engine). | A store holds only the synced window: three months by default. Counting rows would show 12 where the user's other client shows 545. Only JMAP offers the conversation form, so a portable field cannot mean that. |
| 6 | **No badge at zero.** `0` deliberately folds together "nothing unread" and "this provider reports no count", because both mean there is nothing truthful to show. | A `0` badge claims we looked and found nothing. On a provider that reports no count (Gmail today), that is a lie on every folder. |
| 7 | **The unified Inbox row shows every account's Inbox unread, summed** (`MailboxListSnapshot::unified_unread`): Inbox only, never every folder. | It badges the unified list, and that list holds inbox mail. Summing Junk and Archive into it would count mail those rows will never show. |
| 8 | **Account rows carry no count**, and neither does the All Accounts row. | The count belongs to the folders, as in Outlook; a roll-up sits directly above an identical number on the Inbox row beneath it. |
| 9 | **Icons come from the folder's role** (`FolderRow::role`, RFC 6154 SPECIAL-USE / JMAP), never from its name. A role with no distinct icon, and every custom folder, takes the plain folder. | The name is whatever the server calls it. A name test picks the wrong icon in six of the seven shipped languages, and on any server whose folders were renamed. |
| 10 | **Icons are native per platform**; the contract fixes the *meaning*, not the artwork ([`icons.md`](icons.md) says which glyph each platform draws, and from which set). | A Lucide glyph beside Segoe Fluent in a WinUI pane, or beside SF Symbols in a macOS sidebar, reads as a bug. Semantic parity, not pixel parity. |
| 11 | **On a desktop the pane is horizontally resizable**, by dragging its trailing edge, and the width is remembered across launches. It has a floor, a ceiling, and it yields to the window: the mail beside it keeps a minimum width. | An account address is as long as it is. A fixed pane clips `eva.jansen@example.c…` mid-domain, and with several accounts that is precisely the row the user needs to read. Truncation is unavoidable at *some* width, so the row also carries its full address as a tooltip. |
| 12 | **A known folder is called what *we* call it**, from the app's catalog, keyed on `FolderRow::role`. Everything else keeps the server's name. `Other` keeps it too. | The server's name for a special folder is not a name the user chose: `INBOX` shouting in capitals (the one name IMAP mandates), `Deleted Items` from Exchange, `[Gmail]/Sent Mail`. Naming them ourselves is what every mail client does, and it is what makes the folder list follow the **app's** language rather than the server's. `Other` is exempt because the core collapses flagged, important and all-mail into it: one word for three folders would be a lie. |
| 13 | The rename applies **wherever a folder is named**, not only in the pane: the list header, the sync-settings folder list, the account settings dialog. The unified scope is named by its own row, so the header over the unified list reads **Inbox**. | A folder called two things in one app is worse than one called something odd in both. Naming the *group* there instead would put a word on the header that no row the user can select carries. |
| 14 | **A folder is opened by naming it and its account together**, in one intent: `Intent::SelectFolder { account, key }`. There is no folder-only form, and the account is the one whose tree the row sits under, never whichever is selected. An account's whole mailbox is `Intent::SelectAccount`, which is the pane's only other destination. | A folder key is unique only within its account, and every account's tree is on screen (rule 1), so a bare key gets resolved against whichever account happens to be selected. From All Inboxes there is none, and the core's unified scope ignores the folder outright: the click does nothing at all. Two intents cannot express it either: each `dispatch` spawns its own task on a multi-threaded runtime, so an account issued first only *starts* first, and when the folder's handler wins the account's clears the folder it just set. |
| 15 | **Every folder row exposes one named native primary action to assistive technology.** The disclosure control remains separate because expanding an account is not opening it. | Focus and Return are keyboard mechanics, not a semantic action a screen reader can invoke. A row with no action is visible but unreachable. |
| 16 | **The unified list is a group, not a row.** **All Accounts** stands above the accounts as one more tree, with the same rules (2, 3, 4, 8) and its own persisted expansion (`MailboxListSnapshot::unified_expanded`, `Intent::SetUnifiedExpanded`). Its children are the folders the unified scope has, which today is **Inbox** alone; that child is the destination (`Intent::SelectAccount { account: None }`), it carries the badge rule 7 describes, and it is what the message-list header names (rule 13). | The group is the shape the unified scope is *growing into*: a unified Sent and Drafts are folders under one heading, not three more top-level rows. It is also what makes rule 8 cover it: a count on the heading would sit directly above the identical number on the Inbox row beneath. |
| 17 | **The group's own row navigates nowhere; activating it opens or shuts its tree.** The whole row is that control, unlike an account's, which is a destination and so needs a chevron of its own (rule 2). A shut group therefore takes the unified Inbox off screen, exactly as a shut account takes its folders. | Outlook's behaviour, and the honest one: the group heading would otherwise claim an "all mail, every account" scope the core does not have (`Scope` reaches every account's Inbox, not every account's everything). A row that navigates *and* discloses needs two targets in one row, which is what the chevron is for where the row really is a destination. |
| 19 | **A folder inside a folder is drawn inside it**, one indent step per level, with a disclosure control on the folders that hold folders and on no others. A folder's tree follows the account trees' rules (2, 3, 4): shutting one is not navigating, the state is the core's and is persisted per account **and** folder key, and a folder nobody has touched shows what is inside it. Two folders go to the top whatever the server says: a **role-bearing** one, and one whose parent this account does not list. | A mailbox is a tree on three of the four transports, and the fourth (Gmail) spells one in its label names. Drawn flat, the rows are in an order nothing on screen explains: every adapter now names a folder by its own name alone, so `2024` appears twice with nothing saying which Archive each is in. The two exceptions are what stops the tree hiding things: Gmail files Sent, Drafts and Trash inside a `[Gmail]` container over IMAP, and an unsubscribed intermediate would otherwise take its children off screen with it. |
| 18 | **The Outbox is one row above the account trees, and it exists only when something is in it.** It holds every account's unsent messages together, each row naming its own account, and its badge is that count (`MailboxListSnapshot::outbox`). At zero it is not on screen at all. | Unsent mail is the one thing a person goes looking for across *all* their accounts at once: "did that go?" is not a question about a particular mailbox. Hiding it at zero is rule 6's reasoning taken to the row itself, and it is what Outlook does; a permanent Outbox saying nothing trains people to stop reading it, which is the opposite of what an unsent message needs. It sits outside the trees because it is not a folder on anybody's server. |
| 20 | **An empty mail list says why it is empty**, from `MailboxListSnapshot::empty_reason`, and offers the sync-depth setting on `OutsideSyncDepth` and only there. `None` (the list has rows, or it is a search, which states its own horizon) draws nothing. | Rule 5 is what makes this necessary: the badge is the server's count over all time and the list holds only the synced window, so a folder whose mail predates the depth badges its unread above no rows at all. Unqualified, that empty list claims the folder is empty, which contradicts the number beside it and is the one reading the user cannot act on. On `NoMail` there is nothing to offer: the device already holds the whole folder, and a button there would promise mail that widening cannot find. |
| 21 | **The destinations that are not mail are pinned under the tree.** Calendar, Contacts and Settings sit below the accounts and do not scroll with them; the tree takes the height that is left over. Which of the three a pane carries is the platform's own answer (a phone reaches the first two from its tab bar), Settings is on every one. | An account with a few dozen folders fills the pane, and a destination at the end of that list is reached by scrolling past every folder the user has, then scrolling back to where they were. Pinning costs the tree nothing: the bar's height is reserved first and the accounts take the rest, which is the room the scroll was going to need anyway. |

## Changing the tree

A folder is made, renamed, moved and deleted from the pane, and mail is filed by dropping it on
one. Every change goes through the engine's outbox (`Engine::edit_mailbox`), so the rules below
hold offline as well as on.

| # | Rule | Why |
|---|---|---|
| 22 | **A row offers what the core says it may, and nothing else.** `FolderRow::editable` offers Rename, Move to… and Delete, and lets the row be dragged; `accepts_folders` offers New folder on it and takes a dropped folder; `accepts_messages` takes dropped mail; `AccountFolderRow::manages_folders` offers New folder on the **account** row and takes a folder dropped there, which moves it to the top of the tree. A client decides no eligibility of its own, and the core ignores a change the row did not offer. | Role folders are the app's to name and place (rules 12 and 19), Junk takes mail through a report and never a move ([`reporting.md`](reporting.md)), and whether an account can change its tree at all is its provider's capability. Four clients each deciding would be four answers. |
| 23 | **The row menu** is the platform's own context menu (right click, long press, the menu key): New folder, Rename…, Move to…, Delete…, each only where rule 22 allows; on an account row, New folder alone. A menu with nothing in it is not offered. | Every folder action is on the row it acts on, in the place each platform's users already look for one. |
| 24 | **Moving has two routes: dragging, and Move to….** A folder is dragged onto a folder or onto its account's row; a message, or the selection it belongs to, onto a folder. Neither crosses accounts. Move to… lists the account's folders that `accepts_folders`, as `folder_paths` names them, with **Top level** first, and leaves out the folder itself and everything inside it. | A drag reaches neither a keyboard nor a screen reader, and on a phone the drawer covers the list the drag would start from. The menu is the route that works everywhere. |
| 25 | **A name is checked while it is typed** (`MailcalApp::check_folder_name`), and the dialog's confirm button is enabled only on `Valid`. Every other answer has a line of its own (`folder_name_*`), shown under the field. | A server's refusal after the dialog has closed is the worst place to learn a name was taken, and it arrives after the user has moved on. |
| 26 | **Delete puts the folder in Trash, with its subfolders and their mail; deleting it again, from inside Trash, removes it for good.** A client confirms both and words the question by `FolderRow::in_trash`: `folder_delete_*` outside Trash, `folder_delete_permanent_*` inside it. | One mistaken click must not lose a folder of mail, the shape a message delete already has. Gmail cannot file a label in Trash, so there the mail goes to Trash and the label is removed; the engine absorbs the difference. |
| 27 | **A change is drawn at once, and marked while it waits.** Until the server has it the row is `pending`: drawn as waiting (dimmed, with `folder_pending` as its tooltip and accessible description), and it offers no change, no drop and no New folder. The same holds for every folder inside it. | A folder the user made should be there when they look, network or not. A key the server has not confirmed may still move (an IMAP rename re-keys the whole branch), so nothing may be built on it yet. |
| 28 | **A queued change is never applied over a change made elsewhere.** The engine reads the folder list before it sends anything and refuses a change whose folder has been renamed, moved or removed since the user saw it. The refusal stands on the pane as `MailboxListSnapshot::folder_notice` (`folder_notice_changed` or `folder_notice_refused`) until the user dismisses it (`FolderIntent::DismissNotice`) or makes another change, and the tree shows what the server has. | Last-writer-wins on a folder silently undoes someone's work. The user asked for the change, so they are told when it did not happen, and why. |
| 29 | **A folder that is open follows its change.** Renamed or moved, the list stays on it under its new key; trashed or deleted, the list opens its account's mailbox. | An open folder the server no longer has is a list of nothing with a header that names nothing. |

## Where each rule lives

Rules 12 and 13 are client-side by construction: the core has no locale (`AGENTS.md` →
"Localisation is client-side"), so it emits the **role** and each client maps it to a word from its
own catalog: `FolderLabel.For` (Windows), `folderLabel(role:name:)` (Apple), `folderLabel`
(Android), `folder_pane::folder_label` (Linux), over `folder_inbox` … `folder_trash`. Because a
folder is named in more than one screen, each of those is a single function every site calls,
rather than a `switch` per screen. The header half of rule 13 is the same shape and is tested as
one: `listTitle` (Apple), `folder_pane::header_title` (Linux), `FolderLabel.Unified` behind
`MailboxModel.CurrentFolderName` (Windows).

**A renamed special folder takes the app's word anyway.** Someone whose Archive is called
"Archief 2024" on the server sees "Archive". That is deliberate and matches Outlook, Apple Mail and
Thunderbird (a special folder's identity is its role), but it *is* a trade, and the place it would
bite is a provider that mis-tags an ordinary folder with a role.

Rule 14 is core-side in the strongest sense: `Intent::SelectFolder` carries a `FolderRef`, an
account bound to a folder key, built once at the FFI boundary
([`reference.rs`](../crates/mailcal-app/src/reference.rs)), so a client cannot dispatch a folder
without its account, and [`Scope`](../crates/mailcal-app/src/scope.rs) cannot hold one either. The
rule is unrepresentable to break rather than merely tested against.

Rule 18 is core-side and a client gets it by rendering the snapshot: `outbox` carries the rows
in every view mode and `showing_outbox` says the list is showing them rather than mail. The two
cannot be collapsed into the existing selection fields, because `selected_account` and
`selected` are both `None` on the Outbox *and* on the unified inbox, so a client that guessed
would answer a click on the Outbox with everyone's inbox.

Rule 19 is core-side apart from the drawing. `sorted_folder_rows`
([`folders.rs`](../crates/mailcal-viewmodel/src/folders.rs)) flattens the tree **depth-first**,
each folder immediately ahead of the folders inside it, and stamps `parent`, `depth` and
`has_children` on each row; `FolderPaneState::stamp_folders`
([`folder_pane.rs`](../crates/mailcal-app/src/folder_pane.rs)) then fills in `expanded` and
`visible` at publish time, for the reason the account rows' expansion is stamped there. So a pane
that draws a flat list skips a row where `visible` is false and indents by `depth`; it never walks
the chain of parents itself. The Windows pane is the exception and uses `parent`: its
`NavigationView` nests rows natively, so the framework draws the chevron, the indent and the
hiding, and `visible` is the one field it ignores.

Rules 1, 5, 6, 7 and 9 are core-side and a client gets them by rendering the snapshot:
[`folders.rs`](../crates/mailcal-viewmodel/src/folders.rs) projects `FolderRow` (with `unread` and
`role`) and sums `inbox_unread`; [`view.rs`](../crates/mailcal-viewmodel/src/view.rs) carries
`unified_unread`. Rules 2–4 are [`folder_pane.rs`](../crates/mailcal-app/src/folder_pane.rs) plus
`Preferences::collapsed_accounts`. Rules 8 and 10 are the client's to keep.

Rule 16's expansion is the same file and the same shape as rules 2–4, over
`Preferences::unified_collapsed`: the **shut** state is what is stored, so a group nobody has
touched stands open. It is stamped onto every snapshot at publish time (`restamp_expansion`)
rather than set by the projection, because `bool::default()` is *shut* and shut is the one value
that would hide the unified Inbox. Rule 17 is the client's: only it knows whether a row it drew
navigates.

The count reaches the core from the engine, which asks the server for it: JMAP `unreadEmails` and
Graph `unreadItemCount` ride along on the folder object; IMAP has no such field, so the folder-list
sync also asks: one round trip via `LIST … RETURN (STATUS (UNSEEN))` (RFC 5819) where the server
advertises LIST-STATUS, else one `STATUS` per mailbox.

**Rule 20 is decided in the core and only worded by the clients.** Which of the two cases a list
is in comes from `App::empty_reason`, over the **narrowest** sync depth of the accounts in view, so
the unified inbox answers the way a search horizon does: one account bounded at six months is
enough for the server to hold mail this device has not. Each client maps the enum to its own
catalog copy and draws it over the rows rather than in place of the list, so the surfaces around it
do not move as a folder fills: `EmptyMailboxView` (Apple), `MailboxEmpty` (Android),
`mailbox_empty::render` into the `GtkListBox` placeholder (Linux), and `EmptyMailboxLine` behind
the `EmptyMailbox` panel (Windows). The two pure mappings that can be reached without a window are
tested as such: `EmptyMailboxLineTests.cs` and `an_empty_folder_under_a_bounded_depth_says_the_server_may_hold_more`
(`mailcal-app`), which is also what holds the depth arithmetic.

Rules 22 and 27 to 29 are core-side, and a client gets them by rendering the snapshot:
`stamp_folder_actions` and `with_folder_changes`
([`folder_changes.rs`](../crates/mailcal-viewmodel/src/folder_changes.rs)) stamp each row's
flags and draw the queued changes over the stored tree, which is the engine's outbox read
([`folder_tree.rs`](../crates/mailcal-app/src/mail_ops/folder_tree.rs)). Rules 23 to 26 are the
client's to draw, from one catalog: `folder_action_*`, `folder_*_title`, `folder_name_*`,
`folder_delete_*`, `folder_notice_*`. The end-to-end cases, offline and on, are
`tests_folder_ops.rs` (`mailcal-app`).

## Per-platform

| Platform | Tree source | Expansion control | Badge | Role icons | Resizable | Opens with its account | Semantic primary action | All Accounts group (16, 17) | Folders inside folders (19) | Pinned destinations (21) |
|---|---|---|---|---|---|---|---|---|---|---|
| Windows | `account_folders` → `SidebarTree.Reconcile` | `NavigationViewItem.IsExpanded`, two-way → `Intent.SetAccountExpanded` | accent `TextBlock`, trailing | Segoe Fluent (`MainWindow.Sidebar.cs`, `RoleGlyph`) | ✅ `SidebarSplitter` → `OpenPaneLength`, persisted (`PaneLayoutStore`) | `MailboxModel.SelectFolder` ← `SidebarItem.OwnerAccountId` | `NavigationViewItem.Invoke` | ✅ `SidebarTree` group row, `SelectsOnInvoked="False"` → `Intent.SetUnifiedExpanded` | ✅ natively nested: `SidebarItem.Children` off `parent`, the framework's own chevron and indent | ✅ `FooterItems` (Contacts, Calendar) beside the framework's own Settings gear, in the `NavigationView` pane footer |
| macOS | `model.accountFolders` → `sidebarList` | chevron `Button` → `setAccountExpanded` | accent `Text`, trailing | SF Symbols (`Mailcal.Sidebar.swift`, `folderIcon`) | ✅ 220–320 pt: the `HSplitView` pane in `macOSLayout`, autosaved by AppKit (`SplitViewAutosave`) | `selectFolder(in:key:)` | SwiftUI `Button` | ✅ `allAccountsGroup` → `setUnifiedExpanded` | ✅ flat list, `visible` + `depth` × `indentWidth`, `folderDisclosure` | ✅ `sidebarDestinations`, a bottom `safeAreaInset` on the list (`Mailcal.SidebarDestinations.swift`) |
| iPadOS | `model.accountFolders` → `sidebarList` | chevron `Button` → `setAccountExpanded` | accent `Text`, trailing | SF Symbols | n/a: fixed column, per the platform | `selectFolder(in:key:)` | SwiftUI `Button` | ✅ the same pane | ✅ the same pane | ✅ the same bar |
| iOS (iPhone) | `model.accountFolders` → `sidebarList` in a drawer | chevron `Button` → `setAccountExpanded` | accent `Text`, trailing | SF Symbols | n/a: a drawer is not resizable | `selectFolder(in:key:)` | SwiftUI `Button` | ✅ the same pane | ✅ the same pane | ✅ the same bar, carrying Settings alone: Calendar and Contacts are tabs |
| Android | `accountFolders` → `FolderDrawerScaffold` | chevron `IconButton` → `Intent.SetAccountExpanded` | `NavigationDrawerItem` badge slot | Material Symbols (`FolderDrawer.kt`, `folderIcon`) | n/a: a modal drawer is not resizable | `FolderDrawer.kt`, off the row's own account | `NavigationDrawerItem` click semantics | ⬜ still one flat "All Inboxes" row | ✅ flat list, `visible` + `depth` × `FOLDER_INDENT`, `FolderDisclosure` in the icon slot | — the drawer carries no destination: Settings is in the app bar, Calendar and Contacts are tabs |
| Linux | `account_folders` → `folder_pane::render` | chevron `GtkButton` → `Intent::SetAccountExpanded` | accent `GtkLabel` pill, trailing | symbolic icons (`folder_pane_rows.rs`, `role_icon`): Adwaita's, except the bundled inbox and archive it has none of | ✅ 200–560 px: the `GtkPaned`, persisted (`HostPreferences::folder_pane_width`) | `activate_sidebar` → `SidebarTarget::Folder` | row-named `GtkButton`, also the `AdwActionRow` activatable widget | ✅ `SidebarTarget::UnifiedGroup` → `Intent::SetUnifiedExpanded` | ✅ flat list, `visible` + `margin_start` × `INDENT`, `folder_chevron` | ✅ `DestinationBar`, the `AdwToolbarView`'s bottom bar (`shell_sidebar.rs`) |

Rules 22 to 28, per platform:

| Platform | Row menu (23) | Name dialog (25) | Move to… (24) | Delete confirm (26) | Pending (27) | Notice (28) | Folder drag (24) | Message drag (24) |
|---|---|---|---|---|---|---|---|---|
| Windows | `MenuFlyout` from the pane's `ContextRequested` (`MainWindow.Folders.cs`) | `ContentDialog`, live `check_folder_name` (`FolderDialogs`) | `ContentDialog` list | `DialogHelper.ConfirmAsync` | opacity 0.5, tooltip and `HelpText` | closable `InfoBar` | ✅ `CanDrag`, the `NavigationView`'s `DragOver`/`Drop` | ✅ `ListView.CanDragItems` (`MailListView.Drag.cs`) |
| macOS | `.contextMenu` (`Mailcal.FolderActions.swift`) | `FolderNameSheet` | `FolderMoveSheet` | `.alert` | opacity 0.5, `.help`, `accessibilityValue` | banner with Close | ✅ `.draggable` / `.dropDestination` | ✅ the selection, or the row |
| iPadOS | the same | the same | the same | the same | the same | the same | ✅ with a reading pane | ✅ with a reading pane |
| iOS (iPhone) | the same, by long press | the same | the same | the same | the same | the same | — | — |
| Android | long press, `DropdownMenu` (`FolderRowMenu.kt`); each item also an accessibility action | `AlertDialog`, live `checkFolderName` | `FolderMoveDialog` | worded by `inTrash` | half opacity, state description | `FolderNoticeCard` above the list | — | — |
| Linux | right click, long press, Menu or Shift+F10 → `GtkPopover` (`folder_menu.rs`) | modal with live `check_folder_name` (`folder_dialogs.rs`) | modal list | destructive inside Trash | `dim-label`, tooltip, accessible description | `adw::Banner` over the tree | ✅ `DragSource`/`DropTarget`, a process-local boxed payload (`folder_drag.rs`) | ✅ flat and conversation rows |

The decisions behind each column (which items a row offers, the Move to… list, which drops are
taken, the line a name check shows, the delete wording) are plain code in every client and tested
without a window: `FolderActionsTests.cs`, `FolderManagementTests.swift`, `FolderEditingTest.kt`,
`folder_actions_tests.rs`. Neither phone drags: its drawer covers the list, so it moves a folder
through Move to….

The iPhone draws the same pane as the desktop, in a drawer over the whole screen (opened from the
toolbar or a drag off the leading edge). Calendar and Contacts are **not** on it there: they are
tab-bar destinations on a phone, and listing them in both places would be two routes to one screen
with one of them behind a gesture. Settings stays on the pane on every platform: it has no tab, and
it must not be something a user has to remember a gesture to find. Wherever they are on the pane,
rule 21 pins them under it.

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

- **On Windows, a folder two levels below its account does not appear until its parent is shut and
  reopened.** A `NavigationViewItem` realises the rows inside it at the moment it is told it is
  open, and it can only realise the ones it is already holding, so a row opened before its own
  children are attached keeps them hidden under a chevron that already points open. The pane now
  attaches each tree before opening the row and opens it on the layout pass after that, and it
  shuts an account row rather than emptying it when mail is not the destination, which between them
  cover the first two levels and the return from the calendar. A third level, a subfolder's own
  subfolder, is realised by the pass its parent's opening triggers and so is still absent when that
  pass runs. Everything is there and reachable, one click on the parent away; nothing is lost and
  no count is wrong. The other four panes draw a flat list from `visible` and cannot meet this.
  [`docs/client-traps.md`](client-traps.md) carries the mechanism and the oracle, and
  [issue 132](https://github.com/allodia-eu/mail-calendar/issues/132) the two routes not yet taken.

- **Gmail folders report no count.** `users.labels.list` (the call the folder sync makes) does not
  return one; only `users.labels.get` does, which would be one request per label on every
  folder-list sync. The engine reads the field where the API supplies it, so the day that fan-out
  earns its round trips, nothing here changes. Until then Gmail folders show no badge, which rule 6
  makes indistinguishable from "nothing unread".
- **The count refreshes with the folder-list sync, not instantly.** Reading a message updates the
  badge when the sync that follows the action lands, the same moment the row's own unread dot
  clears, so the two never disagree on screen. There is no optimistic local delta.
- **The selected account and folder are still not persisted.** Every launch opens on the unified
  Inbox, with the trees restored. Only expansion survives a restart; where you *were* does not.
- **Rules 16 and 17 are not on Android.** It still draws the unified list as one flat row named
  from `sidebar_all_inboxes` ("All Inboxes"), and its list header still names it the same way.
  Nothing in the core is missing: `unified_expanded` is in every snapshot,
  `Intent::SetUnifiedExpanded` is in the FFI enum, and `sidebar_all_accounts` is in every catalog
  locale, so it is a drawer change and a header change in its own toolkit.
- **The Windows group heading carries no icon**, where every row beside it does. Its whole row is
  the disclosure control (rule 17) and the framework's trailing chevron is the state, so a glyph in
  the leading slot would make it read as one more account row. Rule 10 leaves the artwork to each
  platform, and this is that latitude used; if it ever reads as a missing icon rather than as a
  heading, the fix is a glyph, not a destination.
- **A renamed folder forgets whether it was shut, on IMAP.** Expansion is stored per folder key
  (rule 19), and an IMAP rename or move gives the folder, and every folder inside it, a new key,
  so the branch opens again. JMAP, Graph and Gmail keep their keys and are unaffected.
- **A phone cannot file mail into a folder of its choosing.** Neither phone drags (rule 24), and
  the selection bar has no "Move to folder…" yet ([`list-selection.md`](list-selection.md)), so
  on Android and the iPhone mail still moves only to Archive and Trash.
- **Dragging onto the pane is not automatable on Windows**: synthetic pointer input does not
  reach WinUI content (the resize drag above is the same trap), so the drop rules are pinned in
  `FolderActionsTests.cs` and the gesture itself needs a real mouse.
- **Nothing is undone from the notice.** A refused change says so and the tree shows the
  server's copy; redoing the change is the user's, from the row menu.
- **The Graph and Gmail folder writes have not yet run against a real mailbox.** They are built
  to the documented request shapes and pass offline; their live suites exist in the engine and
  wait for a test account (`providers.md` there).
- **The group has one child.** A unified Sent, Drafts and Archive are what rule 16's shape is for,
  and none of them exists: the core's unified scope reaches every account's **Inbox** only
  ([`scope.rs`](../crates/mailcal-app/src/scope.rs)), so a second child would need a scope to
  select before it needed a row to sit on.

## Enforcement

When you change the folder pane on any client:

1. Render `account_folders`, not `folders`, and take expansion from `AccountRow::expanded`. A
   client that stores its own has broken rules 2 and 3 in a way that looks perfectly fine until it
   is restarted.
2. Hide the badge at zero (rule 6) and keep the account rows bare (rule 8), the All Accounts row
   included: its count belongs on the Inbox row beneath it.
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
6. **Do not give the All Accounts row a destination** (rule 17). It reads as one more account row,
   and an account row navigates; this one would have to claim a scope the core has no `Scope` for,
   and the Inbox child under it is already that click.
7. **Draw the tree from the fields, never from the names.** Skip a row whose `visible` is false,
   indent by `depth`, and draw a disclosure control only where `has_children` is true. Every
   adapter names a folder by its own name alone, so a pane that ignored these would put two rows
   called `2024` in one list with nothing saying which Archive each is in. And **count the same
   rows you drew**: a selection index computed over the whole list and a pane drawn from the
   visible subset are two orderings that have to agree, which is what put the highlight on the
   wrong row the last time this pane held two.
8. **Draw the trees whatever surface is on screen, and let only the highlight follow it** (rules 1
   and 2). Gating the rows on "mail is showing" looks tidy and takes the unified Inbox off the pane
   the moment the user opens the calendar, leaving no way back to it but a folder in some account.
   What the destination *does* decide is which row is lit: a folder highlighted beside a lit
   Calendar claims two surfaces at once.
9. Apply the change to **every** platform that ships a folder pane, update the matrix above, and
   record any shortfall under Known gaps rather than leaving it silent.
