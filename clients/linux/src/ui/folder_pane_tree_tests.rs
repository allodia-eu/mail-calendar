//! The folder tree's GTK contract: what a nested row is drawn as, and what its chevron does.

use adw::prelude::*;
use mailcal_bindings::{AccountFolderRow, FolderRole, FolderRow, MailboxListSnapshot};

use super::{account, folder, pane, rows, shown};
use crate::ui::{AppInput, model::empty_mailbox};

/// A folder filed inside another one, as the core projects it.
fn nested(key: &str, name: &str, parent: Option<&str>, depth: u32) -> FolderRow {
    FolderRow {
        parent: parent.map(str::to_owned),
        depth,
        ..folder(key, name, None, 0)
    }
}

/// One account: an Inbox, an Archive holding `Clients`, and an `Acme` inside that.
fn one_tree(archive_expanded: bool, acme_visible: bool) -> MailboxListSnapshot {
    let mut archive = folder("archive", "Archive", Some(FolderRole::Archive), 0);
    archive.has_children = true;
    archive.expanded = archive_expanded;
    let mut clients = nested("clients", "Clients", Some("archive"), 1);
    clients.has_children = true;
    clients.expanded = acme_visible;
    clients.visible = archive_expanded;
    let mut acme = nested("acme", "Acme", Some("clients"), 2);
    acme.visible = acme_visible;
    MailboxListSnapshot {
        accounts: vec![account("acct-1", "eva.jansen@example.test", true)],
        account_folders: vec![AccountFolderRow {
            account_id: "acct-1".to_owned(),
            manages_folders: false,
            folders: vec![
                folder("inbox", "INBOX", Some(FolderRole::Inbox), 0),
                archive,
                clients,
                acme,
            ],
        }],
        ..empty_mailbox()
    }
}

/// A folder inside a folder is indented under it, and shutting one takes its whole branch off
/// screen rather than only its own children.
pub(crate) fn a_nested_folder_is_indented_and_a_shut_one_is_not_drawn() {
    let (list, _) = pane(&one_tree(true, true));
    let text = shown(&list);
    assert!(text.iter().any(|entry| entry == "Clients"), "{text:?}");
    assert!(text.iter().any(|entry| entry == "Acme"), "{text:?}");

    // One indent step per level, on top of the account's own. Asserted on the row rather than
    // on a number in the source, because the indent is the only thing on screen that says
    // where a folder sits: every adapter names it by its own name alone.
    // The margin is read off the row itself: an `AdwActionRow` **is** a `GtkListBoxRow`, so the
    // widget the list holds is the row, and `child()` would hand back the box inside it.
    let drawn = rows(&list);
    let margin = |index: usize| drawn[index].margin_start();
    // [0] All Accounts, [1] its Inbox, [2] the account, [3] Inbox, [4] Archive, [5] Clients,
    // [6] Acme.
    assert!(margin(5) > margin(4), "Clients sits inside Archive");
    assert!(margin(6) > margin(5), "Acme sits inside Clients");
    assert_eq!(
        margin(6) - margin(5),
        margin(5) - margin(4),
        "one step each"
    );

    // The core says which rows are hidden; the pane must not draw them, and must not count
    // them either, or the selection lands a row off.
    let (shut, _) = pane(&one_tree(false, false));
    let text = shown(&shut);
    assert!(!text.iter().any(|entry| entry == "Clients"), "{text:?}");
    assert!(!text.iter().any(|entry| entry == "Acme"), "{text:?}");
}

/// The chevron on a folder is a control of its own: it reports the toggle and navigates nowhere.
/// A folder holding no folders has no chevron at all.
pub(crate) fn a_folder_chevron_toggles_without_navigating() {
    let (list, receiver) = pane(&one_tree(true, true));
    let drawn = rows(&list);

    // The Archive's own chevron: the trailing button on that row.
    let chevron = super::buttons(drawn[4].upcast_ref::<gtk::Widget>())
        .into_iter()
        .find(|button| {
            button
                .icon_name()
                .is_some_and(|name| name.starts_with("pan-"))
        })
        .expect("a folder holding folders carries a disclosure control");
    chevron.emit_clicked();
    assert!(
        matches!(
            receiver.recv_sync(),
            Some(AppInput::SetFolderExpanded { account, key, expanded })
                if account == "acct-1" && key == "archive" && !expanded
        ),
        "the chevron shuts the folder it sits on, and selects nothing"
    );

    // The Inbox holds no folders, so it must not offer to open one.
    assert!(
        !super::buttons(drawn[3].upcast_ref::<gtk::Widget>())
            .into_iter()
            .any(|button| button
                .icon_name()
                .is_some_and(|name| name.starts_with("pan-"))),
        "a chevron here would promise a tree that is not there"
    );
}
