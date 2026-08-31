//! What the folder pane must draw, and what it must never draw.
//!
//! The GTK halves are called from the crate's single `gtk::init` test (see
//! [`super::super::mailbox::tests`]); the rest are plain unit tests.

use std::collections::HashSet;

use adw::prelude::*;
use mailcal_bindings::{AccountFolderRow, AccountRow, FolderRole, FolderRow, MailboxListSnapshot};

use super::{
    FolderPaneRendering, FolderPaneSelection, SidebarTarget, folder_label, header_title, render,
    role_icon, select_snapshot_row, width,
};
use crate::{
    l10n,
    ui::{AppInput, mailbox::tests::rendered_labels, model::empty_mailbox},
};

fn folder(key: &str, name: &str, role: Option<FolderRole>, unread: u32) -> FolderRow {
    FolderRow {
        key: key.to_owned(),
        name: name.to_owned(),
        role,
        unread,
    }
}

fn account(id: &str, email: &str, expanded: bool) -> AccountRow {
    AccountRow {
        id: id.to_owned(),
        email: email.to_owned(),
        expanded,
    }
}

/// Two accounts, each with a tree, and neither of them the selected one; the shape the pane got
/// wrong everywhere before this contract: a folder key is unique only *within* an account, so
/// both trees hold an `inbox`.
fn two_accounts() -> MailboxListSnapshot {
    MailboxListSnapshot {
        accounts: vec![
            account("acct-1", "eva.jansen@example.test", true),
            account("acct-2", "Research & Development", true),
        ],
        account_folders: vec![
            AccountFolderRow {
                account_id: "acct-1".to_owned(),
                folders: vec![
                    folder("inbox", "INBOX", Some(FolderRole::Inbox), 545),
                    folder("sent", "Sent Items", Some(FolderRole::Sent), 0),
                    folder("custom", "Sales & Marketing", None, 3),
                ],
            },
            AccountFolderRow {
                account_id: "acct-2".to_owned(),
                folders: vec![folder("inbox", "INBOX", Some(FolderRole::Inbox), 7)],
            },
        ],
        unified_unread: 552,
        ..empty_mailbox()
    }
}

fn pane_with_unreachable(
    snapshot: &MailboxListSnapshot,
    unreachable: &HashSet<String>,
) -> (gtk::ListBox, relm4::Receiver<AppInput>) {
    let list = gtk::ListBox::new();
    let (sender, receiver) = relm4::channel::<AppInput>();
    render(&list, snapshot, unreachable, &sender);
    (list, receiver)
}

fn pane(snapshot: &MailboxListSnapshot) -> (gtk::ListBox, relm4::Receiver<AppInput>) {
    pane_with_unreachable(snapshot, &HashSet::new())
}

fn rows(list: &gtk::ListBox) -> Vec<gtk::ListBoxRow> {
    let mut found = Vec::new();
    let mut child = list.first_child();
    while let Some(node) = child {
        if let Some(row) = node.downcast_ref::<gtk::ListBoxRow>() {
            found.push(row.clone());
        }
        child = node.next_sibling();
    }
    found
}

/// The whole pane's text, row by row; what is actually on screen, not what we asked for.
fn shown(list: &gtk::ListBox) -> Vec<String> {
    rows(list)
        .iter()
        .flat_map(|row| rendered_labels(row.upcast_ref::<gtk::Widget>()))
        .filter(|text| !text.is_empty())
        .collect()
}

/// Every account's folders, at once, with the app's own names and the server's counts; and no
/// badge where there is nothing truthful to show.
pub(crate) fn the_pane_draws_every_account_its_folders_and_its_counts() {
    let snapshot = two_accounts();
    let (list, _receiver) = pane(&snapshot);
    let text = shown(&list);

    // Rule 1: the second account's tree is on screen even though neither is selected.
    assert_eq!(
        text.iter().filter(|entry| *entry == "Inbox").count(),
        2,
        "both accounts' inboxes must be on screen at once: {text:?}"
    );
    // Rule 12: the app's word, not the server's: `INBOX` and `Sent Items` are gone.
    assert!(
        !text
            .iter()
            .any(|entry| entry == "INBOX" || entry == "Sent Items"),
        "a role-bearing folder takes the app's name: {text:?}"
    );
    // …and a folder the user made keeps the name the user gave it.
    assert!(
        text.iter().any(|entry| entry == "Sales & Marketing"),
        "a custom folder keeps the server's name: {text:?}"
    );
    // Rules 5 and 7: the folder's own count, and the unified row's sum of the inboxes.
    assert!(text.iter().any(|entry| entry == "545"), "{text:?}");
    assert!(text.iter().any(|entry| entry == "552"), "{text:?}");
    // Rule 6: nothing at zero. Sent is the only row with a zero count, so a stray "0" could
    // only have come from it.
    assert!(
        !text.iter().any(|entry| entry == "0"),
        "a zero count draws no badge at all: {text:?}"
    );

    // Rule 4: a shut account takes its folders off screen, and leaves its neighbour's alone.
    let mut collapsed = two_accounts();
    collapsed.accounts[0].expanded = false;
    let (list, _receiver) = pane(&collapsed);
    let text = shown(&list);
    assert_eq!(
        text.iter().filter(|entry| *entry == "Inbox").count(),
        1,
        "only the open account's folders are drawn: {text:?}"
    );
    assert!(
        !text.iter().any(|entry| entry == "Sales & Marketing"),
        "a shut tree takes every one of its folders with it: {text:?}"
    );
    assert!(
        text.iter().any(|entry| entry == "eva.jansen@example.test"),
        "the account itself stays: {text:?}"
    );
}

/// An address and a folder name are the server's text: a bare ampersand must reach the screen,
/// and must not be parsed on the way there.
///
/// The rendering assertions cannot see the second half. A row built as
/// `.title(…).use_markup(false)` still reads back correctly: libadwaita re-applies the labels
/// when the flag flips: but has already logged a `Failed to set text … from markup` warning into
/// the diagnostic log a user attaches to a support request. The warning is the only observable.
pub(crate) fn a_server_named_row_is_never_parsed_as_markup() {
    let (list, records) = crate::ui::mailbox::tests::glib_records(|| pane(&two_accounts()).0);
    assert!(
        !records.iter().any(|line| line.contains("from markup")),
        "no row may parse the server's text as markup: {records:?}"
    );
    let text = shown(&list);
    assert!(
        text.iter().any(|entry| entry == "Research & Development"),
        "an address with an ampersand must render in full: {text:?}"
    );

    let mut hostile = two_accounts();
    hostile.account_folders[0].folders[2].name = "<b>Wire transfer</b>".to_owned();
    let (list, _receiver) = pane(&hostile);
    let text = shown(&list);
    assert!(
        text.iter().any(|entry| entry == "<b>Wire transfer</b>"),
        "a markup-shaped folder name is shown, never applied: {text:?}"
    );
}

/// The bundled glyphs the pane needs, and the themed ones it relies on the desktop for.
///
/// A name the icon theme does not have is not an error; GTK draws the broken-image icon and the
/// pane carries on; so the only way to know is to ask. Adwaita is the theme the GNOME runtime
/// provides, and it has no inbox and no archive glyph, which is why those two are ours.
pub(crate) fn every_role_icon_resolves_to_a_real_glyph() {
    let display = gtk::gdk::Display::default().expect("a display");
    let theme = gtk::IconTheme::for_display(&display);
    for role in [
        Some(FolderRole::Inbox),
        Some(FolderRole::Drafts),
        Some(FolderRole::Sent),
        Some(FolderRole::Archive),
        Some(FolderRole::Junk),
        Some(FolderRole::Trash),
        Some(FolderRole::Other),
        None,
    ] {
        let icon = role_icon(role.as_ref());
        assert!(theme.has_icon(icon), "the pane must be able to draw {icon}");
    }
    assert!(
        !theme.has_icon("mailcal-not-an-icon-symbolic"),
        "a theme that answers yes to everything would make the check above meaningless"
    );
}

/// A provider outage badges only its own account, never its folders or a healthy neighbour.
pub(crate) fn only_an_unreachable_account_gets_the_warning() {
    let unreachable = HashSet::from(["acct-2".to_owned()]);
    let (list, _receiver) = pane_with_unreachable(&two_accounts(), &unreachable);
    let pane_rows = rows(&list);
    let warning = l10n::connectivity_account_unreachable();

    assert!(
        widget_tooltips(pane_rows[5].upcast_ref())
            .iter()
            .any(|text| text == warning),
        "the affected account carries the warning"
    );
    assert!(
        !widget_tooltips(pane_rows[1].upcast_ref())
            .iter()
            .any(|text| text == warning),
        "a healthy account carries no warning"
    );
    assert!(
        !widget_tooltips(pane_rows[6].upcast_ref())
            .iter()
            .any(|text| text == warning),
        "the warning belongs to the account, not its inbox"
    );
}

#[test]
fn a_role_bearing_folder_is_named_by_the_app_and_every_other_keeps_its_name() {
    assert_eq!(folder_label(Some(&FolderRole::Inbox), "INBOX"), "Inbox");
    assert_eq!(
        folder_label(Some(&FolderRole::Trash), "Deleted Items"),
        "Trash"
    );
    // Renamed on the server, and still called what we call it; the trade rule 12 names.
    assert_eq!(
        folder_label(Some(&FolderRole::Archive), "Archief 2024"),
        "Archive"
    );
    // `Other` collapses flagged, important and all-mail, so there is no one honest word for it.
    assert_eq!(
        folder_label(Some(&FolderRole::Other), "All Mail"),
        "All Mail"
    );
    assert_eq!(folder_label(None, "Sales & Marketing"), "Sales & Marketing");
}

#[test]
fn a_role_with_no_icon_of_its_own_takes_the_plain_folder() {
    assert_eq!(role_icon(None), role_icon(Some(&FolderRole::Other)));
    assert_ne!(role_icon(Some(&FolderRole::Inbox)), role_icon(None));
    assert_ne!(
        role_icon(Some(&FolderRole::Junk)),
        role_icon(Some(&FolderRole::Trash))
    );
}

#[test]
fn the_pane_is_rebuilt_when_a_tree_opens_or_a_count_moves() {
    let snapshot = two_accounts();
    let key = FolderPaneRendering::new(&snapshot, &HashSet::new());
    assert_eq!(
        key,
        FolderPaneRendering::new(&two_accounts(), &HashSet::new())
    );

    // Expansion and the counts change nothing about a row's *text*, so a key without them
    // leaves a shut tree drawn open and a stale badge on screen.
    let mut collapsed = two_accounts();
    collapsed.accounts[0].expanded = false;
    assert_ne!(key, FolderPaneRendering::new(&collapsed, &HashSet::new()));

    let mut counted = two_accounts();
    counted.account_folders[0].folders[0].unread = 546;
    assert_ne!(key, FolderPaneRendering::new(&counted, &HashSet::new()));

    let mut unified = two_accounts();
    unified.unified_unread = 0;
    assert_ne!(key, FolderPaneRendering::new(&unified, &HashSet::new()));

    // Selection moves among the existing rows. Rebuilding the whole tree here puts that work
    // ahead of the message list on every folder switch.
    let mut selected = two_accounts();
    selected.selected_account = Some("acct-1".to_owned());
    selected.selected = Some("inbox".to_owned());
    assert_eq!(key, FolderPaneRendering::new(&selected, &HashSet::new()));

    let unreachable = HashSet::from(["acct-1".to_owned()]);
    assert_ne!(
        key,
        FolderPaneRendering::new(&two_accounts(), &unreachable),
        "a connectivity signal must redraw the account badge without rebuilding the mail list"
    );
}

#[test]
fn the_pane_width_stays_between_its_floor_and_what_the_window_can_spare() {
    // Room to spare: the stored width is honoured.
    assert_eq!(width::clamp(320, 1280), 320);
    assert_eq!(width::clamp(width::MIN - 40, 1280), width::MIN);
    assert_eq!(width::clamp(width::MAX + 200, 2560), width::MAX);
    // The mail beside it keeps its minimum, whatever was stored.
    assert_eq!(width::clamp(560, 900), 900 - width::MIN_CONTENT);
    // A window too narrow for both floors keeps the pane at its own rather than taking the
    // folder tree away; and never asks for a clamp whose bounds have crossed.
    assert_eq!(width::clamp(400, 600), width::MIN);
    assert_eq!(width::clamp(400, 0), width::MIN);
}

/// The pane shows where the core says we are, and every row says which account it belongs to.
///
/// The second half is the one that looks fine until it is used: every provider calls its inbox
/// `inbox`, so a row that dispatched the key alone would resolve it against whichever account
/// happened to be selected and quietly open the wrong mailbox.
pub(crate) fn the_pane_marks_where_the_core_says_we_are() {
    let mut snapshot = two_accounts();
    snapshot.selected_account = Some("acct-2".to_owned());
    snapshot.selected = Some("inbox".to_owned());
    let (list, receiver) = pane(&snapshot);

    // [All Inboxes, acct-1, its three folders, acct-2, its inbox]; the last row is the one the
    // core has selected, and it is *not* the identically named row four above it.
    let pane_rows = rows(&list);
    assert_eq!(pane_rows.len(), 7, "every account's tree is drawn");
    assert_eq!(
        list.selected_row().as_ref(),
        pane_rows.last(),
        "the selected folder's own row carries the mark"
    );

    let row = pane_rows[6]
        .downcast_ref::<adw::ActionRow>()
        .expect("a pane row is an ActionRow");
    row.activatable_widget()
        .and_downcast::<gtk::Button>()
        .expect("a folder row has a native primary action")
        .emit_clicked();
    match receiver.recv_sync().expect("activating a row dispatches") {
        AppInput::ActivateSidebar(target) => assert_eq!(
            target,
            super::SidebarTarget::Folder {
                account: "acct-2".to_owned(),
                key: "inbox".to_owned(),
            },
            "a folder row names its own account, never the selected one"
        ),
        other => panic!("activating a folder row must navigate to it: {other:?}"),
    }

    // The chevron opens a tree; it does not navigate. A different message entirely, which is
    // what keeps a click on it from moving the selection off the folder being read.
    let chevron = buttons(pane_rows[5].upcast_ref::<gtk::Widget>())
        .pop()
        .expect("an account row carries its disclosure control");
    chevron.emit_clicked();
    match receiver.recv_sync().expect("the chevron dispatches") {
        AppInput::SetAccountExpanded { account, expanded } => {
            assert_eq!(account, "acct-2");
            assert!(!expanded, "an open tree's chevron shuts it");
        }
        other => panic!("the chevron may only change expansion: {other:?}"),
    }

    // Nothing selected in the core is the unified view, and the pane says so.
    let (list, _receiver) = pane(&two_accounts());
    assert_eq!(
        list.selected_row().as_ref(),
        rows(&list).first(),
        "the unified row is marked while no account is selected"
    );
}

/// Moving between folders changes only the selected existing row. In particular, the duplicate
/// Inbox under the other account must not be selected and no row widget may be replaced.
pub(crate) fn moving_the_selection_reuses_the_pane() {
    let snapshot = two_accounts();
    let (list, _receiver) = pane(&snapshot);
    let before = rows(&list);

    let mut selected = snapshot;
    selected.selected_account = Some("acct-2".to_owned());
    selected.selected = Some("inbox".to_owned());
    select_snapshot_row(&list, &selected);

    let after = rows(&list);
    assert_eq!(before, after, "selection must not replace pane rows");
    assert_eq!(
        list.selected_row().as_ref(),
        after.last(),
        "the folder is resolved within its own account"
    );
}

/// GTK moves the mark on press, before the core publishes the matching snapshot. An unrelated
/// shell render in that interval must not put the mark back on the previous folder.
pub(crate) fn an_optimistic_click_is_not_undone_by_the_previous_snapshot() {
    let snapshot = two_accounts();
    let (list, _receiver) = pane(&snapshot);
    let mut selection = FolderPaneSelection::default();
    selection.sync(&list, &snapshot);

    let clicked = rows(&list)[6].clone();
    list.select_row(Some(&clicked));
    selection.sync(&list, &snapshot);

    assert_eq!(
        list.selected_row().as_ref(),
        Some(&clicked),
        "the user's immediate selection stands until the snapshot changes"
    );
}

pub(crate) fn folder_rows_expose_their_navigation_as_a_semantic_action() {
    let (sender, receiver) = relm4::channel::<AppInput>();
    let row = super::pane_row(
        "folder-symbolic",
        &sender,
        &SidebarTarget::Folder {
            account: "account".to_owned(),
            key: "archive".to_owned(),
        },
    );
    row.activatable_widget()
        .and_downcast::<gtk::Button>()
        .expect("a folder row has a native primary action")
        .emit_clicked();
    assert!(matches!(
        receiver.recv_sync(),
        Some(AppInput::ActivateSidebar(SidebarTarget::Folder { account, key }))
            if account == "account" && key == "archive"
    ));
}

/// Every `GtkButton` under `root`, in tree order.
fn buttons(root: &gtk::Widget) -> Vec<gtk::Button> {
    let mut found = Vec::new();
    if let Some(button) = root.downcast_ref::<gtk::Button>() {
        found.push(button.clone());
    }
    let mut child = root.first_child();
    while let Some(node) = child {
        found.extend(buttons(&node));
        child = node.next_sibling();
    }
    found
}

fn widget_tooltips(root: &gtk::Widget) -> Vec<String> {
    let mut found = root
        .tooltip_text()
        .map(|text| vec![text.to_string()])
        .unwrap_or_default();
    let mut child = root.first_child();
    while let Some(node) = child {
        found.extend(widget_tooltips(&node));
        child = node.next_sibling();
    }
    found
}

#[test]
fn the_list_header_names_the_scope_the_same_way_the_pane_does() {
    let mut snapshot = two_accounts();
    assert_eq!(header_title(&snapshot), "All Inboxes");

    // An account with no folder chosen is that account's whole mailbox.
    snapshot.selected_account = Some("acct-1".to_owned());
    assert_eq!(header_title(&snapshot), "All Mail");

    // A folder is named by the app, exactly as its row is: never `INBOX`.
    snapshot.selected = Some("inbox".to_owned());
    assert_eq!(header_title(&snapshot), "Inbox");
    snapshot.selected = Some("custom".to_owned());
    assert_eq!(header_title(&snapshot), "Sales & Marketing");

    // The key resolves within the *selected* account, not across the pane: acct-2 has no
    // `custom`, so the header must not borrow acct-1's row for it.
    snapshot.selected_account = Some("acct-2".to_owned());
    assert_eq!(header_title(&snapshot), "Mail");
}
