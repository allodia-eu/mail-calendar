//! The mailbox-list view-model: an immutable snapshot a host renders, in either a flat
//! (messages by date) or threaded (conversations by latest activity) mode.
//!
//! Pure projection over the engine's [`MailListRow`]s; state lives in the engine, the host
//! renders the snapshot. A row already names the account it belongs to, which is what lets the
//! unified "all inboxes" view be one merged list rather than a merge of several. Grouping and
//! ordering happen here so both native renderers share one definition.
//!
//! This file holds the snapshot types and the public entry points; the row projection itself
//! (and the total-order discipline every one of its sorts follows) lives in `view_rows`.

use std::sync::Arc;

use engine_api::{MailListRow, Mailbox};

use crate::{
    avatar::Avatar,
    folders::{AccountFolderRow, FolderRow, sorted_folder_rows},
    view_rows::{build_flat, build_search, build_threaded},
};

/// How the mailbox list is grouped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewMode {
    /// A flat list of messages, newest first.
    #[default]
    Flat,
    /// Conversations grouped by thread, newest activity first.
    Threaded,
}

/// One projected mail row, with the two flags only the app can decide.
///
/// The row itself comes straight from the engine; sender, subject, date, flags, preview and
/// folder membership, and the account it belongs to: so a projection reads scalars and never
/// opens a message.
#[derive(Debug, Clone)]
pub struct AccountMessage {
    /// The engine's projected row, shared (not deep-copied) from the app's cache: a snapshot
    /// rebuild pairs each cached row with its view flags by bumping this `Arc`, so re-projecting
    /// a large mailbox on every navigation / "show more" / search keystroke stays cheap.
    pub row: Arc<MailListRow>,
    /// Whether this message belongs to the folder/view currently shown: the app computes it
    /// (inbox membership for the unified view, the selected folder for a folder view, always
    /// `true` for an account's all-mail). The flat list shows only in-scope messages; a
    /// thread is shown when **any** member is in scope, but the thread's conversation carries
    /// **every** member: so a Sent reply filed only in the Sent folder still appears in a
    /// thread opened from the Inbox (the "see both sent and received" behaviour).
    pub in_scope: bool,
    /// Whether the account owner sent this message (its `From` is the owner's own address);
    /// drives the "Sent" badge on a conversation message. The app computes it, since only it
    /// knows each account's owner address.
    pub outgoing: bool,
}

impl AccountMessage {
    /// The owning account's id.
    #[must_use]
    pub fn account(&self) -> &str {
        self.row.account.as_str()
    }

    /// The message's provider key: the identity a host reconciles a row by.
    #[must_use]
    pub fn key(&self) -> &str {
        self.row.mail.key.as_str()
    }
}

/// One account in the sidebar switcher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountRow {
    /// The account's id (stable identity, used to select it).
    pub id: String,
    /// The account's email address (display label).
    pub email: String,
    /// Whether this account's folder tree is open in the sidebar.
    ///
    /// Independent of selection, and persisted across launches: the two rules that
    /// make the pane behave the way people expect from Outlook. A client renders the
    /// chevron from this and dispatches `Intent::SetAccountExpanded` to change it; it
    /// must not keep its own expansion state, or the tree will disagree with itself
    /// between platforms and across a restart (`docs/folder-pane.md`).
    pub expanded: bool,
}

/// How far back a search actually looked.
///
/// Search reads the local store and nothing else, so it can only find what sync depth kept
/// (`docs/search.md`). Stating that is the difference between "there is no such message" and
/// "we did not look that far back": the second is something the user can fix, and the first
/// is what an unqualified empty result claims.
///
/// A search spanning several accounts takes the **narrowest** of their depths: the answer is
/// only as complete as its least complete account.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchHorizon {
    /// Every message the accounts hold was searched; none of them bounds its sync depth.
    AllTime,
    /// Only mail from the last N months is on this device, so only that was searched.
    Months(u16),
}

/// An immutable mailbox-list snapshot for a host to render.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MailboxListSnapshot {
    /// The configured accounts, for the sidebar switcher.
    pub accounts: Vec<AccountRow>,
    /// The selected account's id, or `None` for the unified "all inboxes" view.
    pub selected_account: Option<String>,
    /// The selected account's folders, for the sidebar (empty in the all-inboxes view).
    pub folders: Vec<FolderRow>,
    /// Every account's sorted folder list, for the folder pane, ordered by account
    /// position in `accounts`. Populated in all view modes so the pane is always available.
    ///
    /// This, not [`folders`](Self::folders), is what a folder pane renders: the whole tree
    /// stays on screen while one account is selected, so selecting an account (or leaving
    /// mail for the calendar) no longer empties it.
    pub account_folders: Vec<AccountFolderRow>,
    /// The unread count behind the unified "All Inboxes" row: every account's Inbox
    /// unread, summed. `0` shows no badge.
    ///
    /// Present in every view mode, because the row it badges is: the pane shows All
    /// Inboxes whether or not an account is selected.
    pub unified_unread: u32,
    /// The selected folder's key, or `None` for the account's unified all-mail view.
    pub selected: Option<String>,
    /// The mode the rows are grouped in.
    pub mode: ViewMode,
    /// The rows, in display order (newest first); at most the requested `limit` of them
    /// (the visible window).
    pub rows: Vec<SnapshotRow>,
    /// How many rows the view holds in all (flat messages, or threads in threaded mode),
    /// before the display `limit`. `rows.len() < total` means more can be shown: the host
    /// requests a larger window as it scrolls (`Intent::ShowMore`).
    pub total: usize,
    /// How far back these results were searched, or `None` when the list is not a search.
    ///
    /// A client renders it beside the results and offers a route to the sync-depth setting
    /// (`docs/search.md`). It is `None` for every non-search list, so a client can key the
    /// whole line off this one field.
    pub search_horizon: Option<SearchHorizon>,
}

/// One mailbox-list row: a single message (flat) or a conversation (threaded).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnapshotRow {
    /// A single message.
    Flat(FlatRow),
    /// A conversation summary.
    Thread(ThreadRow),
}

/// A single message row (flat mode).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlatRow {
    /// The id of the account this message belongs to.
    pub account: String,
    /// The message's provider key (stable identity).
    pub key: String,
    /// The subject (empty if none).
    pub subject: String,
    /// The sender's display name, falling back to their email address when the header carried
    /// no name (empty if there's no sender at all). The reading view carries the full
    /// `Name <email>`; a list row shows just the friendlier name.
    pub from: String,
    /// The sender's bare email address (empty if there's no sender).
    ///
    /// Not for display; [`from`](Self::from) is what a row shows. This is the identity the
    /// avatar is *of*, kept so the app layer can resolve a photo for it without re-deriving
    /// which sender a row names. It is deliberately **not** carried across the FFI: no client
    /// draws it, and a field nobody reads is a field that drifts.
    pub from_address: String,
    /// The sender's monogram, colour and photo.
    pub avatar: Avatar,
    /// The received date, formatted (empty if unknown).
    pub date: String,
    /// Whether the message is unread.
    pub unread: bool,
    /// Whether the message is flagged (`$flagged`).
    pub flagged: bool,
    /// Whether the provider says the message has a non-inline attachment.
    pub has_attachment: bool,
    /// A short body-preview snippet for a two-line list row (empty if the provider didn't supply
    /// one; Microsoft 365 and JMAP populate it; IMAP does not yet).
    pub preview: String,
}

/// A conversation row (threaded mode): a thread and its summary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadRow {
    /// The id of the account this conversation belongs to.
    pub account: String,
    /// The thread's id (stable identity for expansion/actions).
    pub thread_id: String,
    /// The thread's representative message; its latest **in-scope** message (the newest one that
    /// touches the viewed folder). A folder lists and orders a thread by this, and it's what a
    /// host opens from the row; the owner's own Sent replies filed elsewhere ride in
    /// [`Self::messages`] for reference but don't become the thread's face in the folder.
    pub latest_key: String,
    /// The representative (latest in-scope) message's subject (empty if none).
    pub subject: String,
    /// The representative (latest in-scope) message's sender: its display name, falling back to
    /// the email address when the header carried no name (empty if none).
    pub latest_from: String,
    /// The latest message's sender address (empty if there is none); see
    /// [`FlatRow::from_address`].
    pub latest_from_address: String,
    /// The latest sender's monogram, colour and photo.
    pub avatar: Avatar,
    /// The representative (latest in-scope) message's date, formatted (empty if unknown).
    pub latest_date: String,
    /// How many messages the thread holds.
    pub message_count: u32,
    /// How many of them are unread.
    pub unread_count: u32,
    /// Whether any message in the conversation has a non-inline attachment.
    pub has_attachment: bool,
    /// A short preview snippet of the representative message's body (empty if none), for a
    /// two-line list row.
    pub preview: String,
    /// Every message on the thread, **newest first**: the whole conversation (received and
    /// the owner's own Sent replies alike), so a host can expand the row into a stacked
    /// reading view and open any message. Note the first entry is the newest message *overall*,
    /// which may be an out-of-folder Sent reply: not necessarily [`Self::latest_key`] (the
    /// latest **in-scope** message the summary reflects).
    pub messages: Vec<ThreadMessage>,
}

/// One message within an expanded conversation. A [`ThreadRow`] carries the full, ordered set
/// so a host renders the whole thread; including the account owner's own Sent replies filed
/// in another folder, and can open any message for reading by its `key`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadMessage {
    /// The id of the account this message belongs to.
    pub account: String,
    /// The message's provider key (stable identity; what a host opens for reading).
    pub key: String,
    /// The sender's display name, falling back to the email address when the header carried no
    /// name (empty if none).
    pub from: String,
    /// The sender's bare email address (empty if none); see [`FlatRow::from_address`].
    pub from_address: String,
    /// The sender's monogram, colour and photo.
    pub avatar: Avatar,
    /// The message's date, formatted (empty if unknown).
    pub date: String,
    /// A short preview snippet for the collapsed card (empty if none).
    pub preview: String,
    /// Whether the message is unread.
    pub unread: bool,
    /// Whether the account owner sent this message (drives the "Sent" badge).
    pub outgoing: bool,
    /// Whether the message has a non-inline attachment.
    pub has_attachment: bool,
}

/// Builds the snapshot: projects `messages` (already selected by the app: the unified
/// inboxes, or one account's folder) into `mode`, decorated with the `accounts` switcher,
/// the `selected_account`, and the selected account's `folders` + `selected` folder.
///
/// Which messages belong to the shown view is carried per message by
/// [`AccountMessage::in_scope`] (the app computes it; inbox membership for the unified view,
/// the selected folder for a folder view, all of them for an account's all-mail). The flat
/// list shows only in-scope messages; a thread is shown when **any** member is in scope, but
/// each shown thread carries its **whole** conversation across folders. `selected_folder` is
/// only carried through to [`MailboxListSnapshot::selected`] to mark the active sidebar row.
///
/// Only the first `limit` rows (newest first) are built and returned: the visible window;
/// `snapshot.total` reports the full count so the host knows whether scrolling can show
/// more. Ordering and grouping still consider **every** message, so the window is always the
/// true top of the list, only row construction (and the FFI crossing) is capped.
#[must_use]
// Eight parameters match the eight distinct inputs this projection needs; a wrapper struct
// would just move the field names without reducing coupling.
#[allow(clippy::too_many_arguments)]
pub fn build(
    messages: &[AccountMessage],
    folders: &[Mailbox],
    accounts: &[AccountRow],
    account_folders: Vec<AccountFolderRow>,
    selected_account: Option<&str>,
    selected_folder: Option<&str>,
    mode: ViewMode,
    limit: usize,
) -> MailboxListSnapshot {
    let items: Vec<&AccountMessage> = messages.iter().collect();
    let mut snapshot = match mode {
        ViewMode::Flat => build_flat(&items, limit),
        ViewMode::Threaded => build_threaded(&items, limit),
    };
    snapshot.folders = sorted_folder_rows(folders);
    snapshot.unified_unread = unified_unread(&account_folders);
    snapshot.account_folders = account_folders;
    snapshot.selected = selected_folder.map(str::to_owned);
    snapshot.accounts = accounts.to_vec();
    snapshot.selected_account = selected_account.map(str::to_owned);
    snapshot
}

/// The "All Inboxes" badge: every account's Inbox unread, summed.
///
/// Saturating, so a set of accounts whose counts overflow a `u32` reports the ceiling
/// rather than wrapping to a small, confidently wrong number.
fn unified_unread(account_folders: &[AccountFolderRow]) -> u32 {
    account_folders.iter().fold(0u32, |total, account| {
        total.saturating_add(crate::folders::inbox_unread(&account.folders))
    })
}

/// Builds the search-results snapshot by **merging** every account's `hits` into one list
/// ordered **newest first**; capped at `limit` rows overall, then decorated with the
/// `accounts` switcher.
///
/// The merge is by time, not by relevance: hits from several accounts pool into a single
/// chronological list, so the newest match leads whichever account it came from, and no
/// account dominates the head just because it was searched first. `total` equals the returned
/// rows; search results have no "show more", so the host never asks to grow the window.
/// The row projection (`view_rows::build_search`) owns the ordering, and why it is time.
#[must_use]
pub fn search_results(
    hits: &[AccountMessage],
    accounts: &[AccountRow],
    account_folders: Vec<AccountFolderRow>,
    limit: usize,
) -> MailboxListSnapshot {
    let items: Vec<&AccountMessage> = hits.iter().collect();
    let mut snapshot = build_search(&items, limit);
    snapshot.accounts = accounts.to_vec();
    snapshot.unified_unread = unified_unread(&account_folders);
    snapshot.account_folders = account_folders;
    snapshot
}

#[cfg(test)]
#[path = "view_tests.rs"]
mod tests;
