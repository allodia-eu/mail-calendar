//! Which sent mail a style is learned from (`docs/ai.md` rule 11, `ChooseRange`): the presets the
//! core offers, decided from how far back the device holds the account's mail, so a client
//! converts no dates and offers nothing that cannot be honoured.
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
}

/// Which sent mail to learn from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LearnRange {
    /// Everything the device holds.
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
    /// The presets, in the order they are offered.
    pub presets: Vec<LearnRange>,
    /// The earliest day a range of the person's own may start on; `None` when the device holds
    /// everything.
    pub earliest: Option<Date>,
    /// The latest day it may end on: today.
    pub latest: Date,
}

impl LearnRanges {
    /// Whether `range` can be honoured: a preset offered, or a range of the person's own within
    /// the bounds and not ending before it starts.
    #[must_use]
    pub fn admits(&self, range: LearnRange) -> bool {
        match range {
            LearnRange::Between { since, until } => {
                since <= until
                    && until <= self.latest
                    && self.earliest.is_none_or(|earliest| since >= earliest)
            }
            preset => self.presets.contains(&preset),
        }
    }
}

/// The ranges that can be honoured for mail the device holds back to `reach`, on `today`.
/// "Up to the end of last year" is offered only when the device holds mail from before this year.
///
/// The one place a deeper reach than the sync window would be added: a client draws whatever this
/// offers.
#[must_use]
pub fn learn_ranges(reach: SentReach, today: Date) -> LearnRanges {
    let start_of_year = Date::from_calendar_date(today.year(), Month::January, 1).unwrap_or(today);
    let earliest = match reach {
        SentReach::Everything => None,
        SentReach::Since(day) => Some(day),
    };
    let mut presets = vec![LearnRange::Everything];
    if earliest.is_none_or(|earliest| earliest < start_of_year) {
        presets.push(LearnRange::UntilEndOfLastYear);
    }
    LearnRanges {
        presets,
        earliest,
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

    #[test]
    fn three_months_back_offers_no_mail_from_before_this_year() {
        let today = day(2026, Month::November, 19);
        let reach = SentReach::of(SyncDepth::default(), today);
        assert_eq!(reach, SentReach::Since(day(2026, Month::August, 19)));
        let ranges = learn_ranges(reach, today);
        assert_eq!(ranges.presets, [LearnRange::Everything]);
        assert_eq!(ranges.earliest, Some(day(2026, Month::August, 19)));
    }

    #[test]
    fn a_reach_into_last_year_offers_the_end_of_last_year() {
        let today = day(2026, Month::March, 2);
        let ranges = learn_ranges(SentReach::of(SyncDepth::default(), today), today);
        assert_eq!(
            ranges.presets,
            [LearnRange::Everything, LearnRange::UntilEndOfLastYear]
        );
        let all = learn_ranges(SentReach::Everything, today);
        assert!(all.presets.contains(&LearnRange::UntilEndOfLastYear));
        assert_eq!(all.earliest, None);
    }

    #[test]
    fn a_range_of_ones_own_stays_within_what_the_device_holds() {
        let today = day(2026, Month::November, 19);
        let ranges = learn_ranges(SentReach::Since(day(2026, Month::August, 19)), today);
        let between = |since, until| LearnRange::Between { since, until };
        assert!(ranges.admits(between(
            day(2026, Month::September, 1),
            day(2026, Month::October, 1)
        )));
        assert!(!ranges.admits(between(day(2026, Month::July, 1), today)));
        assert!(!ranges.admits(between(today, day(2026, Month::November, 20))));
        assert!(!ranges.admits(between(
            day(2026, Month::October, 2),
            day(2026, Month::October, 1)
        )));
        assert!(!ranges.admits(LearnRange::UntilEndOfLastYear));
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
