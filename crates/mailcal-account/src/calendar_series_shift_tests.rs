//! What a series edit does with clocks that were read from one occurrence.
//!
//! Split into its own `#[path]` file to keep `calendar_tests.rs` under the 500-line limit.
//!
//! The case with teeth is the one a user hit: an editor opened on a later occurrence holds that
//! occurrence's times, and a rule change makes the save mean the series. Writing those clocks onto
//! the master moved its start forward, so every occurrence before it stopped existing.

use engine_core::{
    ids::{CalendarId, EventId, Uid},
    membership::Memberships,
    time::{CalendarDateTime, Duration, LocalDateTime, TimeZoneId, UtcDateTime},
    version::{ETag, RevisionTokens},
};

use super::*;

fn amsterdam(local: &str) -> CalendarDateTime {
    CalendarDateTime::Zoned {
        local: local.parse().unwrap(),
        zone: TimeZoneId::iana("Europe/Amsterdam").unwrap(),
    }
}

fn stored_event() -> Event {
    let mut event = Event::new(
        EventId::try_from("/cal/standup.ics").unwrap(),
        Uid::new("standup@test.local").unwrap(),
        Memberships::of_one(CalendarId::try_from("/cal/").unwrap()),
        amsterdam("2026-01-05T09:30:00"),
    );
    event.title = "Standup".to_owned();
    event.duration = Duration::from_parts(0, 0, 0, 30, 0, 0).unwrap();
    event.revisions = RevisionTokens::from_etag(ETag::new("\"v7\""));
    event
}

fn now() -> UtcDateTime {
    UtcDateTime::new(2026, 2, 10, 11, 30, 0).unwrap()
}

fn local(text: &str) -> LocalDateTime {
    text.parse().unwrap()
}

/// The bug this pins: the editor opened on a later occurrence holds **that** occurrence's times,
/// and a rule change makes the save mean the series. Writing those clocks onto the master moved
/// its start forward to that occurrence, so every earlier one stopped existing. A save that
/// touched no time must leave the series' start exactly where it was.
#[test]
fn a_series_edit_from_a_later_occurrence_leaves_the_series_start_alone() {
    let (target, patch) = build_event_patch(
        &stored_event(),
        &EventEdit {
            // The 26th's occurrence, opened and saved with its own times untouched.
            start: Some(local("2026-01-26T09:30:00")),
            end: Some(local("2026-01-26T10:00:00")),
            times_from_occurrence: Some(local("2026-01-26T09:30:00")),
            ..EventEdit::default()
        },
        now(),
    )
    .unwrap();
    assert_eq!(target, PatchTarget::Series);
    assert_eq!(patch.start_edit(), Some(&amsterdam("2026-01-05T09:30:00")));
    assert_eq!(patch.end_edit(), Some(&amsterdam("2026-01-05T10:00:00")));
}

/// A time moved on one occurrence, saved for the series, moves the series by that much: the same
/// answer a drag on a series gives, and the reason the shift is carried rather than the clock.
#[test]
fn a_time_changed_on_one_occurrence_shifts_the_whole_series_by_that_much() {
    let (_, patch) = build_event_patch(
        &stored_event(),
        &EventEdit {
            // 09:30 -> 11:00 on the 26th: an hour and a half later, every week.
            start: Some(local("2026-01-26T11:00:00")),
            end: Some(local("2026-01-26T11:30:00")),
            times_from_occurrence: Some(local("2026-01-26T09:30:00")),
            ..EventEdit::default()
        },
        now(),
    )
    .unwrap();
    assert_eq!(patch.start_edit(), Some(&amsterdam("2026-01-05T11:00:00")));
    assert_eq!(patch.end_edit(), Some(&amsterdam("2026-01-05T11:30:00")));
}

/// The end follows the edited duration, so a resize on one occurrence resizes the series.
#[test]
fn a_resize_on_one_occurrence_resizes_the_series_by_the_same_span() {
    let (_, patch) = build_event_patch(
        &stored_event(),
        &EventEdit {
            start: Some(local("2026-01-26T09:30:00")),
            end: Some(local("2026-01-26T10:30:00")),
            times_from_occurrence: Some(local("2026-01-26T09:30:00")),
            ..EventEdit::default()
        },
        now(),
    )
    .unwrap();
    assert_eq!(patch.start_edit(), Some(&amsterdam("2026-01-05T09:30:00")));
    // A 30-minute series became an hour, on its own start rather than the occurrence's.
    assert_eq!(patch.end_edit(), Some(&amsterdam("2026-01-05T10:30:00")));
}

/// An editor opened on the series shows the series' own clocks, so there is nothing to shift.
#[test]
fn an_edit_that_names_no_occurrence_writes_its_clocks_as_they_are() {
    let (_, patch) = build_event_patch(
        &stored_event(),
        &EventEdit {
            start: Some(local("2026-01-05T14:00:00")),
            end: Some(local("2026-01-05T14:30:00")),
            ..EventEdit::default()
        },
        now(),
    )
    .unwrap();
    assert_eq!(patch.start_edit(), Some(&amsterdam("2026-01-05T14:00:00")));
}

/// The occurrence-scoped edit lands on the clocks it already describes, so it is never shifted.
#[test]
fn an_occurrence_scoped_edit_is_not_shifted() {
    let (target, patch) = build_event_patch(
        &stored_event(),
        &EventEdit {
            start: Some(local("2026-01-26T14:00:00")),
            end: Some(local("2026-01-26T14:30:00")),
            occurrence: Some(local("2026-01-26T09:30:00")),
            times_from_occurrence: Some(local("2026-01-26T09:30:00")),
            ..EventEdit::default()
        },
        now(),
    )
    .unwrap();
    assert!(matches!(target, PatchTarget::Instance(_)));
    assert_eq!(patch.start_edit(), Some(&amsterdam("2026-01-26T14:00:00")));
}

/// A shift is not a shift with one end of it missing, so it is refused rather than half-applied.
#[test]
fn a_shift_missing_an_edge_is_refused() {
    let refused = build_event_patch(
        &stored_event(),
        &EventEdit {
            start: Some(local("2026-01-26T11:00:00")),
            times_from_occurrence: Some(local("2026-01-26T09:30:00")),
            ..EventEdit::default()
        },
        now(),
    );
    assert!(refused.is_err());
}
