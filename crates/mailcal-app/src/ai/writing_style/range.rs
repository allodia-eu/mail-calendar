//! Which sent mail a style is learned from (`docs/ai.md` rule 11, `ChooseRange`): the presets the
//! core offers, so a client converts no dates. A range may reach back past what the device holds,
//! since the review fetches the older Sent mail it needs from the server.
//!
//! A day is a whole day in UTC, counted as the account's sync window counts its first day.

use mailcal_account::SyncDepth;
use time::{Date, Month, OffsetDateTime, Time};

/// How far back the device holds an account's sent mail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SentReach {
    /// Everything the server holds.
    Everything,
    /// Mail from this day on.
    Since(Date),
}

impl SentReach {
    /// What a sync depth holds on `today`.
    #[must_use]
    pub fn of(depth: SyncDepth, today: Date) -> Self {
        depth.cutoff(today).map_or(Self::Everything, Self::Since)
    }

    /// Whether mail held this far back holds everything a range starting on `first_day` reaches
    /// (`None`: a range with no first day).
    #[must_use]
    pub fn covers(self, first_day: Option<Date>) -> bool {
        match (self, first_day) {
            (Self::Everything, _) => true,
            (Self::Since(_), None) => false,
            (Self::Since(held), Some(first)) => held <= first,
        }
    }
}

/// Which sent mail to learn from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LearnRange {
    /// All of it, as far back as the server holds it.
    Everything,
    /// Everything up to the end of last year, to leave out mail written with an assistant's
    /// help.
    UntilEndOfLastYear,
    /// The person's own range, both days included.
    Between {
        /// The first day.
        since: Date,
        /// The last day.
        until: Date,
    },
}

/// The ranges offered for one account.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LearnRanges {
    /// The presets, in the order they are offered, the range of the person's own last with the
    /// days it starts from: a year back to today.
    pub presets: Vec<LearnRange>,
    /// The latest day a range of the person's own may end on: today. It may start on any day.
    pub latest: Date,
}

impl LearnRanges {
    /// Whether `range` can be honoured: a preset offered, or a range of the person's own not
    /// ending after today nor before it starts.
    #[must_use]
    pub fn admits(&self, range: LearnRange) -> bool {
        match range {
            LearnRange::Between { since, until } => since <= until && until <= self.latest,
            preset => self.presets.contains(&preset),
        }
    }
}

/// The ranges offered on `today`. The one place a range is decided: a client draws whatever this
/// offers.
#[must_use]
pub fn learn_ranges(today: Date) -> LearnRanges {
    let a_year_back = today
        .replace_year(today.year() - 1)
        .unwrap_or_else(|_| today - time::Duration::days(365));
    LearnRanges {
        presets: vec![
            LearnRange::Everything,
            LearnRange::UntilEndOfLastYear,
            LearnRange::Between {
                since: a_year_back,
                until: today,
            },
        ],
        latest: today,
    }
}

impl LearnRange {
    /// The first and last second the range admits, in seconds since the Unix epoch, on `today`;
    /// an open end is `None`.
    #[must_use]
    pub fn bounds(self, today: Date) -> (Option<i64>, Option<i64>) {
        let start = |day: Date| day.with_time(Time::MIDNIGHT).assume_utc().unix_timestamp();
        let end = |day: Date| start(day) + 86_399;
        match self {
            Self::Everything => (None, None),
            Self::UntilEndOfLastYear => {
                let last = Date::from_calendar_date(today.year() - 1, Month::December, 31)
                    .unwrap_or(today);
                (None, Some(end(last)))
            }
            Self::Between { since, until } => (Some(start(since)), Some(end(until))),
        }
    }

    /// The first day of sent mail the range reaches; `None` when it reaches all of it.
    #[must_use]
    pub const fn first_day(self) -> Option<Date> {
        match self {
            Self::Everything | Self::UntilEndOfLastYear => None,
            Self::Between { since, .. } => Some(since),
        }
    }
}

/// Today, in UTC.
pub(crate) fn today() -> Date {
    OffsetDateTime::now_utc().date()
}

/// Whether an instant is inside `bounds`. An undated message is inside only an open range.
pub(crate) fn admits(bounds: (Option<i64>, Option<i64>), instant: Option<i64>) -> bool {
    match instant {
        Some(instant) => {
            bounds.0.is_none_or(|since| instant >= since)
                && bounds.1.is_none_or(|until| instant <= until)
        }
        None => bounds.0.is_none() && bounds.1.is_none(),
    }
}

#[cfg(test)]
mod tests {
    use mailcal_account::SyncDepth;
    use time::{Date, Month};

    use super::{LearnRange, SentReach, admits, learn_ranges};

    fn day(year: i32, month: Month, day: u8) -> Date {
        Date::from_calendar_date(year, month, day).unwrap()
    }

    /// The range step reaches before the sync window: every preset is offered whatever the device
    /// holds, and a range of one's own may start on any day.
    #[test]
    fn every_preset_is_offered_whatever_the_device_holds() {
        let today = day(2026, Month::November, 19);
        let ranges = learn_ranges(today);
        assert_eq!(
            ranges.presets,
            [
                LearnRange::Everything,
                LearnRange::UntilEndOfLastYear,
                LearnRange::Between {
                    since: day(2025, Month::November, 19),
                    until: today
                }
            ]
        );
        let between = |since, until| LearnRange::Between { since, until };
        assert!(ranges.admits(between(day(2015, Month::March, 1), today)));
        assert!(!ranges.admits(between(today, day(2026, Month::November, 20))));
        assert!(!ranges.admits(between(
            day(2026, Month::October, 2),
            day(2026, Month::October, 1)
        )));
    }

    #[test]
    fn a_range_of_ones_own_starts_a_year_back_even_on_a_leap_day() {
        let leap = day(2028, Month::February, 29);
        let ranges = learn_ranges(leap);
        let Some(LearnRange::Between { since, until }) = ranges.presets.last().copied() else {
            panic!("a range of one's own last");
        };
        assert!(since < leap);
        assert_eq!(until, leap);
        assert!(ranges.admits(LearnRange::Between { since, until }));
    }

    /// What the device holds covers a range only back to its first day; a range with none needs
    /// the whole history.
    #[test]
    fn what_the_device_holds_covers_a_range_back_to_its_first_day() {
        let today = day(2026, Month::November, 19);
        let held = SentReach::of(SyncDepth::default(), today);
        assert_eq!(held, SentReach::Since(day(2026, Month::August, 19)));
        assert!(held.covers(Some(day(2026, Month::September, 1))));
        assert!(held.covers(Some(day(2026, Month::August, 19))));
        assert!(!held.covers(Some(day(2026, Month::August, 18))));
        assert!(!held.covers(LearnRange::Everything.first_day()));
        assert!(!held.covers(LearnRange::UntilEndOfLastYear.first_day()));
        assert!(SentReach::Everything.covers(None));
    }

    #[test]
    fn the_last_day_of_a_range_is_kept_to_its_last_second() {
        let today = day(2026, Month::November, 19);
        let bounds = LearnRange::UntilEndOfLastYear.bounds(today);
        let new_years_eve = 1_767_225_599; // 2025-12-31T23:59:59Z
        assert_eq!(bounds, (None, Some(new_years_eve)));
        assert!(admits(bounds, Some(new_years_eve)));
        assert!(!admits(bounds, Some(new_years_eve + 1)));
        assert!(
            !admits(bounds, None),
            "an undated message is outside a bounded range"
        );
        assert!(admits(LearnRange::Everything.bounds(today), None));
    }
}
