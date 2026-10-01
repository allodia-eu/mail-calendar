//! Where the keyboard goes in a destination picker: over the rows that can be chosen, never onto
//! one that cannot.

use super::keyboard_target;

/// Top level offered, then Inbox, Archive kept only as a parent, and 2026 inside it.
const ROWS: [bool; 4] = [true, true, false, true];

#[test]
fn an_arrow_passes_over_a_row_that_cannot_be_chosen() {
    assert_eq!(keyboard_target(&ROWS, 1, 1), Some(3), "Inbox to 2026");
    assert_eq!(keyboard_target(&ROWS, 3, -1), Some(1), "2026 back to Inbox");
}

#[test]
fn a_longer_move_stops_at_the_last_row_that_can_be_chosen() {
    assert_eq!(keyboard_target(&ROWS, 0, 2), Some(3));
    assert_eq!(keyboard_target(&ROWS, 0, i32::MAX), Some(3), "End");
    assert_eq!(keyboard_target(&ROWS, 3, i32::MIN), Some(0), "Home");
}

#[test]
fn there_is_nowhere_to_go_past_the_end_or_onto_a_dimmed_root() {
    assert_eq!(keyboard_target(&ROWS, 3, 1), None);
    // Move to… on a folder at the top: Top level is drawn, but nothing above Inbox can be chosen.
    assert_eq!(keyboard_target(&[false, true, true], 1, -1), None);
    assert_eq!(keyboard_target(&[], 0, 1), None);
}
