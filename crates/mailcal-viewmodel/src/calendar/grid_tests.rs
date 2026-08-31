//! Time-grid cases: the ones that break real calendars.
//!
//! Midnight crossings, all-day off-by-ones, multi-day banners, and the two DST days;
//! each of which renders plausibly-but-wrongly if you get it slightly off, which is
//! exactly why they are pinned here rather than eyeballed on a screen.

use super::*;

fn zone() -> TimeZoneId {
    TimeZoneId::iana("Europe/Amsterdam").expect("valid zone")
}

fn date(raw: &str) -> CalendarDate {
    raw.parse().expect("valid date")
}

/// The week of Mon 2026-07-06 .. Sun 2026-07-12.
fn week() -> Vec<CalendarDate> {
    (6..=12)
        .map(|day| date(&format!("2026-07-{day:02}")))
        .collect()
}

fn at(raw: &str) -> UtcDateTime {
    raw.parse().expect("valid instant")
}

/// A timed occurrence. Times are UTC; Amsterdam is UTC+2 in July.
fn timed(title: &str, start: &str, end: &str) -> Occurrence {
    Occurrence {
        account: "acct".into(),
        event: title.into(),
        calendar: "work".into(),
        title: title.into(),
        start: at(start),
        end: at(end),
        all_day: false,
        can_write: true,
        can_move: true,
        // Empty is "does not recur", which is what a layout fixture wants: the drag cases pin
        // the token separately.
        occurrence_start: String::new(),
        // Most cases here are about layout, so the default is a settled commitment; the
        // unanswered-hold case sets this explicitly.
        participation: crate::invitation::ResponseStatus::Accepted,
    }
}

fn all_day(title: &str, start: &str, end: &str) -> Occurrence {
    Occurrence {
        all_day: true,
        ..timed(title, start, end)
    }
}

/// `(day, start, end, column, columns)` per block, for terse assertions.
fn blocks(grid: &TimeGrid) -> Vec<(u32, u32, u32, u32, u32)> {
    grid.timed
        .iter()
        .map(|s| (s.day, s.start_minutes, s.end_minutes, s.column, s.columns))
        .collect()
}

#[test]
fn an_event_lands_in_its_local_day_column_at_its_wall_clock() {
    // 07:00Z on Wednesday is 09:00 local (UTC+2); column 2 (Mon=0), minute 540.
    let grid = build(
        &week(),
        &[timed(
            "standup",
            "2026-07-08T07:00:00Z",
            "2026-07-08T07:15:00Z",
        )],
        &zone(),
    );
    assert_eq!(grid.days.len(), 7);
    assert_eq!(grid.days[0].date, "2026-07-06");
    assert_eq!(blocks(&grid), vec![(2, 540, 555, 0, 1)]);
    assert_eq!(grid.timezone, "Europe/Amsterdam");
}

#[test]
fn an_event_late_enough_to_shift_a_day_in_the_local_zone_moves_column() {
    // 23:30Z Monday is 01:30 local on TUESDAY. Placing it by its UTC date would draw it
    // on the wrong day: the bug that makes a late-evening meeting appear yesterday.
    let grid = build(
        &week(),
        &[timed(
            "late",
            "2026-07-06T23:30:00Z",
            "2026-07-07T00:30:00Z",
        )],
        &zone(),
    );
    assert_eq!(blocks(&grid), vec![(1, 90, 150, 0, 1)]);
}

#[test]
fn an_event_crossing_midnight_splits_into_one_block_per_day() {
    // 21:00 → 01:00 local (19:00Z Tue → 23:00Z Tue). Two blocks: Tuesday 21:00–24:00 with
    // an open bottom, Wednesday 00:00–01:00 with an open top. A single block cannot be
    // drawn; it would have to leave its column.
    let grid = build(
        &week(),
        &[timed(
            "party",
            "2026-07-07T19:00:00Z",
            "2026-07-07T23:00:00Z",
        )],
        &zone(),
    );
    assert_eq!(blocks(&grid), vec![(1, 1260, 1440, 0, 1), (2, 0, 60, 0, 1)]);
    assert!(!grid.timed[0].continues_before && grid.timed[0].continues_after);
    assert!(grid.timed[1].continues_before && !grid.timed[1].continues_after);
}

#[test]
fn an_event_ending_exactly_at_midnight_does_not_open_a_sliver_on_the_next_day() {
    // 22:00 → 00:00 local. The end is exclusive, so it belongs wholly to Tuesday. A naive
    // split emits a second, zero-height block at the top of Wednesday: a hairline artifact
    // that also steals a tap target.
    let grid = build(
        &week(),
        &[timed(
            "close",
            "2026-07-07T20:00:00Z",
            "2026-07-07T22:00:00Z",
        )],
        &zone(),
    );
    assert_eq!(blocks(&grid), vec![(1, 1320, 1440, 0, 1)]);
    assert!(!grid.timed[0].continues_after);
}

#[test]
fn a_zero_length_event_still_gets_a_tappable_block() {
    let grid = build(
        &week(),
        &[timed(
            "ping",
            "2026-07-08T07:00:00Z",
            "2026-07-08T07:00:00Z",
        )],
        &zone(),
    );
    assert_eq!(blocks(&grid), vec![(2, 540, 555, 0, 1)]);
}

#[test]
fn overlapping_events_split_the_column_within_their_own_day() {
    // Two clashing meetings on Wednesday, and a lone one on Thursday. The Thursday meeting
    // must stay full width; columns are solved per day, so Wednesday's pile-up cannot
    // narrow it.
    let grid = build(
        &week(),
        &[
            timed("a", "2026-07-08T08:00:00Z", "2026-07-08T10:00:00Z"),
            timed("b", "2026-07-08T09:00:00Z", "2026-07-08T11:00:00Z"),
            timed("c", "2026-07-09T08:00:00Z", "2026-07-09T09:00:00Z"),
        ],
        &zone(),
    );
    assert_eq!(
        blocks(&grid),
        vec![
            (2, 600, 720, 0, 2),
            (2, 660, 780, 1, 2),
            (3, 600, 660, 0, 1),
        ]
    );
}

#[test]
fn an_all_day_event_covers_exactly_its_own_day() {
    // A date-only event is `[2026-07-08, 2026-07-09)`; its end is the *next* midnight. If
    // that exclusive end is treated as an inclusive day, every one-day event renders two
    // days wide, which is the classic all-day off-by-one.
    let grid = build(
        &week(),
        &[all_day(
            "holiday",
            "2026-07-08T00:00:00Z",
            "2026-07-09T00:00:00Z",
        )],
        &zone(),
    );
    assert!(grid.timed.is_empty());
    assert_eq!(grid.all_day.len(), 1);
    assert_eq!((grid.all_day[0].day, grid.all_day[0].days), (2, 1));
    assert_eq!(grid.all_day_lanes, 1);
    assert!(!grid.all_day[0].continues_before && !grid.all_day[0].continues_after);
}

#[test]
fn an_all_day_bar_names_the_occurrence_it_draws() {
    // A banner bar is one occurrence like a block is, and a client reaches an edit or a delete
    // from it the same way. Dropping the token here is invisible on screen and only shows up as
    // a whole series going when the user meant one day of it.
    let repeating = Occurrence {
        occurrence_start: "2026-07-08T00:00:00".into(),
        ..all_day("holiday", "2026-07-08T00:00:00Z", "2026-07-09T00:00:00Z")
    };
    let grid = build(&week(), &[repeating], &zone());
    assert_eq!(grid.all_day[0].occurrence_start, "2026-07-08T00:00:00");
}

#[test]
fn a_multi_day_event_bands_across_the_days_it_covers() {
    // Wed–Fri inclusive.
    let grid = build(
        &week(),
        &[all_day(
            "offsite",
            "2026-07-08T00:00:00Z",
            "2026-07-11T00:00:00Z",
        )],
        &zone(),
    );
    assert_eq!((grid.all_day[0].day, grid.all_day[0].days), (2, 3));
}

#[test]
fn an_event_running_past_the_shown_week_is_clipped_with_open_edges() {
    // Starts the previous Saturday, ends the following Tuesday: it covers the whole week,
    // and both edges are open so the client draws it running off both sides.
    let grid = build(
        &week(),
        &[all_day(
            "leave",
            "2026-07-04T00:00:00Z",
            "2026-07-15T00:00:00Z",
        )],
        &zone(),
    );
    assert_eq!((grid.all_day[0].day, grid.all_day[0].days), (0, 7));
    assert!(grid.all_day[0].continues_before && grid.all_day[0].continues_after);
}

#[test]
fn a_long_timed_booking_bands_instead_of_drawing_a_full_height_block() {
    // A 26-hour timed event is a banner, not a block: drawn in the grid it would be a
    // full-height bar in two columns, which reads as two separate all-day events.
    let grid = build(
        &week(),
        &[timed(
            "shift",
            "2026-07-08T06:00:00Z",
            "2026-07-09T08:00:00Z",
        )],
        &zone(),
    );
    assert!(grid.timed.is_empty());
    assert_eq!((grid.all_day[0].day, grid.all_day[0].days), (2, 2));

    // ...but a 23-hour one is still a block, split across the two days it touches.
    let grid = build(
        &week(),
        &[timed(
            "long",
            "2026-07-08T06:00:00Z",
            "2026-07-09T05:00:00Z",
        )],
        &zone(),
    );
    assert!(grid.all_day.is_empty());
    assert_eq!(grid.timed.len(), 2);
}

#[test]
fn banner_bars_stack_into_lanes_and_reuse_a_lane_once_it_is_free() {
    let grid = build(
        &week(),
        &[
            all_day("mon-wed", "2026-07-06T00:00:00Z", "2026-07-09T00:00:00Z"),
            all_day("tue-thu", "2026-07-07T00:00:00Z", "2026-07-10T00:00:00Z"),
            // Starts after `mon-wed` ends, so it drops back into lane 0 rather than
            // opening a third: the banner stays as short as it can.
            all_day("thu", "2026-07-09T00:00:00Z", "2026-07-10T00:00:00Z"),
        ],
        &zone(),
    );
    let lanes: Vec<(u32, u32, u32)> = grid
        .all_day
        .iter()
        .map(|b| (b.day, b.days, b.lane))
        .collect();
    assert_eq!(lanes, vec![(0, 3, 0), (1, 3, 1), (3, 1, 0)]);
    assert_eq!(grid.all_day_lanes, 2);
}

#[test]
fn events_outside_the_shown_days_are_dropped() {
    let grid = build(
        &week(),
        &[timed(
            "last month",
            "2026-06-08T07:00:00Z",
            "2026-06-08T08:00:00Z",
        )],
        &zone(),
    );
    assert!(grid.timed.is_empty() && grid.all_day.is_empty());
    assert_eq!(grid.all_day_lanes, 0);
}

// --- DST: the grid is wall-clock, so both edge days render like any other ------------

#[test]
fn on_the_spring_forward_day_an_event_sits_at_its_wall_clock_not_its_elapsed_minutes() {
    // 2026-03-29: the clocks jump 02:00 → 03:00, so the day is 23 hours long. 08:00Z is
    // 10:00 local. Counting *elapsed* minutes from local midnight would put it at 09:00
    // (540): an hour early, all day, once a year.
    let days = vec![date("2026-03-29")];
    let grid = build(
        &days,
        &[timed(
            "brunch",
            "2026-03-29T08:00:00Z",
            "2026-03-29T09:00:00Z",
        )],
        &zone(),
    );
    assert_eq!(blocks(&grid), vec![(0, 600, 660, 0, 1)]);
}

#[test]
fn on_the_fall_back_day_the_repeated_hour_does_not_move_an_event() {
    // 2026-10-25: 03:00 → 02:00 repeats an hour, so the day is 25 hours long. 09:00Z is
    // 10:00 local (UTC+1, the transition already past).
    let days = vec![date("2026-10-25")];
    let grid = build(
        &days,
        &[timed(
            "brunch",
            "2026-10-25T09:00:00Z",
            "2026-10-25T10:00:00Z",
        )],
        &zone(),
    );
    assert_eq!(blocks(&grid), vec![(0, 600, 660, 0, 1)]);
}

#[test]
fn an_unresolvable_zone_drops_the_block_rather_than_misplacing_it() {
    // A custom/embedded VTIMEZONE has no position on a grid. It is not lost: the agenda
    // still lists it, but guessing a column would silently draw it at the wrong time.
    let custom = TimeZoneId::custom("X-CUSTOM").expect("custom zone");
    let grid = build(
        &week(),
        &[timed(
            "mystery",
            "2026-07-08T07:00:00Z",
            "2026-07-08T08:00:00Z",
        )],
        &custom,
    );
    assert!(grid.timed.is_empty());
}
