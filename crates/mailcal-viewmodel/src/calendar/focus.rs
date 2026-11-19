//! The calendar's focus date: the one day every view is drawn around.
//!
//! A cross-platform product decision (`docs/calendar.md`, "The focus date"). Every view shows the
//! period that contains the focus, and switching views never moves it, so a week, then the month,
//! then the week again comes back to the same week. The calendar's previous and next move it by one
//! period, which is what [`step`] works out:
//!
//! - a day by one day, three days by three, a week (and a work week) by seven, a month by one month
//!   keeping the day of the month, the last day of a shorter month standing in for a day it lacks;
//! - a step that lands on a **week or a month that holds today** lands on today itself, so paging
//!   back to this month and opening the week shows this week, not the week of the day carried
//!   there. A day and three days are anchored on the focus itself, so moving it there would move
//!   the days shown, and they carry it as it is.

use engine_api::CalendarDate;

use super::days::{date_at, day_number, from_civil, week_start};

/// What one step of the calendar's previous and next moves the focus by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stride {
    /// One day.
    Day,
    /// Three days.
    ThreeDays,
    /// A week, or a work week.
    Week,
    /// A calendar month.
    Month,
}

/// Where `steps` strides from `focus` land (negative steps go back), with `today` taking the place
/// of a landing in the week or month that holds it.
#[must_use]
pub fn step(
    focus: CalendarDate,
    stride: Stride,
    steps: i32,
    today: CalendarDate,
    week_starts_monday: bool,
) -> CalendarDate {
    if steps == 0 {
        return focus;
    }
    let landed = match stride {
        Stride::Day => shift_days(focus, i64::from(steps)),
        Stride::ThreeDays => shift_days(focus, 3 * i64::from(steps)),
        Stride::Week => shift_days(focus, 7 * i64::from(steps)),
        Stride::Month => add_months(focus, steps),
    };
    let holds_today = match stride {
        Stride::Week => {
            week_start(landed, week_starts_monday) == week_start(today, week_starts_monday)
        }
        Stride::Month => landed.year() == today.year() && landed.month() == today.month(),
        Stride::Day | Stride::ThreeDays => false,
    };
    if holds_today { today } else { landed }
}

fn shift_days(date: CalendarDate, days: i64) -> CalendarDate {
    date_at(day_number(date) + days)
}

/// `date` moved by `months` calendar months, on the same day of the month, or on the last day of a
/// month too short to have it: the 31st of January, a month on, is the 28th of February.
fn add_months(date: CalendarDate, months: i32) -> CalendarDate {
    let index = date.year() * 12 + i32::from(date.month()) - 1 + months;
    let year = index.div_euclid(12);
    let month = u8::try_from(index.rem_euclid(12) + 1).unwrap_or(1);
    let first = from_civil(year, month, 1);
    let (next_year, next_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    let length = from_civil(next_year, next_month, 1) - first;
    let day = i64::from(date.day()).min(length);
    date_at(first + day - 1)
}

#[cfg(test)]
#[path = "focus_tests.rs"]
mod focus_tests;
