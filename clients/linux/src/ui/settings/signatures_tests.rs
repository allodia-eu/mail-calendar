//! What the Signatures category must draw, and what it must never draw.
//!
//! The two pickers' rules are pure and run as ordinary tests. The rendering half is asserted on
//! the **rendered** labels: `ActionRow::title()` reads back the string it was handed whatever
//! became of the label, so asserting on the property is a green light for a blank row. Those are
//! called from the crate's single `gtk::init` test (see [`crate::ui::mailbox::tests`]).

use adw::prelude::*;

use super::{named_row, slot_choice, slot_selection};
use crate::ui::mailbox::tests::{glib_records, rendered_labels};

#[test]
fn a_slot_opens_on_what_the_account_assigned_and_none_leads() {
    let ids = ["id-work".to_owned(), "id-home".to_owned()];

    assert_eq!(slot_selection(None, &ids), 0, "unassigned reads as None");
    assert_eq!(slot_selection(Some("id-work"), &ids), 1);
    assert_eq!(slot_selection(Some("id-home"), &ids), 2);
    // The core clears every assignment when a signature is deleted, so a slot naming nothing is
    // already impossible; showing None is a display detail, not a second teardown path.
    assert_eq!(slot_selection(Some("id-deleted"), &ids), 0);
}

#[test]
fn picking_the_first_entry_clears_the_slot_rather_than_assigning_the_first_signature() {
    let ids = ["id-work".to_owned(), "id-home".to_owned()];

    // Off by one here is silent and wrong in the worst way: every account would quietly send the
    // signature above the one its picker shows.
    assert_eq!(slot_choice(0, &ids), None);
    assert_eq!(slot_choice(1, &ids), Some("id-work".to_owned()));
    assert_eq!(slot_choice(2, &ids), Some("id-home".to_owned()));
    assert_eq!(slot_choice(3, &ids), None, "a stale index assigns nothing");
}

/// A signature's name and an account's address are text somebody else wrote, and both land in a
/// row title. A bare ampersand renders the row **blank** when it is parsed as markup, and a
/// markup-shaped name is *applied* rather than shown.
pub(crate) fn a_signatures_own_text_is_never_parsed_as_markup() {
    let (rows, records) = glib_records(|| {
        [
            named_row("Sales & Marketing"),
            named_row("ada&grace@example.test"),
            named_row("<b>Work</b>"),
        ]
    });

    // Rendering alone cannot see this half: libadwaita re-applies the labels when the flag flips,
    // so a row built the wrong way still reads correctly and only warns.
    assert!(
        !records.iter().any(|line| line.contains("from markup")),
        "a name or an address must not be parsed as markup: {records:?}"
    );
    for row in &rows {
        assert!(!row.uses_markup());
    }
    let shown = rows
        .iter()
        .flat_map(|row| rendered_labels(row.upcast_ref::<gtk::Widget>()))
        .collect::<Vec<_>>();
    for expected in ["Sales & Marketing", "ada&grace@example.test", "<b>Work</b>"] {
        assert!(
            shown.iter().any(|text| text == expected),
            "{expected:?} must reach the screen as itself: {shown:?}"
        );
    }
}
