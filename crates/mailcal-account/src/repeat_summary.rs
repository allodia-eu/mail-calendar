//! Which sentence describes a repeat rule; decided here, once, for every client.
//!
//! The wording is a client's job and stays one ([`crate::recurrence_shape`] carries the rule
//! itself, which an editor seeds from). What is *not* a client's job is deciding **which** of a
//! closed set of sentences a rule gets, because that decision is product logic and every part of
//! it is a trap:
//!
//! - An empty `days` / `month_days` / `months` list is not a gap to fill in. It means the rule
//!   takes that part from the event's own start, so a weekly rule that names no weekday is still
//!   *on a weekday*: the one the event starts on.
//! - A rule this cannot state **exactly** returns `None`, and a client says only that the event
//!   repeats. Approximating states a series the user does not have, and nothing on screen would
//!   tell them apart. It is the judgement [`describe_recurrence`] already makes deciding `Simple`
//!   against `Complex`, one layer down.
//!
//! Four clients each writing that from the same doc is four sets of disagreements, and only the
//! one a reader happens to be looking at is visible.
//!
//! Its refusals are **not** [`undrawable_reason`]'s and the two must not be merged. That one
//! gates a *write*, asking whether the expander will draw the rule at all; this one gates a
//! *sentence*. Two days of the month draw perfectly well and there are no words here for them.
//!
//! [`undrawable_reason`]: crate::recurrence_shape::undrawable_reason
//! [`describe_recurrence`]: crate::recurrence_shape::describe_recurrence

use engine_core::time::CalendarDate;
use mailcal_viewmodel::calendar::days::{day_number, weekday};

use crate::recurrence_shape::{
    RecurrenceEnd, RecurrenceFrequency, RecurrenceWeekday, SimpleRecurrence,
};

/// The rhythm a rule repeats on: one variant per sentence a client has words for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepeatRhythm {
    /// Every day, or every `interval` days.
    Daily {
        /// Periods between instances; `1` is every one.
        interval: u32,
    },
    /// On `days`, every week or every `interval` weeks. Never empty, and in week order.
    Weekly {
        /// Periods between instances.
        interval: u32,
        /// The weekdays, Monday first.
        days: Vec<RecurrenceWeekday>,
    },
    /// On one day of the month, counted from its start.
    MonthlyOnDay {
        /// Periods between instances.
        interval: u32,
        /// The day of the month, 1–31.
        day: u32,
    },
    /// On the month's last day, whichever date that turns out to be.
    MonthlyOnLastDay {
        /// Periods between instances.
        interval: u32,
    },
    /// On a weekday's position in the month.
    MonthlyOnWeekday {
        /// Periods between instances.
        interval: u32,
        /// Which one: `1`–`5`, or `-1` for the last.
        nth: i32,
        /// The weekday.
        day: RecurrenceWeekday,
    },
    /// On one date of the year.
    YearlyOnDate {
        /// Periods between instances.
        interval: u32,
        /// The month, 1–12.
        month: u32,
        /// The day of that month, 1–31.
        day: u32,
    },
    /// On a weekday's position inside one month of the year.
    YearlyOnWeekday {
        /// Periods between instances.
        interval: u32,
        /// Which one: `1`–`5`, or `-1` for the last.
        nth: i32,
        /// The weekday.
        day: RecurrenceWeekday,
        /// The month it is counted in, 1–12.
        month: u32,
    },
}

/// When a repeat stops, in the terms a sentence needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepeatStop {
    /// It does not.
    Never,
    /// After a date; `YYYY-MM-DD`, in the event's own zone. A client formats it.
    OnDate {
        /// The last date an instance may start on.
        date: String,
    },
    /// After a fixed number of instances, counting the first.
    AfterCount {
        /// How many instances in total.
        count: u32,
    },
}

/// A rule reduced to the two things a sentence needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepeatSummary {
    /// How often, and on what.
    pub rhythm: RepeatRhythm,
    /// What ends it.
    pub stop: RepeatStop,
}

/// A weekday position there is a word for: the first five, or the last.
const STATEABLE_POSITIONS: [i32; 6] = [1, 2, 3, 4, 5, -1];

/// The largest day of the month a rule may name.
const DAYS_IN_LONGEST_MONTH: i32 = 31;

/// Reduce `rule` to the sentence that describes it, read against the event's own `start`.
///
/// `None` means **we cannot say this exactly**; see the module docs.
#[must_use]
pub fn summarize_repeat(rule: &SimpleRecurrence, start: CalendarDate) -> Option<RepeatSummary> {
    let stop = repeat_stop(&rule.end)?;
    Some(RepeatSummary {
        rhythm: repeat_rhythm(rule, start)?,
        stop,
    })
}

#[expect(
    clippy::too_many_lines,
    reason = "one arm per frequency, each a flat match over which parts the rule names; \
              splitting it hides that the four are the same decision made four ways"
)]
fn repeat_rhythm(rule: &SimpleRecurrence, start: CalendarDate) -> Option<RepeatRhythm> {
    let interval = rule.interval;
    if interval == 0 {
        return None;
    }
    let days = &rule.days;
    let month_days = &rule.month_days;
    let months = &rule.months;
    if months.iter().any(|month| !(1..=12).contains(month)) {
        return None;
    }
    match rule.frequency {
        RecurrenceFrequency::Daily => {
            if days.is_empty() && month_days.is_empty() && months.is_empty() {
                Some(RepeatRhythm::Daily { interval })
            } else {
                None
            }
        }

        RecurrenceFrequency::Weekly => {
            if !month_days.is_empty() || !months.is_empty() {
                return None;
            }
            if days.iter().any(|day| day.nth.is_some()) {
                return None;
            }
            let mut weekdays: Vec<RecurrenceWeekday> = if days.is_empty() {
                vec![start_weekday(start)]
            } else {
                days.iter().map(|day| day.day).collect()
            };
            weekdays.sort_unstable_by_key(|day| week_order(*day));
            weekdays.dedup();
            Some(RepeatRhythm::Weekly {
                interval,
                days: weekdays,
            })
        }

        RecurrenceFrequency::Monthly => {
            if !months.is_empty() {
                return None;
            }
            match (days.as_slice(), month_days.as_slice()) {
                ([], []) => Some(RepeatRhythm::MonthlyOnDay {
                    interval,
                    day: u32::from(start.day()),
                }),
                ([], [-1]) => Some(RepeatRhythm::MonthlyOnLastDay { interval }),
                ([], [day]) if (1..=DAYS_IN_LONGEST_MONTH).contains(day) => {
                    Some(RepeatRhythm::MonthlyOnDay {
                        interval,
                        day: day.unsigned_abs(),
                    })
                }
                ([day], [])
                    if day
                        .nth
                        .is_some_and(|nth| STATEABLE_POSITIONS.contains(&nth)) =>
                {
                    Some(RepeatRhythm::MonthlyOnWeekday {
                        interval,
                        nth: day.nth?,
                        day: day.day,
                    })
                }
                _ => None,
            }
        }

        RecurrenceFrequency::Yearly => {
            let month = months
                .first()
                .copied()
                .unwrap_or_else(|| u32::from(start.month()));
            if months.len() > 1 {
                return None;
            }
            match (days.as_slice(), month_days.as_slice()) {
                ([], []) => Some(RepeatRhythm::YearlyOnDate {
                    interval,
                    month,
                    day: u32::from(start.day()),
                }),
                ([], [day]) if (1..=DAYS_IN_LONGEST_MONTH).contains(day) => {
                    Some(RepeatRhythm::YearlyOnDate {
                        interval,
                        month,
                        day: day.unsigned_abs(),
                    })
                }
                // A yearly position must name its month: positions are counted per month, and a
                // rule naming none is one `undrawable_reason` refuses to write in the first place.
                ([day], [])
                    if !months.is_empty()
                        && day
                            .nth
                            .is_some_and(|nth| STATEABLE_POSITIONS.contains(&nth)) =>
                {
                    Some(RepeatRhythm::YearlyOnWeekday {
                        interval,
                        nth: day.nth?,
                        day: day.day,
                        month,
                    })
                }
                _ => None,
            }
        }
    }
}

/// The stop, or `None` for one we cannot read.
///
/// Dropping an end we cannot read would describe a series that never stops, which is a claim
/// rather than a summary: so it takes the whole rule out of the stateable set with it.
fn repeat_stop(end: &RecurrenceEnd) -> Option<RepeatStop> {
    match end {
        RecurrenceEnd::Never => Some(RepeatStop::Never),
        RecurrenceEnd::OnDate { date } => date_part(date).map(|date| RepeatStop::OnDate { date }),
        RecurrenceEnd::AfterCount { count } => {
            (*count > 0).then_some(RepeatStop::AfterCount { count: *count })
        }
    }
}

/// The `YYYY-MM-DD` of a wall clock this crate itself wrote, checked rather than assumed.
fn date_part(wall_clock: &str) -> Option<String> {
    let date = wall_clock.split('T').next()?;
    let mut parts = date.split('-');
    let year: i32 = parts.next()?.parse().ok()?;
    let month: u8 = parts.next()?.parse().ok()?;
    let day: u8 = parts.next()?.parse().ok()?;
    if parts.next().is_some() || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    Some(format!("{year:04}-{month:02}-{day:02}"))
}

/// The weekday an event starting on `start` falls on.
pub(crate) fn start_weekday(start: CalendarDate) -> RecurrenceWeekday {
    match weekday(day_number(start)) {
        0 => RecurrenceWeekday::Monday,
        1 => RecurrenceWeekday::Tuesday,
        2 => RecurrenceWeekday::Wednesday,
        3 => RecurrenceWeekday::Thursday,
        4 => RecurrenceWeekday::Friday,
        5 => RecurrenceWeekday::Saturday,
        _ => RecurrenceWeekday::Sunday,
    }
}

/// Monday-first order, so a listed set of weekdays reads as a week rather than as it arrived.
pub(crate) fn week_order(day: RecurrenceWeekday) -> u8 {
    match day {
        RecurrenceWeekday::Monday => 0,
        RecurrenceWeekday::Tuesday => 1,
        RecurrenceWeekday::Wednesday => 2,
        RecurrenceWeekday::Thursday => 3,
        RecurrenceWeekday::Friday => 4,
        RecurrenceWeekday::Saturday => 5,
        RecurrenceWeekday::Sunday => 6,
    }
}

#[cfg(test)]
#[path = "repeat_summary_tests.rs"]
mod tests;
