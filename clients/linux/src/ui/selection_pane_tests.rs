//! The count the reading pane states over several selected rows.
//!
//! Called from the crate's single `gtk::init` test (see [`crate::ui::mailbox::tests`]).

use adw::prelude::*;
use mailcal_bindings::ViewMode;

use super::SelectionCountPane;
use crate::ui::selection::SelectionSummary;

/// The count belongs to the reading pane, and only above one row: at one, that row is the message
/// the pane is showing.
pub(crate) fn the_pane_states_the_count_only_while_it_covers_something_worth_covering() {
    let pane = SelectionCountPane::new();
    let summary = |count: usize| SelectionSummary { count };

    pane.render(summary(0), &ViewMode::Threaded);
    assert!(!pane.widget().is_visible());
    pane.render(summary(1), &ViewMode::Threaded);
    assert!(
        !pane.widget().is_visible(),
        "one selected row is the one the pane is already showing"
    );
    pane.render(summary(2), &ViewMode::Threaded);
    assert!(pane.widget().is_visible());
    pane.render(summary(0), &ViewMode::Threaded);
    assert!(
        !pane.widget().is_visible(),
        "clearing the selection gives the pane back what it was holding"
    );
}
