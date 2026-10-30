//! Widget-level regressions for what a conversation row draws of its messages' state, and how
//! the list folds a conversation away (`docs/row-actions.md`).
//!
//! Functions rather than `#[test]`s, called from [`super::tests`], for the reason
//! [`super::thread_tests`] gives: the crate initialises GTK exactly once.

use adw::prelude::*;
use mailcal_bindings::SnapshotRow;

use super::{
    ThreadKey,
    tests::labels,
    thread_tests::{expander_at, message, rendered, reported, snapshot, thread},
};

/// A conversation draws the flag while any message on it is flagged, which is when its menu
/// offers Unflag, and each message inside it draws its own (`docs/row-actions.md`, rule 3). A
/// draft inside it says it is one, so it resumes in a composer from whatever folder it is shown in
/// (`docs/drafts.md`).
pub(super) fn a_conversation_draws_its_flag_and_a_draft_resumes_from_inside_it() {
    let mut flagged = message("flagged", "Anna Bakker", "Flagged reply", false, false);
    flagged.flagged = true;
    let mut draft = message("draft", "Me", "Unfinished answer", false, false);
    draft.draft = true;
    let mut conversation = thread(
        "Contribution for next year",
        vec![
            draft,
            flagged,
            message("plain", "Bram", "Opening", false, false),
        ],
    );
    conversation.flagged = true;
    let key = ThreadKey::of(&conversation);
    let rows = snapshot(vec![SnapshotRow::Thread {
        row: conversation.clone(),
    }]);
    let (widgets, sender, receiver) = rendered(&rows, std::slice::from_ref(&key));
    let expander = expander_at(&widgets, 0);
    let sub_row = |preview: &str| {
        labels(expander.upcast_ref::<gtk::Widget>())
            .into_iter()
            .find(|label| label.text() == preview)
            .and_then(|label| label.ancestor(adw::ActionRow::static_type()))
            .and_downcast::<adw::ActionRow>()
            .unwrap_or_else(|| panic!("no sub-row shows {preview:?}"))
    };
    assert_eq!(
        flags(expander.upcast_ref::<gtk::Widget>()),
        2,
        "the conversation's own flag and its flagged message's"
    );
    assert_eq!(flags(sub_row("Flagged reply").upcast_ref()), 1);
    assert_eq!(flags(sub_row("Opening").upcast_ref()), 0);

    sub_row("Unfinished answer")
        .activatable_widget()
        .and_downcast::<gtk::Button>()
        .expect("a message row has a native primary action")
        .emit_clicked();
    assert_eq!(reported(&sender, &receiver), vec!["open draft as a draft"]);

    // Nothing flagged on it: no flag on the conversation either.
    conversation.flagged = false;
    for message in &mut conversation.messages {
        message.flagged = false;
    }
    let rows = snapshot(vec![SnapshotRow::Thread { row: conversation }]);
    let (widgets, _sender, _receiver) = rendered(&rows, std::slice::from_ref(&key));
    assert_eq!(flags(expander_at(&widgets, 0).upcast_ref()), 0);
}

/// The list draws the model's expansion back onto its rows, so leaving a conversation folds it
/// away (`docs/row-actions.md`, rule 8); and a row reports the change, which the model ignores
/// because it already holds it.
pub(super) fn the_list_folds_a_conversation_the_selection_has_left() {
    let mut second = thread(
        "Second",
        vec![message("second", "Bram", "Second preview", false, false)],
    );
    second.thread_id = "thread-2".to_owned();
    let first = thread(
        "First",
        vec![message("first", "Anna", "First preview", false, false)],
    );
    let (one, two) = (ThreadKey::of(&first), ThreadKey::of(&second));
    let rows = snapshot(vec![
        SnapshotRow::Thread { row: first },
        SnapshotRow::Thread { row: second },
    ]);
    let (widgets, sender, receiver) = rendered(&rows, &[one.clone(), two.clone()]);
    crate::ui::selection_input::sync_expansion(
        &widgets,
        &rows.rows,
        &std::collections::HashSet::from([two]),
    );
    assert!(!expander_at(&widgets, 0).is_expanded());
    assert!(expander_at(&widgets, 1).is_expanded());
    assert_eq!(reported(&sender, &receiver), vec!["expanded false"]);
}

/// How many flags `root` draws.
fn flags(root: &gtk::Widget) -> usize {
    let own = root
        .downcast_ref::<gtk::Image>()
        .is_some_and(|image| image.icon_name().as_deref() == Some(crate::ui::icons::FLAG));
    let mut count = usize::from(own);
    let mut child = root.first_child();
    while let Some(widget) = child {
        count += flags(&widget);
        child = widget.next_sibling();
    }
    count
}
