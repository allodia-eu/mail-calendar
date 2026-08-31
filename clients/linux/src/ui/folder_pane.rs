//! The accounts-and-folders pane: every account's tree, on screen at once.
//!
//! The rules it keeps, and why each is a rule rather than a preference, are in
//! `docs/folder-pane.md`. What this file owns is the GTK half: which widget draws a row, which
//! icon a role takes, and what the row dispatches. Expansion, the counts and the ordering all
//! arrive in the snapshot; the pane keeps no state of its own, which is what makes it agree
//! with the other clients and survive a restart.

use std::collections::HashSet;

use adw::prelude::*;
use gtk::accessible::Property as AccessibleProperty;
use mailcal_bindings::{AccountRow, FolderRole, FolderRow, Intent, MailboxListSnapshot};

use super::{AppInput, AppModel, PrimaryView, mailbox, row_action};
use crate::l10n;

/// A navigation target represented by one pane row. Carried by the row's own handler rather than
/// looked up by index: a folder key is unique only *within* its account, and the pane holds every
/// account's tree, so several rows share the key `inbox`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SidebarTarget {
    AllInboxes,
    Account(String),
    Folder { account: String, key: String },
}

/// What a folder is **called** on screen: the app's own word for a role-bearing folder, the
/// server's name for everything else.
///
/// The server's name for a special folder is not a name the user chose; it is whatever their
/// provider stores, in whatever language and casing it likes: `INBOX` shouting in capitals (the
/// one name IMAP mandates), `Deleted Items` from Exchange, `[Gmail]/Sent Mail`. Naming them
/// ourselves is also what makes the folder list follow the **app's** language.
///
/// [`FolderRole::Other`] keeps the server's name deliberately: the core collapses flagged,
/// important and all-mail into that one value, so any single word for it would rename three
/// different folders to the same thing.
///
/// Public to the UI module because the pane is not the only place a folder is named; the list
/// header and the sync-settings folder list show one too, and a folder called two things in one
/// app is worse than one called something odd in both.
pub(crate) fn folder_label(role: Option<&FolderRole>, name: &str) -> String {
    match role {
        Some(FolderRole::Inbox) => l10n::folder_inbox().to_owned(),
        Some(FolderRole::Drafts) => l10n::folder_drafts().to_owned(),
        Some(FolderRole::Sent) => l10n::folder_sent().to_owned(),
        Some(FolderRole::Archive) => l10n::folder_archive().to_owned(),
        Some(FolderRole::Junk) => l10n::folder_junk().to_owned(),
        Some(FolderRole::Trash) => l10n::folder_trash().to_owned(),
        Some(FolderRole::Other) | None => name.to_owned(),
    }
}

/// What the mail list's header calls the scope on screen: the unified inbox, the selected folder
/// by the app's own name for it, or the account's whole mailbox.
///
/// Here rather than in the shell because the pane is not the only place a folder is named, and one
/// function every site calls is what stops the header and the tree disagreeing (rule 13).
pub(crate) fn header_title(snapshot: &MailboxListSnapshot) -> String {
    let Some(account) = snapshot.selected_account.as_deref() else {
        return l10n::sidebar_all_inboxes().to_owned();
    };
    let Some(key) = snapshot.selected.as_deref() else {
        return l10n::sidebar_all_mail().to_owned();
    };
    folders_of(snapshot, account)
        .iter()
        .find(|folder| folder.key == key)
        .map_or_else(
            // A key with no row behind it: the folder list has moved on (a rename, a sync) and
            // the header would otherwise name a folder that is no longer there.
            || l10n::folder_fallback().to_owned(),
            |folder| folder_label(folder.role.as_ref(), &folder.name),
        )
}

/// The symbolic icon for a folder's special role; a plain folder for anything without one.
///
/// Keyed on the role the core resolves (RFC 6154 SPECIAL-USE / JMAP), never on the folder's name:
/// the name is whatever the server calls it, so a name test picks the wrong icon in six of the
/// seven shipped languages, and on any server whose folders were renamed.
///
/// Inbox and Archive are **ours** (`mailcal-*`); the rest are the desktop's own. Adwaita: the
/// theme the GNOME runtime provides, and so the one the Flatpak actually runs against; ships
/// neither `mail-inbox-symbolic` nor `mail-archive-symbolic`, and a name the theme does not have
/// draws the broken-image icon while the pane carries on as though nothing happened. Yaru has
/// both, which is exactly what would have made this look fine on the machine it was written on.
fn role_icon(role: Option<&FolderRole>) -> &'static str {
    match role {
        Some(FolderRole::Inbox) => INBOX_ICON,
        Some(FolderRole::Drafts) => "document-edit-symbolic",
        Some(FolderRole::Sent) => "mail-send-symbolic",
        Some(FolderRole::Archive) => "mailcal-archive-symbolic",
        Some(FolderRole::Junk) => "mail-mark-junk-symbolic",
        Some(FolderRole::Trash) => "user-trash-symbolic",
        // A role we recognise but draw no distinct icon for (flagged / all / important), and
        // every ordinary custom folder, take the plain folder.
        Some(FolderRole::Other) | None => "folder-symbolic",
    }
}

/// The tray, on All Inboxes and on every account's Inbox; the same glyph for both, because the
/// unified row *is* those inboxes summed.
const INBOX_ICON: &str = "mailcal-inbox-symbolic";

/// The account row's icon. Its own person glyph rather than a mail one, so an account reads as a
/// heading over its folders rather than as another folder among them.
const ACCOUNT_ICON: &str = "avatar-default-symbolic";

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
    sender: &relm4::Sender<AppInput>,
) {
    mailbox::install_styles();
    mailbox::clear(list);

    let unified = unified_row(sender);
    unified.set_title(l10n::sidebar_all_inboxes());
    add_badge(&unified, snapshot.unified_unread);
    list.append(&unified);

    for account in &snapshot.accounts {
        let row = account_row(account, unreachable_accounts.contains(&account.id), sender);
        list.append(&row);
        if !account.expanded {
            continue;
        }
        for folder in folders_of(snapshot, &account.id) {
            let row = folder_row(&account.id, folder, sender);
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
    let Some(selected_account) = snapshot.selected_account.as_deref() else {
        return Some(0);
    };
    let mut index = 1usize;
    for account in &snapshot.accounts {
        if account.id == selected_account {
            let Some(selected_folder) = snapshot.selected.as_deref() else {
                return i32::try_from(index).ok();
            };
            if !account.expanded {
                return None;
            }
            return folders_of(snapshot, &account.id)
                .iter()
                .position(|folder| folder.key == selected_folder)
                .and_then(|position| i32::try_from(index + 1 + position).ok());
        }
        index += 1;
        if account.expanded {
            index += folders_of(snapshot, &account.id).len();
        }
    }
    None
}

/// One account's folders. **`account_folders`, never `folders`**: the latter holds the selected
/// account's alone, so rendering it is what emptied the pane on All Inboxes.
///
/// Visible to the UI module because the search filter names the folder it would narrow to, and it
/// has to be the same row this pane drew.
pub(super) fn folders_of<'a>(snapshot: &'a MailboxListSnapshot, account: &str) -> &'a [FolderRow] {
    snapshot
        .account_folders
        .iter()
        .find(|row| row.account_id == account)
        .map_or(&[], |row| row.folders.as_slice())
}

fn unified_row(sender: &relm4::Sender<AppInput>) -> adw::ActionRow {
    let row = pane_row(INBOX_ICON, sender, &SidebarTarget::AllInboxes);
    row.set_tooltip_text(Some(l10n::sidebar_all_inboxes()));
    row
}

/// An account: its address, a chevron that opens its tree, and **no count**; the counts belong to
/// the folders, and a roll-up here would sit directly above an identical number on the Inbox row
/// beneath it.
fn account_row(
    account: &AccountRow,
    unreachable: bool,
    sender: &relm4::Sender<AppInput>,
) -> adw::ActionRow {
    let row = pane_row(
        ACCOUNT_ICON,
        sender,
        &SidebarTarget::Account(account.id.clone()),
    );
    row.set_title(&account.email);
    // An address is as long as it is, and the pane has a floor: the row that gets truncated is
    // precisely the one the user needs to read.
    row.set_tooltip_text(Some(&account.email));
    if unreachable {
        let warning = gtk::Image::from_icon_name("dialog-warning-symbolic");
        warning.add_css_class("warning");
        warning.set_tooltip_text(Some(l10n::connectivity_account_unreachable()));
        warning.update_property(&[AccessibleProperty::Label(
            l10n::connectivity_account_unreachable(),
        )]);
        row.add_suffix(&warning);
    }
    row.add_suffix(&chevron(account, sender));
    row
}

/// The disclosure control, a button of its own rather than the row.
///
/// Opening a tree is not navigating (`docs/folder-pane.md` rule 2), so it must not move the
/// selection: which is exactly what activating the row does. A `GtkButton` inside the row
/// consumes its own click, so the two gestures stay separate.
fn chevron(account: &AccountRow, sender: &relm4::Sender<AppInput>) -> gtk::Button {
    let button = gtk::Button::from_icon_name(if account.expanded {
        "pan-down-symbolic"
    } else {
        "pan-end-symbolic"
    });
    button.add_css_class("flat");
    button.set_valign(gtk::Align::Center);
    let spoken = if account.expanded {
        l10n::a11y_collapse_account()
    } else {
        l10n::a11y_expand_account()
    };
    button.set_tooltip_text(Some(spoken));
    button.update_property(&[AccessibleProperty::Label(spoken)]);
    let input = sender.clone();
    let id = account.id.clone();
    let expanded = account.expanded;
    button.connect_clicked(move |_| {
        input.emit(AppInput::SetAccountExpanded {
            account: id.clone(),
            expanded: !expanded,
        });
    });
    button
}

fn folder_row(
    account: &str,
    folder: &FolderRow,
    sender: &relm4::Sender<AppInput>,
) -> adw::ActionRow {
    let row = pane_row(
        role_icon(folder.role.as_ref()),
        sender,
        &SidebarTarget::Folder {
            account: account.to_owned(),
            key: folder.key.clone(),
        },
    );
    row.set_title(&folder_label(folder.role.as_ref(), &folder.name));
    // Under its account, so the tree reads as a tree. A margin rather than a nested list: the
    // rows stay siblings, which is what keeps one keyboard traversal over the whole pane.
    row.set_margin_start(18);
    add_badge(&row, folder.unread);
    row
}

/// The shared skeleton: one line, an icon, the whole row activatable.
fn pane_row(
    icon: &str,
    sender: &relm4::Sender<AppInput>,
    target: &SidebarTarget,
) -> adw::ActionRow {
    // A folder's name and an account's address are the server's text: a bare ampersand must not
    // be read as an entity, and a markup-shaped name must render as itself.
    let row = mailbox::plain_text_row();
    row.set_title_lines(1);
    row.set_activatable(true);
    row.add_prefix(&gtk::Image::from_icon_name(icon));
    let input = sender.clone();
    let target = target.clone();
    row_action::action_row(&row, move || {
        input.emit(AppInput::ActivateSidebar(target.clone()));
    });
    row
}

/// The unread count at the trailing edge; **nothing at zero**, which also covers a provider that
/// reports no count at all (Gmail today). A badge reading `0` would claim we looked and found
/// nothing.
fn add_badge(row: &adw::ActionRow, unread: u32) {
    if unread == 0 {
        return;
    }
    let label = mailbox::badge(&unread.to_string());
    // The bare number reads as a position in a list; the spoken label says what it counts.
    let spoken = l10n::a11y_unread_count(i64::from(unread));
    label.set_tooltip_text(Some(&spoken));
    label.update_property(&[AccessibleProperty::Label(&spoken)]);
    row.add_suffix(&label);
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
}

/// Applies a snapshot selection once, leaving GTK's optimistic row mark alone until it changes.
#[derive(Default)]
pub(super) struct FolderPaneSelection {
    rendered: Option<(Option<String>, Option<String>)>,
}

impl FolderPaneSelection {
    pub(super) fn sync(&mut self, list: &gtk::ListBox, snapshot: &MailboxListSnapshot) {
        let next = (snapshot.selected_account.clone(), snapshot.selected.clone());
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
    folders: Vec<RenderedFolder>,
}

/// One folder as the pane draws it: the name the **user** reads, not the server's.
#[derive(Clone, Debug, PartialEq, Eq)]
struct RenderedFolder {
    key: String,
    label: String,
    unread: u32,
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
                    folders: folders_of(snapshot, &account.id)
                        .iter()
                        .map(|folder| RenderedFolder {
                            key: folder.key.clone(),
                            label: folder_label(folder.role.as_ref(), &folder.name),
                            unread: folder.unread,
                        })
                        .collect(),
                })
                .collect(),
            unified_unread: snapshot.unified_unread,
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
