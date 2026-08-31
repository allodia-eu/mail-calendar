//! Tests for **whose event it is**: the question a drag asks, and the one `can_move` answers.
//!
//! Split from [`crate::invitations_conflict_tests`], which asks a different question of the same
//! fixtures: that file is about which commitments *clash*, this one about which may be *re-timed*.
//! Keeping them apart is also what keeps each under the line limit.

use engine_api::{ParticipantRole, ParticipationStatus};
use mailcal_viewmodel::ResponseStatus;

use crate::{
    invitations::{diary_participation, owns_or_organizes},
    invitations_test_support::{attendee, event_with, organizer},
};

// --- whose event is it, for the purpose of dragging it -------------------------------

#[test]
fn an_appointment_nobody_was_invited_to_is_ours_to_drag() {
    // The commonest case by far: something the user typed into their own diary. There is nobody
    // to tell, so moving it is a private act.
    assert!(owns_or_organizes(
        &event_with(vec![]),
        &["me@test.local".to_owned()]
    ));
}

#[test]
fn a_meeting_we_called_is_ours_to_drag() {
    let ours = event_with(vec![
        organizer("Me", "me@test.local"),
        attendee("someone@test.local", ParticipationStatus::Accepted),
    ]);
    assert!(owns_or_organizes(&ours, &["me@test.local".to_owned()]));
}

#[test]
fn a_meeting_we_were_invited_to_is_not_ours_to_drag() {
    // This is the whole point of the flag being narrower than `can_write`. Our calendar is
    // writable and this event is on it, and re-timing somebody else's meeting behind their back
    // is not a move, it is a *proposal*, which is iTIP COUNTER and a feature of its own.
    let theirs = event_with(vec![
        organizer("Boss", "boss@test.local"),
        attendee("me@test.local", ParticipationStatus::Accepted),
    ]);
    assert!(!owns_or_organizes(&theirs, &["me@test.local".to_owned()]));
}

#[test]
fn a_room_booking_on_a_shared_calendar_is_not_ours_to_drag() {
    // Attendees, none of them us. `diary_participation` calls this a commitment; correctly, for
    // *drawing*. For *writing* it is somebody else's, and the two questions must not be answered
    // by one function.
    let theirs = event_with(vec![
        organizer("Facilities", "rooms@test.local"),
        attendee("someone@test.local", ParticipationStatus::Accepted),
    ]);
    assert!(!owns_or_organizes(&theirs, &["me@test.local".to_owned()]));
    assert_eq!(
        diary_participation(&theirs, &["me@test.local".to_owned()]),
        ResponseStatus::Accepted,
        "the drawing question still answers 'commitment', that is the distinction"
    );
}

#[test]
fn a_meeting_we_organize_from_an_alias_is_still_ours_to_drag() {
    // The grid has only the persisted address set to go on, and an organiser line written at an
    // alias is how a shared or role address organises. Missing it would make the user's own
    // meetings undraggable, with nothing on screen to explain why.
    let ours = event_with(vec![
        organizer("Info", "info@example.com"),
        attendee("someone@test.local", ParticipationStatus::Accepted),
    ]);
    assert!(owns_or_organizes(
        &ours,
        &[
            "alice@example.com".to_owned(),
            "info@example.com".to_owned()
        ]
    ));
}

#[test]
fn an_organizer_split_across_two_lines_is_still_recognized_as_us() {
    // JSCalendar merges ORGANIZER and the matching ATTENDEE into one participant; a plain
    // iCalendar server leaves them as two. Reading only the matched attendee misses the split
    // shape, and then the same meeting is draggable on one server and not on another.
    let mut ours = event_with(vec![
        organizer("Me", "me@test.local"),
        attendee("me@test.local", ParticipationStatus::NeedsAction),
    ]);
    ours.participants[0].roles = std::collections::BTreeSet::from([ParticipantRole::Owner]);
    assert!(owns_or_organizes(&ours, &["me@test.local".to_owned()]));
}
