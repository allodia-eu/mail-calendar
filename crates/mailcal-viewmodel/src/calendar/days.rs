//! Calendar-date arithmetic: day numbers, weekdays, and the day list a view shows.
//!
//! The day axis is **civil day numbers** (days from 1970-01-01) not date strings. A date
//! string can only be compared against the days already on screen, and a grid needs
//! arithmetic that works off screen too: "the day before this one" for an event whose
//! exclusive-midnight end falls outside the window, or "the Monday of this week" when the
//! anchor is a Sunday.

use engine_api::CalendarDate;

/// Days from the civil epoch (1970-01-01) for a proleptic-Gregorian date; Howard
/// Hinnant's `days_from_civil`, exact for every date the engine can represent and free of
/// leap-year special cases.
#[must_use]
pub fn day_number(date: CalendarDate) -> i64 {
    from_civil(date.year(), date.month(), date.day())
}

/// The day-number form of a `(year, month, day)` triple.
#[must_use]
pub fn from_civil(year: i32, month: u8, day: u8) -> i64 {
    let (y, m, d) = (i64::from(year), i64::from(month), i64::from(day));
    // Shift the year to start in March, so a leap day falls at the end of a "year".
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let year_of_era = y - era * 400;
    let day_of_year = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// The calendar date at a day number: the inverse of [`day_number`].
///
/// # Panics
///
/// Panics only on a day number outside representable calendar time, which the callers here
/// cannot produce (they offset a real date by at most a few days).
#[must_use]
pub fn date_at(day: i64) -> CalendarDate {
    let z = day + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day_of_month = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    };
    let year = if month <= 2 { year + 1 } else { year };
    CalendarDate::new(
        i32::try_from(year).expect("a representable year"),
        u8::try_from(month).expect("1..=12"),
        u8::try_from(day_of_month).expect("1..=31"),
    )
    .expect("a real calendar date")
}

/// The weekday of a day number: `0` = Monday … `6` = Sunday.
///
/// Monday-based because Europe is, and because `WorkWeek` then falls out as "the first five".
#[must_use]
pub fn weekday(day: i64) -> u8 {
    // 1970-01-01 (day 0) was a Thursday, which is index 3 in a Monday-based week.
    u8::try_from((day + 3).rem_euclid(7)).expect("0..=6")
}

/// The `columns` consecutive days starting at `from`: the day axis of every time grid.
///
/// Consecutive **from the anchor**, not snapped to anything. That is what lets the user zoom the
/// day axis without the grid relocating: widening a three-day view to seven keeps the same first
/// day, so the days they were reading stay where they were.
///
/// Week *alignment* is a separate, deliberate act (see [`week_start`]) applied when a user picks
/// "Week" from a menu, not every time the column count changes.
#[must_use]
pub fn days_from(from: CalendarDate, columns: u32) -> Vec<CalendarDate> {
    let day = day_number(from);
    (0..i64::from(columns))
        .map(|step| date_at(day + step))
        .collect()
}

/// The first day of the week containing `date`.
///
/// `week_starts_monday` picks it: Monday across Europe, Sunday in the US and much of Asia. Get it
/// wrong and every column of an aligned week shifts, so the user reads Tuesday's meetings under
/// Monday's heading, which is why the core owns the setting and no client passes it.
#[must_use]
pub fn week_start(date: CalendarDate, week_starts_monday: bool) -> CalendarDate {
    let day = day_number(date);
    let index = i64::from(weekday(day));
    // Monday-based indices, so a Sunday-start week is the same run rotated one day back.
    let offset = if week_starts_monday {
        index
    } else {
        (index + 1) % 7
    };
    date_at(day - offset)
}

#[cfg(test)]
#[path = "days_tests.rs"]
mod days_tests;
