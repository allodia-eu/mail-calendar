//! What a drag does to a stored event's own wall clock.
//!
//! Every case here is one the *display* is no help with: the event is in a zone the grid was
//! not drawn in, or the day it lands on is 23 hours long, or the edge the user pulled has run
//! past its opposite. A delta is what makes all three answerable without the client knowing
//! any of it.

use engine_api::{OccurrenceRow, TzdataVersion};
use engine_core::{
    calendar::{Event, Frequency, Recurrence, RecurrenceRule},
    ids::{CalendarId, EventId, Uid},
    membership::Memberships,
    time::{CalendarDate, CalendarDateTime, Duration, LocalDateTime, TimeZoneId, UtcDateTime},
};

use super::{EventDrag, EventEdge, apply_event_drag, names_an_occurrence, occurrence_wall_clock};

fn base(start: CalendarDateTime) -> Event {
    Event::new(
        EventId::try_from("/cal/e.ics").unwrap(),
        Uid::new("e@h").unwrap(),
        Memberships::of_one(CalendarId::try_from("work").unwrap()),
        start,
    )
}

/// A timed event in Amsterdam, an hour long, starting at the given wall clock.
fn amsterdam(year: i32, month: u8, day: u8, hour: u8, minute: u8) -> Event {
    let mut event = base(CalendarDateTime::Zoned {
        local: LocalDateTime::new(year, month, day, hour, minute, 0).unwrap(),
        zone: TimeZoneId::iana("Europe/Amsterdam").unwrap(),
    });
    event.title = "Standup".to_owned();
    event.duration = Duration::from_parts(0, 0, 1, 0, 0, 0).unwrap();
    event
}

/// An all-day event of `days` whole days, starting on the given date.
fn all_day(year: i32, month: u8, day: u8, days: u64) -> Event {
    let mut event = base(CalendarDateTime::Date(
        CalendarDate::new(year, month, day).unwrap(),
    ));
    event.title = "Offsite".to_owned();
    event.duration = Duration::from_parts(0, days, 0, 0, 0, 0).unwrap();
    event
}

fn wall(edit: &crate::EventEdit) -> (String, String) {
    let render = |at: LocalDateTime| {
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}",
            at.year(),
            at.month(),
            at.day(),
            at.hour(),
            at.minute()
        )
    };
    (
        render(edit.start.expect("a drag always moves an edge")),
        render(edit.end.expect("a drag always moves an edge")),
    )
}

fn drag(edge: EventEdge, days: i32, minutes: i32) -> EventDrag {
    EventDrag {
        edge,
        days,
        minutes,
        occurrence: None,
    }
}

#[test]
fn a_whole_drag_moves_both_edges_and_keeps_the_duration() {
    let event = amsterdam(2026, 7, 1, 9, 0);
    let edit = apply_event_drag(&event, &drag(EventEdge::Whole, 1, -30)).unwrap();
    assert_eq!(
        wall(&edit),
        ("2026-07-02T08:30".to_owned(), "2026-07-02T09:30".to_owned())
    );
}

#[test]
fn a_drag_writes_the_events_own_wall_clock_not_the_grids() {
    // The point of the delta. This event reads 09:00 in Amsterdam and 03:00 in New York; a
    // client drawing it in New York and dragging it down an hour sends `+60`, and what must be
    // written is 10:00 **Amsterdam**: not 04:00, and not 10:00 New York.
    let event = amsterdam(2026, 7, 1, 9, 0);
    let edit = apply_event_drag(&event, &drag(EventEdge::Whole, 0, 60)).unwrap();
    assert_eq!(wall(&edit).0, "2026-07-01T10:00");
}

#[test]
fn a_drag_across_spring_forward_keeps_the_clock_reading() {
    // Amsterdam loses an hour on 2026-03-29. Dragged one day on, a 09:00 meeting is at 09:00;
    // which is what the grid showed the user, because the grid is a wall clock. Adding 24 hours
    // of *elapsed* time instead would land it at 10:00.
    let event = amsterdam(2026, 3, 28, 9, 0);
    let edit = apply_event_drag(&event, &drag(EventEdge::Whole, 1, 0)).unwrap();
    assert_eq!(
        wall(&edit),
        ("2026-03-29T09:00".to_owned(), "2026-03-29T10:00".to_owned())
    );
}

#[test]
fn a_drag_over_midnight_carries_the_day() {
    let event = amsterdam(2026, 7, 1, 23, 30);
    let edit = apply_event_drag(&event, &drag(EventEdge::Whole, 0, 45)).unwrap();
    assert_eq!(wall(&edit).0, "2026-07-02T00:15");
}

#[test]
fn a_backwards_drag_over_midnight_carries_the_day_back() {
    let event = amsterdam(2026, 7, 2, 0, 15);
    let edit = apply_event_drag(&event, &drag(EventEdge::Whole, 0, -45)).unwrap();
    assert_eq!(wall(&edit).0, "2026-07-01T23:30");
}

#[test]
fn resizing_the_bottom_edge_leaves_the_start_alone() {
    let event = amsterdam(2026, 7, 1, 9, 0);
    let edit = apply_event_drag(&event, &drag(EventEdge::End, 0, 30)).unwrap();
    assert_eq!(
        wall(&edit),
        ("2026-07-01T09:00".to_owned(), "2026-07-01T10:30".to_owned())
    );
}

#[test]
fn resizing_the_top_edge_leaves_the_end_alone() {
    let event = amsterdam(2026, 7, 1, 9, 0);
    let edit = apply_event_drag(&event, &drag(EventEdge::Start, 0, -30)).unwrap();
    assert_eq!(
        wall(&edit),
        ("2026-07-01T08:30".to_owned(), "2026-07-01T10:00".to_owned())
    );
}

#[test]
fn a_bottom_edge_dragged_above_the_top_clamps_to_the_minimum() {
    // Not refused: a block that will not shrink, with nothing on screen to say why, is worse
    // than one that stops.
    let event = amsterdam(2026, 7, 1, 9, 0);
    let edit = apply_event_drag(&event, &drag(EventEdge::End, 0, -600)).unwrap();
    assert_eq!(
        wall(&edit),
        ("2026-07-01T09:00".to_owned(), "2026-07-01T09:15".to_owned())
    );
}

#[test]
fn a_top_edge_dragged_below_the_bottom_clamps_to_the_minimum() {
    let event = amsterdam(2026, 7, 1, 9, 0);
    let edit = apply_event_drag(&event, &drag(EventEdge::Start, 0, 600)).unwrap();
    assert_eq!(
        wall(&edit),
        ("2026-07-01T09:45".to_owned(), "2026-07-01T10:00".to_owned())
    );
}

#[test]
fn an_all_day_event_moves_by_whole_days_and_ignores_the_minutes() {
    // A bare date has no clock to move along, so a stray minute component must not round the
    // event onto its neighbour. The end stays exclusive: a one-day event on the 3rd ends on
    // the 4th.
    let event = all_day(2026, 7, 1, 1);
    let edit = apply_event_drag(&event, &drag(EventEdge::Whole, 2, 45)).unwrap();
    assert_eq!(
        wall(&edit),
        ("2026-07-03T00:00".to_owned(), "2026-07-04T00:00".to_owned())
    );
}

#[test]
fn an_all_day_resize_keeps_at_least_one_whole_day() {
    let event = all_day(2026, 7, 1, 3);
    let edit = apply_event_drag(&event, &drag(EventEdge::End, -9, 0)).unwrap();
    assert_eq!(
        wall(&edit),
        ("2026-07-01T00:00".to_owned(), "2026-07-02T00:00".to_owned())
    );
}

#[test]
fn a_drag_beyond_the_bound_is_refused_rather_than_aborting() {
    // The value crosses the FFI, and the civil-date conversion panics outside representable
    // time. A gesture cannot produce this; a caller can.
    let event = amsterdam(2026, 7, 1, 9, 0);
    assert!(apply_event_drag(&event, &drag(EventEdge::Whole, 5_000, 0)).is_err());
    assert!(apply_event_drag(&event, &drag(EventEdge::Whole, 0, i32::MAX)).is_err());
}

#[test]
fn a_drag_touches_nothing_but_the_times() {
    // It rides `build_event_patch`, which patches rather than rebuilds, but only because the
    // edit leaves every other property `None`. A title or a location set here would be written.
    let event = amsterdam(2026, 7, 1, 9, 0);
    let edit = apply_event_drag(&event, &drag(EventEdge::Whole, 0, 15)).unwrap();
    assert_eq!(edit.title, None);
    assert_eq!(edit.notes, None);
    assert_eq!(edit.location, None);
}

#[test]
fn the_occurrence_rides_through_untouched() {
    let original = LocalDateTime::new(2026, 7, 7, 9, 0, 0).unwrap();
    let event = amsterdam(2026, 7, 1, 9, 0);
    let edit = apply_event_drag(
        &event,
        &EventDrag {
            edge: EventEdge::Whole,
            days: 0,
            minutes: 30,
            occurrence: Some(original),
        },
    )
    .unwrap();
    assert_eq!(edit.occurrence, Some(original));
}

#[test]
fn an_occurrence_token_is_minted_in_the_events_own_zone() {
    // 07:30 UTC is 09:30 in Amsterdam in July, and the `RECURRENCE-ID` a patch must name is
    // the Amsterdam one, whatever zone the grid was drawn in.
    let mut event = amsterdam(2026, 7, 1, 9, 30);
    event.recurrence = Some(Recurrence::from_rule(RecurrenceRule::new(
        Frequency::Weekly,
    )));
    let instant = UtcDateTime::new(2026, 7, 8, 7, 30, 0).unwrap();
    assert_eq!(
        occurrence_wall_clock(&event, instant, &TimeZoneId::utc()),
        Some("2026-07-08T09:30:00".to_owned())
    );
}

#[test]
fn an_all_day_occurrence_token_is_not_localized() {
    // All-day values are zoneless and expand to UTC midnights. Localising one east of UTC drags
    // it onto the next day: the bug `docs/calendar.md` §1 records twice. Amsterdam is east of
    // UTC, so a token reading the 9th here would be that bug.
    let event = all_day(2026, 7, 1, 1);
    let instant = UtcDateTime::new(2026, 7, 8, 0, 0, 0).unwrap();
    assert_eq!(
        occurrence_wall_clock(
            &event,
            instant,
            &TimeZoneId::iana("Europe/Amsterdam").unwrap()
        ),
        Some("2026-07-08T00:00:00".to_owned())
    );
}

#[test]
fn every_occurrence_token_parses_as_the_wall_clock_the_ffi_hands_back() {
    // The token crosses the FFI as a string and comes back to be parsed as a `LocalDateTime`.
    // A bare date for the all-day case would parse on one side and not the other, which is a
    // drag that silently does nothing, on exactly the events nobody tests by hand.
    let instant = UtcDateTime::new(2026, 7, 8, 7, 30, 0).unwrap();
    let zone = TimeZoneId::iana("Europe/Amsterdam").unwrap();
    for event in [amsterdam(2026, 7, 1, 9, 30), all_day(2026, 7, 1, 1)] {
        let token = occurrence_wall_clock(&event, instant, &zone).unwrap();
        assert!(
            token.parse::<LocalDateTime>().is_ok(),
            "{token:?} does not round-trip"
        );
    }
}

#[test]
fn a_floating_occurrence_token_uses_the_zone_the_horizon_was_expanded_in() {
    // A floating event's occurrences resolve through the host zone (engine-recurrence's
    // `host_zone`), so recovering the wall clock has to use the same one.
    let local = LocalDateTime::new(2026, 7, 1, 9, 0, 0).unwrap();
    let mut event = base(CalendarDateTime::Floating(local));
    event.duration = Duration::from_parts(0, 0, 1, 0, 0, 0).unwrap();
    let instant = UtcDateTime::new(2026, 7, 8, 7, 0, 0).unwrap();
    assert_eq!(
        occurrence_wall_clock(
            &event,
            instant,
            &TimeZoneId::iana("Europe/Amsterdam").unwrap()
        ),
        Some("2026-07-08T09:00:00".to_owned())
    );
}

/// A materialized occurrence row for `event`, starting at `start`, optionally an override of
/// the slot at `recurrence_id`.
fn row(event: &Event, start: UtcDateTime, recurrence_id: Option<UtcDateTime>) -> OccurrenceRow {
    OccurrenceRow {
        event: event.id.key().clone(),
        start,
        end: start,
        recurrence_id,
        tzdata_version: TzdataVersion::new("test"),
    }
}

/// 09:30 Amsterdam on 12 January 2026 is 08:30Z; winter, UTC+1.
fn utc(day: u8, hour: u8, minute: u8) -> UtcDateTime {
    UtcDateTime::new(2026, 1, day, hour, minute, 0).unwrap()
}

fn weekly_series() -> Event {
    let mut event = amsterdam(2026, 1, 5, 9, 30);
    event.recurrence = Some(Recurrence::from_rule(RecurrenceRule::new(
        Frequency::Weekly,
    )));
    event
}

#[test]
fn a_token_the_grid_minted_names_a_stored_occurrence() {
    let event = weekly_series();
    let rows = [row(&event, utc(12, 8, 30), None)];

    assert!(names_an_occurrence(
        &event,
        &rows,
        LocalDateTime::new(2026, 1, 12, 9, 30, 0).unwrap(),
        &TimeZoneId::utc(),
    ));
}

#[test]
fn a_time_the_series_does_not_have_names_nothing() {
    // The failure this guard exists for: a token that reads like a plausible wall clock but
    // addresses no instance. Writing it splits an override at a time the rule never produces;
    // or, on a delete, quietly removes nothing at all.
    let event = weekly_series();
    let rows = [row(&event, utc(12, 8, 30), None)];

    for (what, named) in [
        (
            "an hour out",
            LocalDateTime::new(2026, 1, 12, 10, 30, 0).unwrap(),
        ),
        (
            "a day out",
            LocalDateTime::new(2026, 1, 13, 9, 30, 0).unwrap(),
        ),
    ] {
        assert!(
            !names_an_occurrence(&event, &rows, named, &TimeZoneId::utc()),
            "{what} names no occurrence"
        );
    }
}

#[test]
fn a_moved_occurrence_is_named_by_where_it_started() {
    // Its identity is the slot it came from, and it keeps that after the user drags it. Naming
    // the moved time addresses nothing: the second drag of the same Monday would leave the
    // first override standing and split another beside it.
    let event = weekly_series();
    let rows = [row(&event, utc(12, 13, 0), Some(utc(12, 8, 30)))];

    assert!(
        names_an_occurrence(
            &event,
            &rows,
            LocalDateTime::new(2026, 1, 12, 9, 30, 0).unwrap(),
            &TimeZoneId::utc(),
        ),
        "the recurrence id still names it"
    );
    assert!(
        !names_an_occurrence(
            &event,
            &rows,
            LocalDateTime::new(2026, 1, 12, 14, 0, 0).unwrap(),
            &TimeZoneId::utc(),
        ),
        "where it now sits does not"
    );
}

#[test]
fn a_one_off_event_is_named_by_no_token_at_all() {
    // A one-off carries no token, so any is a client asking to split an override out of an
    // event that has no series to split it from.
    let event = amsterdam(2026, 1, 5, 9, 30);
    let rows = [row(&event, utc(5, 8, 30), None)];

    assert!(!names_an_occurrence(
        &event,
        &rows,
        LocalDateTime::new(2026, 1, 5, 9, 30, 0).unwrap(),
        &TimeZoneId::utc(),
    ));
}

#[test]
fn another_events_occurrence_does_not_answer_for_this_one() {
    // The rows come from an account-wide read, so a matching time on a *different* series is
    // the near miss most likely to be there.
    let event = weekly_series();
    let mut other = weekly_series();
    other.id = EventId::try_from("/cal/other.ics").unwrap();
    let rows = [row(&other, utc(12, 8, 30), None)];

    assert!(!names_an_occurrence(
        &event,
        &rows,
        LocalDateTime::new(2026, 1, 12, 9, 30, 0).unwrap(),
        &TimeZoneId::utc(),
    ));
}
