//! The accounts-and-folders pane: every account's tree, on screen at once.
//!
//! The rules it keeps, and why each is a rule rather than a preference, are in
//! `docs/folder-pane.md`. What this file owns is the GTK half: which widget draws a row, which
//! icon a role takes, and what the row dispatches. Expansion, the counts and the ordering all
//! arrive in the snapshot; the pane keeps no state of its own, which is what makes it agree
//! with the other clients and survive a restart.

use std::collections::HashSet;

use mailcal_bindings::{FolderRow, Intent, MailboxListSnapshot};

use super::{
    AppInput, AppModel, PrimaryView,
    folder_actions::NameCheck,
    folder_names::folder_label,
    folder_pane_edit,
    folder_pane_rows::{account_row, folder_row, unified_group_row, unified_inbox_row},
    mailbox, outbox,
};

/// A navigation target represented by one pane row. Carried by the row's own handler rather than
/// looked up by index: a folder key is unique only *within* its account, and the pane holds every
/// account's tree, so several rows share the key `inbox`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SidebarTarget {
    UnifiedGroup,
    AllInboxes,
    Account(String),
    Folder { account: String, key: String },
}

/// Renders the whole pane, and restores the selection the snapshot reports.
///
/// Every row carries its own handler. The pane used to map a row **index** onto a target computed
/// separately from the rendering, which is two orderings that have to agree; a tree of accounts,
/// each with a folder list that appears and disappears, is not an ordering worth maintaining
/// twice.
pub(crate) fn render(
    list: &gtk::ListBox,
    snapshot: &MailboxListSnapshot,
    unreachable_accounts: &HashSet<String>,
    check: &NameCheck,
    sender: &relm4::Sender<AppInput>,
) {
    mailbox::install_styles();
    mailbox::clear(list);

    // Rule 18: above the account trees, and off screen entirely at zero.
    if !snapshot.outbox.is_empty() {
        list.append(&outbox::pane_row(snapshot.outbox.len(), sender));
    }
    list.append(&unified_group_row(snapshot.unified_expanded, sender));
    if snapshot.unified_expanded {
        list.append(&unified_inbox_row(snapshot.unified_unread, sender));
    }

    for account in &snapshot.accounts {
        let row = account_row(account, unreachable_accounts.contains(&account.id), sender);
        folder_pane_edit::account(
            &row,
            &account.id,
            manages_folders(snapshot, &account.id),
            check,
            sender,
        );
        list.append(&row);
        if !account.expanded {
            continue;
        }
        let folders = folders_of(snapshot, &account.id);
        for folder in drawn_folders(snapshot, &account.id) {
            let row = folder_row(&account.id, folder, sender);
            folder_pane_edit::folder(&row, &account.id, folder, folders, check, sender);
            list.append(&row);
        }
    }

    // After the rows exist, and without notifying: the pane mirrors where the core says we are,
    // and re-reporting that as a click would dispatch a navigation on every refresh.
    select_snapshot_row(list, snapshot);
}

/// Moves the selection among rows already on screen without rebuilding the account trees.
pub(super) fn select_snapshot_row(list: &gtk::ListBox, snapshot: &MailboxListSnapshot) {
    let selected = selected_row_index(snapshot).and_then(|index| list.row_at_index(index));
    match selected.as_ref() {
        Some(row) => list.select_row(Some(row)),
        None => list.unselect_all(),
    }
}

fn selected_row_index(snapshot: &MailboxListSnapshot) -> Option<i32> {
    // The Outbox row when it is drawn, which also shifts every row beneath it down by one.
    let outbox = usize::from(!snapshot.outbox.is_empty());
    // Neither selection scalar can say this: both are `None` on the Outbox *and* on the unified
    // inbox, so a pane that guessed would highlight All Inboxes while the Outbox is on screen.
    if snapshot.showing_outbox {
        return (outbox == 1).then_some(0);
    }
    let Some(selected_account) = snapshot.selected_account.as_deref() else {
        return snapshot
            .unified_expanded
            .then(|| i32::try_from(outbox + 1).ok())
            .flatten();
    };
    let mut index = outbox + 1 + usize::from(snapshot.unified_expanded);
    for account in &snapshot.accounts {
        if account.id == selected_account {
            let Some(selected_folder) = snapshot.selected.as_deref() else {
                return i32::try_from(index).ok();
            };
            if !account.expanded {
                return None;
            }
            return drawn_folders(snapshot, &account.id)
                .position(|folder| folder.key == selected_folder)
                .and_then(|position| i32::try_from(index + 1 + position).ok());
        }
        index += 1;
        if account.expanded {
            index += drawn_folders(snapshot, &account.id).count();
        }
    }
    None
}

/// One account's folders. **`account_folders`, never `folders`**: the latter holds the selected
/// account's alone, so rendering it is what emptied the pane on All Inboxes.
///
/// Visible to the UI module because the search filter names the folder it would narrow to, and it
/// has to be the same row this pane drew.
pub(crate) fn folders_of<'a>(snapshot: &'a MailboxListSnapshot, account: &str) -> &'a [FolderRow] {
    snapshot
        .account_folders
        .iter()
        .find(|row| row.account_id == account)
        .map_or(&[], |row| row.folders.as_slice())
}

/// Whether `account`'s folders can be changed at all.
fn manages_folders(snapshot: &MailboxListSnapshot, account: &str) -> bool {
    snapshot
        .account_folders
        .iter()
        .any(|row| row.account_id == account && row.manages_folders)
}

/// The folders this account actually puts on screen: not the ones inside a folder the user shut.
///
/// The core has already walked the chain of parents, so this is a filter rather than a climb back
/// up it. **Every** count of the pane's rows goes through here, the render and the selection
/// index alike: two orderings that disagree is what puts the highlight on the wrong row.
fn drawn_folders<'a>(
    snapshot: &'a MailboxListSnapshot,
    account: &str,
) -> impl Iterator<Item = &'a FolderRow> {
    folders_of(snapshot, account)
        .iter()
        .filter(|folder| folder.visible)
}

/// The pane's width bounds, and the clamp that applies them.
///
/// The floor keeps it a folder tree rather than a column of ellipses; the ceiling stops one drag
/// from taking the window. `available` is what the window has to divide, so the mail beside the
/// pane keeps a minimum whatever the user drags; and when the two floors cannot both be met, the
/// pane takes its floor rather than disappearing.
pub(crate) mod width {
    /// The narrowest the pane may be dragged.
    pub(crate) const MIN: i32 = 200;
    /// The widest, before the window's own size has a say.
    pub(crate) const MAX: i32 = 560;
    /// What must be left for the mail beside it.
    pub(crate) const MIN_CONTENT: i32 = 480;
    /// The width a pane nobody has dragged opens at.
    pub(crate) const DEFAULT: i32 = 240;

    /// `width` brought within the bounds a window of `available` pixels allows.
    pub(crate) fn clamp(width: i32, available: i32) -> i32 {
        let ceiling = MAX.min(available - MIN_CONTENT);
        if ceiling < MIN {
            return MIN;
        }
        width.clamp(MIN, ceiling)
    }
}

/// Only the snapshot fields the pane draws; the pane is rebuilt when this changes, so a field a
/// row shows and this omits leaves a stale row on screen. Expansion and the counts are in it
/// precisely because they change without any row's text changing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct FolderPaneRendering {
    accounts: Vec<RenderedAccount>,
    unified_unread: u32,
    unified_expanded: bool,
    /// How many messages are waiting to be sent: the Outbox row's badge, and whether that row
    /// is drawn at all. Its own field for the reason the counts are: the row appears, changes
    /// and goes away without any account's text changing.
    queued: usize,
}

/// Applies a snapshot selection once, leaving GTK's optimistic row mark alone until it changes.
#[derive(Default)]
pub(crate) struct FolderPaneSelection {
    rendered: Option<(Option<String>, Option<String>, bool)>,
}

impl FolderPaneSelection {
    pub(super) fn sync(&mut self, list: &gtk::ListBox, snapshot: &MailboxListSnapshot) {
        // `showing_outbox` is part of the key because the two scalars beside it cannot tell the
        // Outbox from the unified inbox: both are `None` in each case, so without it opening the
        // Outbox is "no change" and the highlight stays on All Inboxes.
        let next = (
            snapshot.selected_account.clone(),
            snapshot.selected.clone(),
            snapshot.showing_outbox,
        );
        if self.rendered.as_ref() == Some(&next) {
            return;
        }
        select_snapshot_row(list, snapshot);
        self.rendered = Some(next);
    }
}

/// One account as the pane draws it, with the tree it is showing.
#[derive(Clone, Debug, PartialEq, Eq)]
struct RenderedAccount {
    id: String,
    email: String,
    expanded: bool,
    unreachable: bool,
    manages_folders: bool,
    folders: Vec<RenderedFolder>,
}

/// One folder as the pane draws it: the name the **user** reads, not the server's, and where in
/// the tree the row sits.
///
/// The tree fields are here for the reason the counts are: each of them changes what is on
/// screen, or which way a chevron points, without any row's text changing at all.
#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "each flag is a separate fact the row draws, not the state of a state machine"
)]
struct RenderedFolder {
    key: String,
    label: String,
    unread: u32,
    depth: u32,
    has_children: bool,
    expanded: bool,
    visible: bool,
    /// What the row offers and how it looks while waiting: each changes the row's menu, its
    /// drag and drop, or its style without changing its text.
    parent: Option<String>,
    pending: bool,
    in_trash: bool,
    editable: bool,
    accepts_folders: bool,
    accepts_messages: bool,
}

impl FolderPaneRendering {
    pub(super) fn new(
        snapshot: &MailboxListSnapshot,
        unreachable_accounts: &HashSet<String>,
    ) -> Self {
        Self {
            accounts: snapshot
                .accounts
                .iter()
                .map(|account| RenderedAccount {
                    id: account.id.clone(),
                    email: account.email.clone(),
                    expanded: account.expanded,
                    unreachable: unreachable_accounts.contains(&account.id),
                    manages_folders: manages_folders(snapshot, &account.id),
                    folders: folders_of(snapshot, &account.id)
                        .iter()
                        .map(|folder| RenderedFolder {
                            key: folder.key.clone(),
                            label: folder_label(folder.role.as_ref(), &folder.name),
                            unread: folder.unread,
                            depth: folder.depth,
                            has_children: folder.has_children,
                            expanded: folder.expanded,
                            visible: folder.visible,
                            parent: folder.parent.clone(),
                            pending: folder.pending,
                            in_trash: folder.in_trash,
                            editable: folder.editable,
                            accepts_folders: folder.accepts_folders,
                            accepts_messages: folder.accepts_messages,
                        })
                        .collect(),
                })
                .collect(),
            unified_unread: snapshot.unified_unread,
            unified_expanded: snapshot.unified_expanded,
            queued: snapshot.outbox.len(),
        }
    }
}

impl AppModel {
    /// Navigates to what a folder-pane row points at. Never touches expansion: opening a tree is
    /// not navigating, and every tree that was open stays open.
    pub(super) fn activate_sidebar(&mut self, target: &SidebarTarget) {
        // A pane row is a mail destination, so it takes the primary view back from the calendar.
        self.primary = PrimaryView::Mail;
        match target {
            SidebarTarget::UnifiedGroup => self.dispatch(Intent::SetUnifiedExpanded {
                expanded: !self.snapshot.unified_expanded,
            }),
            SidebarTarget::AllInboxes => self.dispatch(Intent::SelectAccount { account: None }),
            SidebarTarget::Account(account) => self.select_account(account),
            // A folder tap names its account, in one intent: every account's tree is on
            // screen, and a folder key is unique only within its account, so the key alone
            // would be resolved against whichever account happened to be selected; or against
            // none at all from All Inboxes (docs/folder-pane.md, rule 14).
            SidebarTarget::Folder { account, key } => self.dispatch(Intent::SelectFolder {
                account: account.clone(),
                key: key.clone(),
            }),
        }
    }

    fn select_account(&self, account: &str) {
        self.dispatch(Intent::SelectAccount {
            account: Some(account.to_owned()),
        });
    }
}

#[cfg(test)]
#[path = "folder_pane_tests.rs"]
pub(super) mod tests;
