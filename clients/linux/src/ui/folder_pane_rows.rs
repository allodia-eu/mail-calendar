//! The widgets the folder pane is built from: one constructor per kind of row, plus the icon a
//! role takes.
//!
//! Split from `folder_pane`, which decides which rows exist and in what order, so both stay
//! under the repo line limit. The rules are `docs/folder-pane.md`.

use adw::prelude::*;
use gtk::accessible::Property as AccessibleProperty;
use mailcal_bindings::{AccountRow, FolderRole, FolderRow};

use super::{
    AppInput, folder_names::folder_label, folder_pane::SidebarTarget, mailbox, row_action,
};
use crate::{l10n, ui::icons};

/// The symbolic icon for a folder's special role; a plain folder for anything without one.
///
/// Keyed on the role the core resolves (RFC 6154 SPECIAL-USE / JMAP), never on the folder's name:
/// the name is whatever the server calls it, so a name test picks the wrong icon in six of the
/// seven shipped languages, and on any server whose folders were renamed.
///
/// Inbox and Archive are bundled (`mailcal-*`), because Adwaita, the theme the GNOME runtime
/// provides, ships neither; the rest are the desktop's own. Yaru has both, which is exactly what
/// makes a themed name for either look fine on an Ubuntu desktop and broken in the Flatpak.
pub(super) fn role_icon(role: Option<&FolderRole>) -> &'static str {
    match role {
        Some(FolderRole::Inbox) => icons::INBOX,
        Some(FolderRole::Drafts) => icons::DRAFTS,
        Some(FolderRole::Sent) => icons::SENT,
        Some(FolderRole::Archive) => icons::ARCHIVE,
        Some(FolderRole::Junk) => icons::JUNK,
        Some(FolderRole::Trash) => icons::TRASH,
        // A role we recognise but draw no distinct icon for (flagged / all / important), and
        // every ordinary custom folder, take the plain folder.
        Some(FolderRole::Other) | None => icons::FOLDER,
    }
}

pub(super) fn unified_inbox_row(unread: u32, sender: &relm4::Sender<AppInput>) -> adw::ActionRow {
    let row = pane_row(icons::INBOX, sender, &SidebarTarget::AllInboxes);
    row.set_title(l10n::folder_inbox());
    row.set_margin_start(INDENT);
    add_badge(&row, unread);
    row
}

pub(super) fn unified_group_row(
    expanded: bool,
    sender: &relm4::Sender<AppInput>,
) -> adw::ActionRow {
    let row = mailbox::plain_text_row();
    row.set_title(l10n::sidebar_all_accounts());
    row.set_title_lines(1);
    row.set_activatable(true);
    let button = gtk::Button::from_icon_name(if expanded {
        icons::EXPANDED
    } else {
        icons::COLLAPSED
    });
    button.add_css_class("flat");
    button.set_valign(gtk::Align::Center);
    let spoken = if expanded {
        l10n::a11y_collapse_account()
    } else {
        l10n::a11y_expand_account()
    };
    button.set_tooltip_text(Some(spoken));
    button.update_property(&[AccessibleProperty::Label(l10n::sidebar_all_accounts())]);
    let input = sender.clone();
    button.connect_clicked(move |_| {
        input.emit(AppInput::ActivateSidebar(SidebarTarget::UnifiedGroup));
    });
    row.add_prefix(&button);
    row.set_activatable_widget(Some(&button));
    row
}

/// An account: its address, a chevron that opens its tree, and **no count**; the counts belong to
/// the folders, and a roll-up here would sit directly above an identical number on the Inbox row
/// beneath it.
pub(super) fn account_row(
    account: &AccountRow,
    unreachable: bool,
    sender: &relm4::Sender<AppInput>,
) -> adw::ActionRow {
    let row = pane_row(
        icons::ACCOUNT,
        sender,
        &SidebarTarget::Account(account.id.clone()),
    );
    row.set_title(&account.email);
    // An address is as long as it is, and the pane has a floor: the row that gets truncated is
    // precisely the one the user needs to read.
    row.set_tooltip_text(Some(&account.email));
    if unreachable {
        let warning = gtk::Image::from_icon_name(icons::WARNING);
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
        icons::EXPANDED
    } else {
        icons::COLLAPSED
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

pub(super) fn folder_row(
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
    // Under its account, and one step further per folder it is filed inside, so the tree reads
    // as a tree. A margin rather than a nested list: the rows stay siblings, which is what keeps
    // one keyboard traversal over the whole pane.
    row.set_margin_start(INDENT * (1 + i32::try_from(folder.depth).unwrap_or(1)));
    add_badge(&row, folder.unread);
    // Only where there is a tree to open: a chevron beside a folder holding nothing but mail
    // promises one that is not there.
    if folder.has_children {
        row.add_suffix(&folder_chevron(account, folder, sender));
    }
    row
}

/// One step of the pane's indent: what an account's own rows are moved by, and what each level of
/// folders inside folders adds again.
const INDENT: i32 = 18;

/// The disclosure control on a folder that holds folders: a button of its own, for the reason the
/// account row's chevron is one.
///
/// Opening a folder to see what is inside it is not opening its mail, so this must not move the
/// selection, which is exactly what activating the row does. A `GtkButton` inside the row consumes
/// its own click, so the two gestures stay separate.
fn folder_chevron(
    account: &str,
    folder: &FolderRow,
    sender: &relm4::Sender<AppInput>,
) -> gtk::Button {
    let button = gtk::Button::from_icon_name(if folder.expanded {
        icons::EXPANDED
    } else {
        icons::COLLAPSED
    });
    button.add_css_class("flat");
    button.set_valign(gtk::Align::Center);
    let spoken = if folder.expanded {
        l10n::a11y_collapse_folder()
    } else {
        l10n::a11y_expand_folder()
    };
    button.set_tooltip_text(Some(spoken));
    button.update_property(&[AccessibleProperty::Label(spoken)]);
    let input = sender.clone();
    let account = account.to_owned();
    let key = folder.key.clone();
    let expanded = folder.expanded;
    button.connect_clicked(move |_| {
        input.emit(AppInput::SetFolderExpanded {
            account: account.clone(),
            key: key.clone(),
            expanded: !expanded,
        });
    });
    button
}

/// The shared skeleton: one line, an icon, the whole row activatable.
pub(super) fn pane_row(
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
