//! Sidebar folder ordering and folder-row types for the mailbox-list view-model.
//!
//! Orders an account's mailboxes so the special (role-bearing) folders lead in a fixed
//! canonical order ahead of every custom folder, so the folder tree reads identically on every
//! platform regardless of the (arbitrary) order the provider lists folders in
//! ; grouping and ordering live in the core.
//!
//! [`FolderRole`] and [`FolderRow`] live here because they model folder identity and metadata;
//! the natural home alongside the ordering logic, and are re-exported from the crate root.

use engine_api::{Mailbox, MailboxRole};

/// The special role a folder plays, mirroring RFC 6154 SPECIAL-USE and JMAP equivalents.
/// Exposed on [`FolderRow`] so clients can badge or group well-known folders without
/// hard-coding name heuristics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FolderRole {
    /// The primary inbox.
    Inbox,
    /// Drafts; messages in-progress.
    Drafts,
    /// Sent: a copy of every sent message.
    Sent,
    /// Archive; long-term storage.
    Archive,
    /// Junk / Spam; server-side spam filter destination.
    Junk,
    /// Trash; recoverable deleted messages.
    Trash,
    /// Other role-bearing special folder (flagged, all, important, …).
    Other,
}

/// One sidebar folder: its key, display name, optional special role, and unread count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderRow {
    /// The mailbox's provider key (stable identity, used to select it).
    pub key: String,
    /// The folder's display name.
    pub name: String,
    /// The folder's special role, or `None` for an ordinary custom folder.
    pub role: Option<FolderRole>,
    /// How many messages in the folder are unread, as the **server** counts them;
    /// so it covers mail older than the synced window, which is what makes it the
    /// same number the user's other mail client shows.
    ///
    /// `0` means "show no badge", and folds together the two cases that render
    /// identically: nothing is unread, and the provider reported no count at all
    /// (Gmail today, or an IMAP folder the server refused to `STATUS`). A client
    /// hides the badge at zero rather than drawing one, so distinguishing them
    /// would change no pixel.
    pub unread: u32,
}

/// One account's sorted folder list; used by the folder pane to show every
/// account's folders at once, with each account as an expandable group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountFolderRow {
    /// The account's stable id, matching `AccountRow::id`.
    pub account_id: String,
    /// The account's sorted folder rows, ready for the sidebar.
    pub folders: Vec<FolderRow>,
}

/// One mailbox's client-visible role, or `None` for an ordinary folder.
///
/// Public because the sync-settings list names the same folders the sidebar does, and a second
/// mapping would be a second chance to disagree about which folder is the Trash.
#[must_use]
pub fn folder_role(mailbox: &Mailbox) -> Option<FolderRole> {
    mailbox.role.as_ref().map(mailbox_role_to_folder_role)
}

fn folder_row(mailbox: &Mailbox) -> FolderRow {
    FolderRow {
        key: mailbox.id.key().as_str().to_owned(),
        name: mailbox.name.clone(),
        role: folder_role(mailbox),
        unread: mailbox.unread_count.unwrap_or(0),
    }
}

/// The unread count of the account's Inbox within `folders`, or `0` when it has no
/// Inbox row (or the provider reported no count for it).
///
/// Only the Inbox: the unified view lists every account's **inbox** mail, so its badge
/// has to count the same folders the list shows. Summing every folder would count Junk
/// and Archive into a number sitting above rows that will never include them.
#[must_use]
pub fn inbox_unread(folders: &[FolderRow]) -> u32 {
    folders
        .iter()
        .find(|folder| folder.role == Some(FolderRole::Inbox))
        .map_or(0, |folder| folder.unread)
}

/// Maps an engine [`MailboxRole`] to the client-visible [`FolderRole`].
fn mailbox_role_to_folder_role(role: &MailboxRole) -> FolderRole {
    match role {
        MailboxRole::Inbox => FolderRole::Inbox,
        MailboxRole::Drafts => FolderRole::Drafts,
        MailboxRole::Sent => FolderRole::Sent,
        MailboxRole::Archive => FolderRole::Archive,
        MailboxRole::Junk => FolderRole::Junk,
        MailboxRole::Trash => FolderRole::Trash,
        MailboxRole::Flagged
        | MailboxRole::Important
        | MailboxRole::All
        | MailboxRole::Other(_) => FolderRole::Other,
    }
}

/// Orders the sidebar folders so the **special** (role-bearing) mailboxes lead, in a
/// fixed canonical order (Inbox, Drafts, Sent, …, Trash), ahead of **every** other
/// folder, which then follow by the provider's sort hint, then case-insensitive name.
/// Ordering lives here, in the shared view-model, so the folder tree reads identically
/// on every platform regardless of the (arbitrary) order the provider lists folders in
/// ; grouping and ordering live in the core.
pub fn sorted_folder_rows(folders: &[Mailbox]) -> Vec<FolderRow> {
    let mut ordered: Vec<&Mailbox> = folders.iter().collect();
    ordered.sort_by_key(|mailbox| folder_sort_key(mailbox));
    ordered.into_iter().map(folder_row).collect()
}

/// The sort key for one folder: special folders form the first group (`0`) ordered by
/// their [`role_rank`]; all others form the second group (`1`). Within each group the
/// provider's `sort_order` then the case-insensitive name break ties: so two custom
/// folders sort by name, and a provider that does supply sort hints (JMAP) is honoured.
fn folder_sort_key(mailbox: &Mailbox) -> (u8, u8, u32, String) {
    let (group, rank) = match mailbox.role.as_ref().and_then(role_rank) {
        Some(rank) => (0, rank),
        None => (1, 0),
    };
    (group, rank, mailbox.sort_order, mailbox.name.to_lowercase())
}

/// The canonical sidebar position of a recognised special-use role, or `None` for an
/// unrecognized ([`MailboxRole::Other`]) role, which is treated as an ordinary folder
/// rather than a special one. The match is exhaustive on purpose: a new engine role
/// must be given an explicit position here (the build breaks until it is) rather than
/// silently falling among the custom folders.
fn role_rank(role: &MailboxRole) -> Option<u8> {
    Some(match role {
        MailboxRole::Inbox => 0,
        MailboxRole::Drafts => 1,
        MailboxRole::Sent => 2,
        MailboxRole::Archive => 3,
        MailboxRole::Junk => 4,
        MailboxRole::Trash => 5,
        MailboxRole::Flagged => 6,
        MailboxRole::Important => 7,
        MailboxRole::All => 8,
        MailboxRole::Other(_) => return None,
    })
}
