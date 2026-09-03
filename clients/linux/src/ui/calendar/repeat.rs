//! The repeat rule as a sentence: "Every 2 weeks on Monday, Friday, until 3 Jun 2027".
//!
//! Wording only. The core decided which sentence the rule gets; it read the event's start for
//! every part the rule leaves out, put the weekdays in week order, and dropped the rules it cannot
//! state exactly: so this is a `match` over a closed set and a catalog lookup. Weekday and month
//! names come from the platform's own locale data, the way the grid's headings do (`date.rs`),
//! rather than from the catalog: they are the one part of a localised string we do not have to
//! translate ourselves.

use mailcal_bindings::{RecurrenceWeekday, RepeatRhythm, RepeatStop, RepeatSummary};

use super::date::{date_from_wall, month_full, short_date, weekday_full};
use crate::l10n;

/// The repeat summary shown on an event's detail.
///
/// `summary` is `None` for an event with no rule, and for one whose rule the core would not state
/// exactly; those get the bare *Repeats*, because approximating states a series the user does not
/// have and nothing on screen would tell them apart.
pub(super) fn sentence(summary: Option<&RepeatSummary>, is_recurring: bool) -> String {
    let Some(summary) = summary else {
        return if is_recurring {
            l10n::event_repeat_other().to_owned()
        } else {
            l10n::event_repeat_none().to_owned()
        };
    };
    let rule = rhythm(&summary.rhythm);
    match &summary.stop {
        RepeatStop::Never => rule,
        RepeatStop::OnDate { date } => l10n::event_repeat_sum_until(&rule, &end_date(date)),
        RepeatStop::AfterCount { count } => l10n::event_repeat_sum_times(&rule, i64::from(*count)),
    }
}

/// The rhythm alone, without what ends it.
fn rhythm(rhythm: &RepeatRhythm) -> String {
    match rhythm {
        RepeatRhythm::Daily { interval } => {
            if *interval == 1 {
                l10n::event_repeat_daily().to_owned()
            } else {
                l10n::event_repeat_sum_daily_n(i64::from(*interval))
            }
        }
        RepeatRhythm::Weekly { interval, days } => {
            let named = days
                .iter()
                .map(|day| weekday_full(iso_weekday(day)))
                .collect::<Vec<_>>()
                .join(", ");
            if *interval == 1 {
                l10n::event_repeat_sum_weekly(&named)
            } else {
                l10n::event_repeat_sum_weekly_n(i64::from(*interval), &named)
            }
        }
        RepeatRhythm::MonthlyOnDay { interval, day } => {
            let day = day.to_string();
            if *interval == 1 {
                l10n::event_repeat_sum_monthly_day(&day)
            } else {
                l10n::event_repeat_sum_monthly_day_n(i64::from(*interval), &day)
            }
        }
        RepeatRhythm::MonthlyOnLastDay { interval } => {
            if *interval == 1 {
                l10n::event_repeat_sum_monthly_last().to_owned()
            } else {
                l10n::event_repeat_sum_monthly_last_n(i64::from(*interval))
            }
        }
        RepeatRhythm::MonthlyOnWeekday { interval, nth, day } => {
            let at = position(*nth, day);
            if *interval == 1 {
                l10n::event_repeat_sum_monthly_nth(&at)
            } else {
                l10n::event_repeat_sum_monthly_nth_n(i64::from(*interval), &at)
            }
        }
        RepeatRhythm::YearlyOnDate {
            interval,
            month,
            day,
        } => {
            let day = day.to_string();
            let named = month_full(*month);
            if *interval == 1 {
                l10n::event_repeat_sum_yearly(&day, &named)
            } else {
                l10n::event_repeat_sum_yearly_n(i64::from(*interval), &day, &named)
            }
        }
        RepeatRhythm::YearlyOnWeekday {
            interval,
            nth,
            day,
            month,
        } => {
            let at = position(*nth, day);
            let named = month_full(*month);
            if *interval == 1 {
                l10n::event_repeat_sum_yearly_nth(&at, &named)
            } else {
                l10n::event_repeat_sum_yearly_nth_n(i64::from(*interval), &at, &named)
            }
        }
    }
}

/// "on the fourth Monday", "na quarta segunda-feira": the phrase both by-weekday sentences drop
/// into, **carrying its own article**.
///
/// The article belongs here rather than in the frame because in some languages it has to agree
/// with the weekday, and the weekday is not known until this point. So each position has two
/// wordings, and **which weekdays take the alternative one is stated in the catalog**
/// (`event_repeat_nth_alt_days`, ISO weekday numbers) rather than as a table of genders in here:
/// it is a fact about a language, and it belongs beside that language's words. A language where
/// the question does not arise leaves the set empty and ships the same wording twice.
fn position(nth: i32, day: &RecurrenceWeekday) -> String {
    let iso = iso_weekday(day);
    let weekday = weekday_full(iso);
    let alt = alt_weekdays(l10n::event_repeat_nth_alt_days()).contains(&iso);
    match (nth, alt) {
        (1, false) => l10n::event_repeat_nth_first(&weekday),
        (1, true) => l10n::event_repeat_nth_first_alt(&weekday),
        (2, false) => l10n::event_repeat_nth_second(&weekday),
        (2, true) => l10n::event_repeat_nth_second_alt(&weekday),
        (3, false) => l10n::event_repeat_nth_third(&weekday),
        (3, true) => l10n::event_repeat_nth_third_alt(&weekday),
        (4, false) => l10n::event_repeat_nth_fourth(&weekday),
        (4, true) => l10n::event_repeat_nth_fourth_alt(&weekday),
        (5, false) => l10n::event_repeat_nth_fifth(&weekday),
        (5, true) => l10n::event_repeat_nth_fifth_alt(&weekday),
        (_, false) => l10n::event_repeat_nth_last(&weekday),
        (_, true) => l10n::event_repeat_nth_last_alt(&weekday),
    }
}

/// The catalog's alternative-form weekdays, as ISO numbers. Empty for a language where the
/// ordinal does not inflect, which is why an unparseable entry is simply dropped: the two
/// wordings are the same string there, so nothing on screen can go wrong.
fn alt_weekdays(entry: &str) -> Vec<u8> {
    entry
        .split(',')
        .filter_map(|value| value.trim().parse().ok())
        .collect()
}

/// The core's weekday as its ISO number; Monday 1 through Sunday 7, which is what the catalog's
/// alternative-form sets are written in.
pub(super) const fn iso_weekday(day: &RecurrenceWeekday) -> u8 {
    match day {
        RecurrenceWeekday::Monday => 1,
        RecurrenceWeekday::Tuesday => 2,
        RecurrenceWeekday::Wednesday => 3,
        RecurrenceWeekday::Thursday => 4,
        RecurrenceWeekday::Friday => 5,
        RecurrenceWeekday::Saturday => 6,
        RecurrenceWeekday::Sunday => 7,
    }
}

/// The last date a repeat may start on, written the way the rest of the app writes a date. An
/// unparseable one is shown as it arrived rather than dropped; a wrong-looking date says more
/// than a missing clause.
fn end_date(iso: &str) -> String {
    date_from_wall(iso).map_or_else(|| iso.to_owned(), short_date)
}

#[cfg(test)]
#[path = "repeat_tests.rs"]
mod tests;
