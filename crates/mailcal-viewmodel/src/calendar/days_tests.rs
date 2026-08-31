//! Date-arithmetic cases: the round trip, weekdays, and the day list each view shows.

use super::*;

fn date(raw: &str) -> CalendarDate {
    raw.parse().expect("valid date")
}

fn labels(dates: &[CalendarDate]) -> Vec<String> {
    dates.iter().map(ToString::to_string).collect()
}

#[test]
fn day_numbers_round_trip_including_the_awkward_dates() {
    for raw in [
        "1970-01-01", // the epoch itself
        "1969-12-31", // negative day numbers
        "2026-07-12",
        "2024-02-29", // a leap day
        "2100-03-01", // a century that is NOT a leap year
        "2000-02-29", // a century that IS
        "1899-12-31",
        "2400-01-01",
    ] {
        let parsed = date(raw);
        assert_eq!(
            date_at(day_number(parsed)).to_string(),
            raw,
            "{raw} did not survive the round trip"
        );
    }
    // Consecutive dates are consecutive numbers, across a month and a year boundary.
    assert_eq!(
        day_number(date("2026-08-01")) - day_number(date("2026-07-31")),
        1
    );
    assert_eq!(
        day_number(date("2027-01-01")) - day_number(date("2026-12-31")),
        1
    );
    // The epoch is day zero.
    assert_eq!(day_number(date("1970-01-01")), 0);
}

#[test]
fn weekdays_are_monday_based() {
    // 2026-07-06 is a Monday; 2026-07-12 a Sunday.
    assert_eq!(weekday(day_number(date("2026-07-06"))), 0);
    assert_eq!(weekday(day_number(date("2026-07-10"))), 4); // Friday
    assert_eq!(weekday(day_number(date("2026-07-11"))), 5); // Saturday
    assert_eq!(weekday(day_number(date("2026-07-12"))), 6); // Sunday
    // The epoch was a Thursday.
    assert_eq!(weekday(0), 3);
    // And it holds before the epoch, where a naive `%` would go negative.
    assert_eq!(weekday(day_number(date("1969-12-29"))), 0); // a Monday
}

#[test]
fn the_day_axis_runs_consecutively_from_the_anchor_and_snaps_to_nothing() {
    // This is what lets a client zoom the day axis without the grid relocating: widening three
    // columns to seven keeps the same FIRST day, so the days the user was reading stay where they
    // were. Snapping to a Monday-aligned week instead would have to jump; it cannot contain an
    // arbitrary three-day window.
    assert_eq!(
        labels(&days_from(date("2026-07-09"), 1)),
        vec!["2026-07-09"]
    );
    assert_eq!(
        labels(&days_from(date("2026-07-09"), 3)),
        vec!["2026-07-09", "2026-07-10", "2026-07-11"]
    );
    // Widened to a week, from the SAME first day: nothing the user was looking at has moved.
    assert_eq!(
        labels(&days_from(date("2026-07-09"), 7)),
        vec![
            "2026-07-09",
            "2026-07-10",
            "2026-07-11",
            "2026-07-12",
            "2026-07-13",
            "2026-07-14",
            "2026-07-15",
        ]
    );
    // And it crosses a month boundary without arithmetic trouble.
    assert_eq!(
        labels(&days_from(date("2026-07-31"), 3)),
        vec!["2026-07-31", "2026-08-01", "2026-08-02"]
    );
}

#[test]
fn zero_columns_is_an_empty_axis_rather_than_a_panic() {
    assert!(days_from(date("2026-07-09"), 0).is_empty());
}

#[test]
fn a_week_is_aligned_only_when_a_user_deliberately_asks_for_one() {
    // Alignment is a separate act from choosing a column count, that separation is the whole
    // point. 2026-07-06 is a Monday; 2026-07-12 the Sunday that closes its week.
    for anchor in ["2026-07-06", "2026-07-09", "2026-07-12"] {
        assert_eq!(
            week_start(date(anchor), true).to_string(),
            "2026-07-06",
            "anchor {anchor}"
        );
    }
}

#[test]
fn a_sunday_start_week_shifts_the_whole_run_not_just_the_heading() {
    // Sunday 2026-07-12 OPENS the following week when weeks start on Sunday, but CLOSES the
    // previous one when they start on Monday. Getting this wrong shifts every column, so the
    // user reads Tuesday's meetings under Monday's heading.
    assert_eq!(
        week_start(date("2026-07-12"), false).to_string(),
        "2026-07-12"
    );
    assert_eq!(
        week_start(date("2026-07-12"), true).to_string(),
        "2026-07-06"
    );

    // A whole Sunday-start week, from the aligned first day.
    assert_eq!(
        labels(&days_from(week_start(date("2026-07-12"), false), 7)),
        vec![
            "2026-07-12",
            "2026-07-13",
            "2026-07-14",
            "2026-07-15",
            "2026-07-16",
            "2026-07-17",
            "2026-07-18",
        ]
    );
}

#[test]
fn a_work_week_is_the_first_five_days_of_an_aligned_week() {
    assert_eq!(
        labels(&days_from(week_start(date("2026-07-09"), true), 5)),
        vec![
            "2026-07-06",
            "2026-07-07",
            "2026-07-08",
            "2026-07-09",
            "2026-07-10",
        ]
    );
}
