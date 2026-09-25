//! Sidebar folder ordering and folder-row types for the mailbox-list view-model.
//!
//! Orders an account's mailboxes so the special (role-bearing) folders lead in a fixed
//! canonical order ahead of every custom folder, so the folder tree reads identically on every
//! platform regardless of the (arbitrary) order the provider lists folders in
//! ; grouping and ordering live in the core.
//!
//! [`FolderRole`] and [`FolderRow`] live here because they model folder identity and metadata;
//! the natural home alongside the ordering logic, and are re-exported from the crate root.

use std::collections::{BTreeMap, BTreeSet};

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

/// One sidebar folder: its key, display name, optional special role, unread count, and where
/// it sits in the account's folder tree.
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent facts a pane reads one at a time, not the states of one machine"
)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderRow {
    /// The mailbox's provider key (stable identity, used to select it).
    pub key: String,
    /// The folder's display name: the folder's **own** name, never the path to it. Every
    /// adapter gives the segment alone, so the row says `2024` and the indent says which
    /// Archive it is in.
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
    /// The [`key`](FolderRow::key) of the row this one is drawn underneath, or `None` at the
    /// top of the account's tree.
    ///
    /// The row's parent, which is not always the server's: a role-bearing folder is drawn at
    /// the top however the provider files it, and a folder whose parent this account does not
    /// list has nowhere else to go ([`sorted_folder_rows`]).
    pub parent: Option<String>,
    /// How many rows up the chain of parents reaches the top: `0` for a top-level folder, `1`
    /// for one filed inside it, and so on. What a client multiplies by one indent step.
    pub depth: u32,
    /// Whether any other row in this account names this one as its [`parent`](FolderRow::parent).
    ///
    /// What decides whether the row draws a disclosure control at all: a folder that holds only
    /// mail has nothing to open, and a chevron beside it would promise a tree that is not there.
    pub has_children: bool,
    /// Whether this folder's own children are showing. Meaningless, and always `false`, where
    /// [`has_children`](FolderRow::has_children) is `false`.
    ///
    /// The core's answer, persisted per account and folder, and changed with
    /// `Intent::SetFolderExpanded`; a client keeps none of its own
    /// (`docs/folder-pane.md`, rule 3).
    pub expanded: bool,
    /// Whether the row belongs on screen: `false` when some folder above it in the chain is
    /// shut.
    ///
    /// Answered here rather than by each pane, because the four of them would each have to
    /// walk the chain and three of them draw a flat list with an indent, where the walk is not
    /// something the toolkit does for them. A pane whose framework nests rows of its own
    /// (Windows) renders the tree and lets the framework hide what is shut, so it reads
    /// [`expanded`](FolderRow::expanded) and ignores this.
    pub visible: bool,
    /// Whether a change to this folder, or to one it sits inside, has not reached the server
    /// yet: queued while offline, or still on its way. A pending folder takes no further
    /// change, no subfolder and no mail until the server has it, because its key may still move.
    pub pending: bool,
    /// Whether the folder sits inside the account's Trash, where deleting it is permanent.
    pub in_trash: bool,
    /// Whether the user may rename, move or delete this folder. Never a folder with a role,
    /// which the app names and places itself (`docs/folder-pane.md`, rules 12 and 19).
    pub editable: bool,
    /// Whether a new folder may be made inside this one, or a folder dropped onto it.
    pub accepts_folders: bool,
    /// Whether messages may be dropped onto this folder.
    pub accepts_messages: bool,
}

/// One account's sorted folder list; used by the folder pane to show every
/// account's folders at once, with each account as an expandable group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountFolderRow {
    /// The account's stable id, matching `AccountRow::id`.
    pub account_id: String,
    /// The account's sorted folder rows, ready for the sidebar.
    pub folders: Vec<FolderRow>,
    /// Whether this account's folders can be created, renamed, moved and deleted from here.
    /// What decides whether its account row offers "New folder" and takes a dropped folder.
    pub manages_folders: bool,
}

/// One mailbox's client-visible role, or `None` for an ordinary folder.
///
/// Public because the sync-settings list names the same folders the sidebar does, and a second
/// mapping would be a second chance to disagree about which folder is the Trash.
#[must_use]
pub fn folder_role(mailbox: &Mailbox) -> Option<FolderRole> {
    mailbox.role.as_ref().map(mailbox_role_to_folder_role)
}

fn folder_row(
    mailbox: &Mailbox,
    parent: Option<&str>,
    depth: u32,
    has_children: bool,
) -> FolderRow {
    FolderRow {
        key: mailbox.id.key().as_str().to_owned(),
        name: mailbox.name.clone(),
        role: folder_role(mailbox),
        unread: mailbox.unread_count.unwrap_or(0),
        parent: parent.map(str::to_owned),
        depth,
        has_children,
        // Open, and on screen, until the core says otherwise. Expansion is stamped onto the
        // snapshot at publish time (`App::restamp_expansion`) rather than read here, so a
        // rebuild that began before the user's chevron cannot publish the state as it was
        // then; the same reason the account rows' own expansion is stamped there.
        expanded: has_children,
        visible: true,
        // Stamped by `stamp_folder_actions`, which knows the account and what is queued.
        pending: false,
        in_trash: false,
        editable: false,
        accepts_folders: false,
        accepts_messages: false,
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

/// The account's folders as a **tree**, flattened depth-first into the order a pane draws
/// them: each folder immediately followed by the folders filed inside it.
///
/// Siblings are ordered the way the whole list used to be: the
/// **special** (role-bearing) mailboxes lead, in a fixed canonical order (Inbox, Drafts,
/// Sent, …, Trash), ahead of every other folder, which then follow by the provider's sort
/// hint and then case-insensitive name. Ordering lives here, in the shared view-model, so the
/// folder tree reads identically on every platform regardless of the (arbitrary) order the
/// provider lists folders in.
///
/// Flat rather than nested because a `Vec<FolderRow>` crosses the FFI and three of the four
/// panes draw a flat list anyway; [`depth`](FolderRow::depth) is what they indent by, and
/// [`parent`](FolderRow::parent) is what the fourth re-nests from.
#[must_use]
pub fn sorted_folder_rows(folders: &[Mailbox]) -> Vec<FolderRow> {
    let known: BTreeSet<&str> = folders.iter().map(|mailbox| mailbox.id.as_str()).collect();
    let mut children: BTreeMap<&str, Vec<&Mailbox>> = BTreeMap::new();
    let mut top: Vec<&Mailbox> = Vec::new();
    for mailbox in folders {
        match drawn_under(mailbox, &known) {
            Some(parent) => children.entry(parent).or_default().push(mailbox),
            None => top.push(mailbox),
        }
    }
    top.sort_by_key(|mailbox| folder_sort_key(mailbox));
    for group in children.values_mut() {
        group.sort_by_key(|mailbox| folder_sort_key(mailbox));
    }

    let mut rows = Vec::with_capacity(folders.len());
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut stack: Vec<(&Mailbox, Option<&str>, u32)> =
        top.into_iter().rev().map(|m| (m, None, 0)).collect();
    while let Some((mailbox, parent, depth)) = stack.pop() {
        let key = mailbox.id.as_str();
        if !seen.insert(key) {
            continue;
        }
        let nested: &[&Mailbox] = children.get(key).map_or(&[], Vec::as_slice);
        rows.push(folder_row(mailbox, parent, depth, !nested.is_empty()));
        // Reversed, because the stack pops last-in first: this is what puts a folder's own
        // children directly beneath it, in their sorted order.
        stack.extend(
            nested
                .iter()
                .rev()
                .map(|child| (*child, Some(key), depth + 1)),
        );
    }

    // A folder the walk never reached is one in a cycle, which no server should be able to
    // report. It still gets a row, at the top: a folder missing from the pane is a folder
    // whose mail the user has no way to open.
    for mailbox in folders {
        if seen.insert(mailbox.id.as_str()) {
            rows.push(folder_row(mailbox, None, 0, false));
        }
    }
    rows
}

/// Each row's name with the folders it sits inside, `Clients / Acme`, in the rows' own order.
///
/// For a list that is **flat** and so cannot show the nesting any other way: the sync-settings
/// push-folder list, where two folders called `2024` would otherwise read as one word each. The
/// pane does not use it; there the indent says where a folder sits.
///
/// Built by walking the rows' own parents, never by splitting a name: the separator is the
/// server's and every adapter has already taken it off, so this repository has no business
/// guessing it back.
///
/// ⚠️ An ancestor that carries a **role** contributes the *server's* word for it, where the row
/// itself would take ours (`docs/folder-pane.md`, rule 12). The two differ only on a server whose
/// special folders are named in another language, and a role-bearing folder is drawn at the top
/// of the pane anyway, so it never has a prefix of its own.
#[must_use]
pub fn folder_paths(rows: &[FolderRow]) -> Vec<String> {
    let by_key: BTreeMap<&str, &FolderRow> =
        rows.iter().map(|row| (row.key.as_str(), row)).collect();
    rows.iter()
        .map(|row| {
            let mut parts = vec![row.name.as_str()];
            let mut parent = row.parent.as_deref();
            // Bounded by the number of rows: a chain longer than that is one that loops, which
            // `sorted_folder_rows` does not produce and no server should report.
            while let Some(key) = parent.filter(|_| parts.len() <= rows.len()) {
                let Some(ancestor) = by_key.get(key) else {
                    break;
                };
                parts.push(ancestor.name.as_str());
                parent = ancestor.parent.as_deref();
            }
            parts.reverse();
            parts.join(" / ")
        })
        .collect()
}

/// The key of the row this folder is **drawn** underneath, which is not always the one the
/// server files it under. `None` puts it at the top of the account's tree.
///
/// Two folders go to the top although their provider nests them:
///
/// - A **role-bearing** folder. Gmail files Sent, Drafts, Trash and All Mail inside a `[Gmail]`
///   container over IMAP, and Exchange has its own arrangements; burying the folder a person
///   reaches for most inside something their provider invented is what no mail client does. We
///   already rename these to our own words (`docs/folder-pane.md`, rule 12), so drawing them where
///   the role says rather than where the path says is the same decision.
/// - An **orphan**, whose parent this account does not list: an unsubscribed or hidden
///   intermediate. It has to go somewhere, and it cannot go under a row that is not there.
fn drawn_under<'a>(mailbox: &'a Mailbox, known: &BTreeSet<&'a str>) -> Option<&'a str> {
    if mailbox.role.as_ref().and_then(role_rank).is_some() {
        return None;
    }
    let parent = mailbox.parent.as_ref()?.as_str();
    known.contains(parent).then_some(parent)
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
