//! The repeat rule as the parts a sentence needs, across the FFI.
//!
//! Distinct from [`SimpleRecurrence`](crate::SimpleRecurrence), which is the rule an **editor**
//! seeds from and writes back. This is the rule a client **states**: the core has already read
//! the event's start for every part the rule leaves out, dropped the rules it cannot state
//! exactly, and put the weekdays in week order. A client is left with a `match` over a closed
//! set and its own words for each arm.

use mailcal_account::{RepeatRhythm as CoreRhythm, RepeatStop as CoreStop};

use crate::records_recurrence::RecurrenceWeekday;

/// The rhythm a rule repeats on: one variant per sentence a client has words for.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
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
        /// The weekdays, Monday first: the event's own when the rule names none.
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
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum RepeatStop {
    /// It does not.
    Never,
    /// After a date; `YYYY-MM-DD`, in the event's own zone. The client formats it, as it
    /// formats every other date.
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
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct RepeatSummary {
    /// How often, and on what.
    pub rhythm: RepeatRhythm,
    /// What ends it.
    pub stop: RepeatStop,
}

impl From<mailcal_account::RepeatSummary> for RepeatSummary {
    fn from(summary: mailcal_account::RepeatSummary) -> Self {
        Self {
            rhythm: summary.rhythm.into(),
            stop: summary.stop.into(),
        }
    }
}

impl From<CoreRhythm> for RepeatRhythm {
    fn from(rhythm: CoreRhythm) -> Self {
        match rhythm {
            CoreRhythm::Daily { interval } => Self::Daily { interval },
            CoreRhythm::Weekly { interval, days } => Self::Weekly {
                interval,
                days: days.into_iter().map(Into::into).collect(),
            },
            CoreRhythm::MonthlyOnDay { interval, day } => Self::MonthlyOnDay { interval, day },
            CoreRhythm::MonthlyOnLastDay { interval } => Self::MonthlyOnLastDay { interval },
            CoreRhythm::MonthlyOnWeekday { interval, nth, day } => Self::MonthlyOnWeekday {
                interval,
                nth,
                day: day.into(),
            },
            CoreRhythm::YearlyOnDate {
                interval,
                month,
                day,
            } => Self::YearlyOnDate {
                interval,
                month,
                day,
            },
            CoreRhythm::YearlyOnWeekday {
                interval,
                nth,
                day,
                month,
            } => Self::YearlyOnWeekday {
                interval,
                nth,
                day: day.into(),
                month,
            },
        }
    }
}

impl From<CoreStop> for RepeatStop {
    fn from(stop: CoreStop) -> Self {
        match stop {
            CoreStop::Never => Self::Never,
            CoreStop::OnDate { date } => Self::OnDate { date },
            CoreStop::AfterCount { count } => Self::AfterCount { count },
        }
    }
}
