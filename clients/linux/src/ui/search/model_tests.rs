//! What the search chrome must say, and what it must never let drift from the core.

use mailcal_bindings::{
    AccountFolderRow, AccountRow, FolderRole, FolderRow, MailboxListSnapshot, SearchHorizon,
    SearchScope,
};

use super::{QueryChange, SearchState, horizon_label, scope_label};
use crate::ui::model::empty_mailbox;

fn folder(key: &str, name: &str, role: Option<FolderRole>) -> FolderRow {
    FolderRow {
        key: key.to_owned(),
        name: name.to_owned(),
        role,
        unread: 0,
    }
}

/// One account standing in a folder, the shape the "this scope" side of the filter names.
fn snapshot(selected_account: Option<&str>, selected: Option<&str>) -> MailboxListSnapshot {
    MailboxListSnapshot {
        accounts: vec![AccountRow {
            id: "acct-1".to_owned(),
            email: "eva.jansen@example.test".to_owned(),
            expanded: true,
        }],
        selected_account: selected_account.map(str::to_owned),
        selected: selected.map(str::to_owned),
        account_folders: vec![AccountFolderRow {
            account_id: "acct-1".to_owned(),
            folders: vec![
                folder("inbox", "INBOX", Some(FolderRole::Inbox)),
                folder("custom", "Sales & Marketing", None),
            ],
        }],
        ..empty_mailbox()
    }
}

/// A whitespace-only query is not a search; the core reads it as none, and a client that
/// disagreed would draw a scope filter and a horizon line over an ordinary folder.
#[test]
fn only_a_query_with_something_in_it_is_a_search() {
    let mut state = SearchState::default();
    assert!(!state.is_active());

    assert_eq!(state.set_query("   ".to_owned()), None);
    assert!(!state.is_active());

    assert_eq!(
        state.set_query("quarterly".to_owned()),
        Some(QueryChange::Run("quarterly".to_owned()))
    );
    assert!(state.is_active());
}

/// An empty field with no search behind it asks the core for nothing.
///
/// Escape reports an empty field whether or not one is running, and every such report reaching the
/// core as "leave search" would rebuild the snapshot: which resets the list's window, throwing a
/// user who had scrolled a long way down back to the newest mail for a key that changed nothing.
#[test]
fn leaving_a_search_nobody_started_asks_the_core_for_nothing() {
    let mut state = SearchState::default();
    assert_eq!(state.set_query(String::new()), None);

    state.set_query("quarterly".to_owned());
    assert_eq!(state.set_query(String::new()), Some(QueryChange::Leave));
    assert_eq!(state.set_query(String::new()), None);
}

/// Rule 6: clearing the query resets the scope in the client's own control, as one action. The
/// core does the same on its side, and a filter still claiming "this folder" over a search the
/// core has widened is a narrowing the user cannot see.
#[test]
fn clearing_the_query_resets_the_scope_the_filter_is_showing() {
    let mut state = SearchState::default();
    state.set_query("quarterly".to_owned());
    state.set_scope(SearchScope::CurrentFolder);
    assert_eq!(state.scope(), SearchScope::CurrentFolder);

    assert_eq!(state.set_query(String::new()), Some(QueryChange::Leave));
    assert_eq!(state.scope(), SearchScope::AllFolders);
}

/// The narrowing side names what is on screen: every account's Inbox in the unified view, the
/// account when no folder is picked, and otherwise the folder; by the **app's** name for it, the
/// one the pane and the list header already use, never the server's `INBOX`.
#[test]
fn the_filter_names_the_view_the_search_was_opened_from() {
    assert_eq!(scope_label(&snapshot(None, None)), "Inboxes");
    assert_eq!(scope_label(&snapshot(Some("acct-1"), None)), "This account");
    assert_eq!(
        scope_label(&snapshot(Some("acct-1"), Some("inbox"))),
        "Inbox"
    );
    // A folder with no special role keeps the server's name, ampersand and all.
    assert_eq!(
        scope_label(&snapshot(Some("acct-1"), Some("custom"))),
        "Sales & Marketing"
    );
    // A key the folder list has moved on from; a rename, a sync; must not leave the filter
    // offering a folder that is no longer there.
    assert_eq!(
        scope_label(&snapshot(Some("acct-1"), Some("gone"))),
        "This folder"
    );
}

/// Rule 8: the horizon is stated for a search and for nothing else, so the client keys the whole
/// line off the one snapshot field.
#[test]
fn the_horizon_is_stated_only_where_there_was_a_search() {
    assert_eq!(horizon_label(None), None);
    assert_eq!(
        horizon_label(Some(&SearchHorizon::AllTime)),
        Some("Searching all mail".to_owned())
    );
    assert_eq!(
        horizon_label(Some(&SearchHorizon::Months { months: 3 })),
        Some("Searching the last 3 months".to_owned())
    );
}
