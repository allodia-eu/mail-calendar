//! What the search chrome must put on screen, and what it must never dispatch back.
//!
//! Called from the crate's single `gtk::init` test (see [`crate::ui::mailbox::tests`]).

use adw::prelude::*;
use mailcal_bindings::{
    AccountFolderRow, AccountRow, FolderRole, FolderRow, MailboxListSnapshot, SearchHorizon,
    SearchScope,
};

use super::{
    super::{super::AppInput, model::SearchState},
    SEARCH_DELAY_MS, SearchBar,
};
use crate::ui::{mailbox::tests::rendered_labels, model::empty_mailbox};

/// One account standing in a custom folder whose **server-supplied** name carries an ampersand,
/// the character that renders a markup-parsing label blank.
fn snapshot(horizon: Option<SearchHorizon>) -> MailboxListSnapshot {
    MailboxListSnapshot {
        accounts: vec![AccountRow {
            id: "acct-1".to_owned(),
            email: "eva.jansen@example.test".to_owned(),
            expanded: true,
        }],
        selected_account: Some("acct-1".to_owned()),
        selected: Some("custom".to_owned()),
        account_folders: vec![AccountFolderRow {
            account_id: "acct-1".to_owned(),
            folders: vec![FolderRow {
                key: "custom".to_owned(),
                name: "Sales & Marketing".to_owned(),
                role: None::<FolderRole>,
                unread: 0,
            }],
        }],
        search_horizon: horizon,
        ..empty_mailbox()
    }
}

fn searching(query: &str) -> SearchState {
    let mut state = SearchState::default();
    state.set_query(query.to_owned());
    state
}

fn bar() -> (SearchBar, relm4::Receiver<AppInput>) {
    let (sender, receiver) = relm4::channel::<AppInput>();
    (SearchBar::new(&sender), receiver)
}

/// The field, reached through the bar's own tree; nothing in the shipping code holds it, because
/// nothing in the shipping code writes it.
fn entry(bar: &SearchBar) -> gtk::SearchEntry {
    bar.root
        .first_child()
        .and_then(|child| child.downcast::<gtk::SearchEntry>().ok())
        .expect("the field is the first thing in the bar")
}

/// The filter and the horizon describe a **search**, so they are on screen for one and for nothing
/// else; and they leave again when the query does. A scope filter over an ordinary folder offers
/// to narrow something that is not running.
pub(crate) fn the_filter_and_the_horizon_are_shown_for_a_search_and_nothing_else() {
    let (bar, _receiver) = bar();

    bar.render(&SearchState::default(), &snapshot(None));
    assert!(!bar.filter.reveals_child(), "no search, no scope filter");
    assert!(!bar.horizon_row.is_visible());

    bar.render(
        &searching("quarterly"),
        &snapshot(Some(SearchHorizon::Months { months: 3 })),
    );
    assert!(bar.filter.reveals_child());
    assert!(bar.horizon_row.is_visible());
    assert_eq!(bar.horizon.text(), "Searching the last 3 months");

    // Rule 6's visible half: clearing takes the whole chrome with it, in the same render.
    bar.render(&SearchState::default(), &snapshot(None));
    assert!(!bar.filter.reveals_child());
    assert!(!bar.horizon_row.is_visible());
}

/// The narrowing side names the folder the mailbox list is showing, in full: the name is the
/// server's, so an ampersand has to reach the screen rather than being read as an entity; which
/// renders the button blank while the label property still reads back correctly.
pub(crate) fn the_filter_names_the_folder_it_would_narrow_to() {
    let (bar, _receiver) = bar();
    bar.render(&searching("quarterly"), &snapshot(None));

    let shown = rendered_labels(bar.current.upcast_ref::<gtk::Widget>());
    assert!(
        shown.iter().any(|text| text == "Sales & Marketing"),
        "the folder's own name must render on the filter: {shown:?}"
    );
    let all = rendered_labels(bar.all.upcast_ref::<gtk::Widget>());
    assert!(all.iter().any(|text| text == "All mail"), "{all:?}");
}

/// Moving the filter dispatches the side that became **active**, once.
///
/// The two toggles are grouped, so switching sides fires on both; the one being turned off as
/// well. Without the guard the widening click would also report the narrowing the user just left,
/// and the last message to land would be the scope they are no longer in.
pub(crate) fn moving_the_filter_dispatches_only_the_side_that_became_active() {
    let (bar, receiver) = bar();
    bar.render(&searching("quarterly"), &snapshot(None));

    bar.current.emit_clicked();
    assert!(matches!(
        receiver.recv_sync(),
        Some(AppInput::SetSearchScope(SearchScope::CurrentFolder))
    ));
    bar.all.emit_clicked();
    assert!(
        matches!(
            receiver.recv_sync(),
            Some(AppInput::SetSearchScope(SearchScope::AllFolders))
        ),
        "widening must not also report the narrowing it switched off"
    );
}

/// Rendering the core's state is not the user acting on it.
///
/// The filter is driven from the model on every render, so a chrome that echoed those writes back
/// would dispatch a scope nobody moved; on every snapshot the core publishes. Had it echoed, the
/// assertion below would read *its* message instead of the press that follows.
pub(crate) fn rendering_the_cores_state_dispatches_nothing_back() {
    let (bar, receiver) = bar();
    let mut narrowed = searching("quarterly");
    narrowed.set_scope(SearchScope::CurrentFolder);

    bar.render(&narrowed, &snapshot(None));
    assert!(bar.current.is_active(), "the filter shows the core's scope");
    assert!(!bar.all.is_active(), "one group, one lit side");
    bar.render(&SearchState::default(), &snapshot(None));
    assert!(bar.all.is_active(), "leaving search widens the filter too");

    bar.current.emit_clicked();
    assert!(matches!(
        receiver.recv_sync(),
        Some(AppInput::SetSearchScope(SearchScope::CurrentFolder))
    ));
}

/// A render carrying a query the typing has already moved past must leave the field alone.
///
/// A search is asynchronous, and the first one is the slowest; the list swaps from threaded
/// folder rows to flat results and the query runs cold; so the snapshot lands while the next
/// character is being typed. A field written from that state loses the character and puts the
/// cursor at the front, which is where every keystroke after it then lands.
pub(crate) fn a_render_behind_the_typing_leaves_the_field_alone() {
    let (bar, _receiver) = bar();
    entry(&bar).set_text("quar");
    entry(&bar).set_position(4);

    // The snapshot for what the core was told, arriving one keystroke late.
    bar.render(&searching("qua"), &snapshot(None));

    assert_eq!(
        entry(&bar).text(),
        "quar",
        "the field is the user's, not the model's"
    );
    assert_eq!(entry(&bar).position(), 4, "and so is the cursor");
}

/// Escape leaves search; the desktop's way out of a mode, and the one rule 5 asks a client for.
/// The core answers by restoring the account and folder the search was opened from.
pub(crate) fn escape_leaves_search() {
    let (bar, receiver) = bar();
    entry(&bar).set_text("quarterly");
    bar.render(&searching("quarterly"), &snapshot(None));

    entry(&bar).emit_stop_search();
    match receiver.recv_sync() {
        Some(AppInput::SearchMail(query)) => assert!(query.is_empty()),
        other => panic!("Escape must leave search, got {other:?}"),
    }
    // Emptied here rather than left for a render: this is the one moment the query changes with
    // no keystroke behind it, and nothing writes the field back afterwards.
    assert_eq!(entry(&bar).text(), "");
}

/// Typing is not searching: the field waits for the typing to settle, on the beat the contract
/// names, rather than dispatching a full-text query per account per keystroke.
pub(crate) fn typing_is_debounced_on_the_contracts_beat() {
    let (bar, _receiver) = bar();
    assert_eq!(entry(&bar).search_delay(), SEARCH_DELAY_MS);
}
