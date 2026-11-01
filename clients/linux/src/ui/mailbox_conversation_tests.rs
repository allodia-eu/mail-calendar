//! Widget-level regressions for what a conversation row draws of its messages' state, and how
//! the list folds a conversation away (`docs/row-actions.md`).
//!
//! Functions rather than `#[test]`s, called from [`super::tests`], for the reason
//! [`super::thread_tests`] gives: the crate initialises GTK exactly once.

use adw::prelude::*;
use mailcal_bindings::SnapshotRow;

/// Every case here, in the order the runner calls them.
pub(super) fn run() {
    a_conversation_draws_its_flag_and_a_draft_resumes_from_inside_it();
    the_list_folds_a_conversation_the_selection_has_left();
    a_press_on_a_rows_menu_is_the_menus_and_not_a_click_on_the_row();
}

use super::{
    ThreadKey,
    tests::labels,
    thread_tests::{expander_at, message, rendered, reported, snapshot, thread},
};

/// A conversation draws the flag while any message on it is flagged, which is when its menu
/// offers Unflag, and each message inside it draws its own (`docs/row-actions.md`, rule 3). A
/// draft inside it says it is one, so it resumes in a composer from whatever folder it is shown in
/// (`docs/drafts.md`).
fn a_conversation_draws_its_flag_and_a_draft_resumes_from_inside_it() {
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
fn the_list_folds_a_conversation_the_selection_has_left() {
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
    let drawn: Vec<_> = (rows.rows.iter())
        .map(|row| crate::ui::mailbox_display::display_row(row, "UTC"))
        .collect();
    let expanded = std::collections::HashSet::from([two]);
    crate::ui::selection_input::sync_expansion(&widgets, &drawn, &expanded);
    assert!(!expander_at(&widgets, 0).is_expanded());
    assert!(expander_at(&widgets, 1).is_expanded());
    assert_eq!(reported(&sender, &receiver), vec!["expanded false"]);
}

/// The ⋯ on a row, a conversation's or a message's inside it, takes its own press: the list does
/// not read it as a click on the row, so opening the menu leaves the selection as it was
/// (`docs/list-selection.md`, rule 12). A press on the row's text is still the row's.
fn a_press_on_a_rows_menu_is_the_menus_and_not_a_click_on_the_row() {
    let rows = snapshot(vec![SnapshotRow::Thread {
        row: thread(
            "Venue for the spring meeting",
            vec![message("only", "Anna", "Preview", false, false)],
        ),
    }]);
    let key = ThreadKey::of(match &rows.rows[0] {
        SnapshotRow::Thread { row } => row,
        SnapshotRow::Flat { .. } => unreachable!(),
    });
    let (list, _sender, _receiver) = rendered(&rows, &[key]);
    let menus = more_buttons(list.upcast_ref());
    assert_eq!(menus.len(), 2, "the conversation's ⋯ and its message's");
    for menu in &menus {
        let (x, y) = centre_in(menu, &list);
        assert!(
            crate::ui::selection_input::pressed_on_control(&list, x, y),
            "a press on ⋯ is the button's"
        );
    }
    // The conversation is the list's first row: its leading edge holds no control.
    assert!(
        !crate::ui::selection_input::pressed_on_control(&list, 4.0, 12.0),
        "a press at the row's leading edge is the row's"
    );
}

/// Every ⋯ button under `root`.
fn more_buttons(root: &gtk::Widget) -> Vec<gtk::Button> {
    let mut found = Vec::new();
    if let Some(button) = root.downcast_ref::<gtk::Button>()
        && button.icon_name().as_deref() == Some(crate::ui::icons::MORE)
    {
        found.push(button.clone());
    }
    let mut child = root.first_child();
    while let Some(widget) = child {
        found.extend(more_buttons(&widget));
        child = widget.next_sibling();
    }
    found
}

/// The centre of `widget`, in `list`'s coordinates.
fn centre_in(widget: &impl IsA<gtk::Widget>, list: &gtk::ListBox) -> (f64, f64) {
    let bounds = widget
        .as_ref()
        .compute_bounds(list)
        .expect("the widget is inside the list");
    (
        f64::from(bounds.x() + bounds.width() / 2.0),
        f64::from(bounds.y() + bounds.height() / 2.0),
    )
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
