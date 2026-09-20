//! The folder-pane records: what a folder is, where it sits in its account's tree, and the
//! per-account lists a pane draws.
//!
//! Their own file because `records.rs` was over the 500-line limit once a folder row carried its
//! place in the tree as well as its name. `lib.rs` re-exports them, so the split is invisible to
//! a host. The pane's rules are `docs/folder-pane.md`.

// Named only by an intra-doc link below, which rustdoc resolves against this module.
#[allow(unused_imports)]
use crate::records::AccountRow;

/// The special role a folder plays (RFC 6154 SPECIAL-USE / JMAP equivalent), exposed on
/// [`FolderRow`] so a client can badge or group well-known folders without name heuristics.
#[derive(uniffi::Enum)]
pub enum FolderRole {
    /// The primary inbox.
    Inbox,
    /// Drafts; messages in-progress.
    Drafts,
    /// Sent; copies of sent messages.
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
///
/// The rows arrive **depth-first**, each folder immediately followed by the folders inside it,
/// so a flat list drawn in order with `depth` indent steps is already the tree
/// (`docs/folder-pane.md`).
#[derive(uniffi::Record)]
pub struct FolderRow {
    /// The mailbox's provider key (used to select it).
    pub key: String,
    /// The folder's display name: the folder's **own** name, never the path to it. So two
    /// folders called `2024` read alike, and the indent is what says which Archive each is in.
    pub name: String,
    /// The folder's special role, or `None` for an ordinary custom folder.
    pub role: Option<FolderRole>,
    /// How many messages in the folder are unread, as the **server** counts them: so it
    /// covers mail older than the synced window. **Show no badge at `0`**: zero folds
    /// together "nothing unread" and "this provider reports no count", and both must
    /// render as nothing (`docs/folder-pane.md`).
    pub unread: u32,
    /// The `key` of the row this one is drawn underneath, or `None` at the top of the tree.
    ///
    /// For a pane whose framework nests rows of its own: attach the row to the item carrying
    /// this key. A pane that draws a flat list wants `depth` instead.
    pub parent: Option<String>,
    /// How many indent steps in the row sits: `0` at the top, `1` for a folder filed inside
    /// one of those, and so on.
    pub depth: u32,
    /// Whether any other row names this one as its `parent`.
    ///
    /// **Draw a disclosure control only where this is true.** A folder holding only mail has
    /// no tree to open, and a chevron beside it promises one.
    pub has_children: bool,
    /// Whether the folders inside this one are showing. Always `false` where `has_children`
    /// is: there is nothing to be open.
    ///
    /// The chevron's direction. The core owns and persists it; keep no copy, and change it
    /// with `Intent::SetFolderExpanded` (`docs/folder-pane.md`, rule 3).
    pub expanded: bool,
    /// Whether the row belongs on screen at all: `false` while some folder above it in the
    /// chain is shut.
    ///
    /// **Skip a row where this is false.** The core has already walked the chain, so a pane
    /// drawing a flat list never has to. A pane whose framework nests rows and hides a shut
    /// item's children for it ignores this and uses `parent` and `expanded`.
    pub visible: bool,
}

/// One account's sorted folder list, for the navigation drawer that shows all accounts at once.
#[derive(uniffi::Record)]
pub struct AccountFolderRow {
    /// The account's stable id, matching [`AccountRow::id`].
    pub account_id: String,
    /// The account's sorted folder rows, ready for display.
    pub folders: Vec<FolderRow>,
}
