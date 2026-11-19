use engine_api::CalendarDate;

use super::{Stride, step};

fn date(raw: &str) -> CalendarDate {
    raw.parse().unwrap()
}

/// A Thursday far from the dates stepped through, so no step below lands on its week or month.
fn today() -> CalendarDate {
    date("2026-11-19")
}

fn moved(focus: &str, stride: Stride, steps: i32) -> String {
    step(date(focus), stride, steps, today(), true).to_string()
}

#[test]
fn a_month_step_keeps_the_day_of_the_month() {
    // Outlook's rule, and the one a person can predict: the 13th of September, a month on, is the
    // 13th of October, so the week opened from there is the week of the 13th.
    assert_eq!(moved("2027-09-13", Stride::Month, 1), "2027-10-13");
    assert_eq!(moved("2027-09-13", Stride::Month, -3), "2027-06-13");
    assert_eq!(moved("2027-11-13", Stride::Month, 2), "2028-01-13");
    assert_eq!(moved("2028-01-13", Stride::Month, -2), "2027-11-13");
}

#[test]
fn a_month_too_short_for_the_day_lands_on_its_last_day() {
    assert_eq!(moved("2027-01-31", Stride::Month, 1), "2027-02-28");
    assert_eq!(moved("2028-01-31", Stride::Month, 1), "2028-02-29");
    assert_eq!(moved("2027-03-31", Stride::Month, -1), "2027-02-28");
    assert_eq!(moved("2027-05-31", Stride::Month, 1), "2027-06-30");
}

#[test]
fn the_grid_strides_by_its_own_length() {
    assert_eq!(moved("2027-09-13", Stride::Day, 1), "2027-09-14");
    assert_eq!(moved("2027-09-13", Stride::ThreeDays, -1), "2027-09-10");
    assert_eq!(moved("2027-09-13", Stride::Week, 1), "2027-09-20");
    assert_eq!(moved("2027-12-29", Stride::Week, 1), "2028-01-05");
}

#[test]
fn a_step_into_the_month_that_holds_today_lands_on_today() {
    // Paging back to this month and opening the week shows this week, not the 13th's.
    assert_eq!(moved("2026-12-13", Stride::Month, -1), "2026-11-19");
    assert_eq!(moved("2027-09-13", Stride::Month, -10), "2026-11-19");
    // And out of it again carries today's day of the month.
    assert_eq!(moved("2026-11-19", Stride::Month, 1), "2026-12-19");
}

#[test]
fn a_step_into_the_week_that_holds_today_lands_on_today() {
    // Monday the 9th, a week on, is in today's week (Monday the 16th to Sunday the 22nd).
    assert_eq!(moved("2026-11-09", Stride::Week, 1), "2026-11-19");
    assert_eq!(moved("2026-11-30", Stride::Week, -2), "2026-11-19");
    // The week is the user's: with a Sunday start, Sunday the 15th opens today's week too.
    let sunday_start = step(date("2026-11-08"), Stride::Week, 1, today(), false);
    assert_eq!(sunday_start.to_string(), "2026-11-19");
}

#[test]
fn a_day_or_three_days_never_jump_to_today() {
    // Their first column is the focus itself, so moving it to today would move the days shown.
    assert_eq!(moved("2026-11-17", Stride::ThreeDays, 1), "2026-11-20");
    assert_eq!(moved("2026-11-18", Stride::Day, -1), "2026-11-17");
}

#[test]
fn no_step_leaves_the_focus_where_it_is() {
    assert_eq!(moved("2026-11-02", Stride::Week, 0), "2026-11-02");
    assert_eq!(moved("2026-11-02", Stride::Month, 0), "2026-11-02");
}
