//! The repeat rule as a client can show and edit it.
//!
//! A stored [`Recurrence`] is the full RFC 8984 model: several rules, exclusion rules,
//! `bySetPosition`, ISO week numbers, a non-Gregorian `rscale`. An editor built from four
//! presets and a weekday row cannot express most of that, so this module projects the rule
//! onto the subset an editor *can* express, and says so plainly when it does not fit.
//!
//! # Why the fit is decided by a round trip
//!
//! [`describe_recurrence`] builds the projection and then **rebuilds the engine rule from it**.
//! Anything that does not come back identical is [`EventRecurrence::Complex`]: shown, never edited.
//!
//! A whitelist of the fields we understand would decide the same question by hand, and would
//! be wrong the day the engine gains a field: a rule carrying it would look simple, the
//! editor would seed itself from a projection missing that field, and saving would write the
//! rule *back without it*. Silently, on the user's real series. The round trip cannot make
//! that mistake: a field this module does not carry is a field the rebuilt rule does not
//! match on, so the rule degrades to read-only instead of being quietly rewritten.
//!
//! Overrides are **not** consulted. A moved or cancelled occurrence is not part of the rule;
//! a series is no less editable for having one.

use std::num::{NonZeroI32, NonZeroU32};

use engine_api::{Frequency, NDay, Recurrence, RecurrenceBound, RecurrenceRule, Weekday};

use crate::event_detail::datetime_str;

/// How often an event repeats, in the terms an editor offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecurrenceFrequency {
    /// Every day.
    Daily,
    /// Every week.
    Weekly,
    /// Every month.
    Monthly,
    /// Every year.
    Yearly,
}

/// A weekday a rule names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecurrenceWeekday {
    /// Monday.
    Monday,
    /// Tuesday.
    Tuesday,
    /// Wednesday.
    Wednesday,
    /// Thursday.
    Thursday,
    /// Friday.
    Friday,
    /// Saturday.
    Saturday,
    /// Sunday.
    Sunday,
}

/// One weekday of a rule, optionally pinned to its nth occurrence in the period.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecurrenceDay {
    /// The weekday.
    pub day: RecurrenceWeekday,
    /// Which one within the period; `1` is the first, `-1` the last, `None` every one.
    pub nth: Option<i32>,
}

/// When a repeat stops.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecurrenceEnd {
    /// It does not.
    Never,
    /// On a wall clock in the event's own zone, inclusive (`YYYY-MM-DDTHH:MM:SS`).
    OnDate {
        /// The last wall clock an instance may start at.
        date: String,
    },
    /// After a fixed number of instances, counting the first.
    AfterCount {
        /// How many instances in total.
        count: u32,
    },
}

/// A repeat rule an editor can both show and change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimpleRecurrence {
    /// The base frequency.
    pub frequency: RecurrenceFrequency,
    /// How many periods between instances; never zero, and `1` for every period.
    pub interval: u32,
    /// The weekdays named, empty when the rule takes them from the start.
    pub days: Vec<RecurrenceDay>,
    /// The days of the month named, empty when the rule takes them from the start.
    /// Negative counts from the end of the month.
    pub month_days: Vec<i32>,
    /// The months named (1–12), empty when the rule takes them from the start.
    pub months: Vec<u32>,
    /// When it stops.
    pub end: RecurrenceEnd,
}

/// What an edit does to an event's repeat rule: give it one, or take its rule away.
///
/// Three states with `Option<RecurrenceChange>`: leaving recurrence out of an edit keeps the
/// series exactly as it was, which is not the same as turning a repeating event into a
/// single one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecurrenceChange {
    /// Replace the rule, or give a one-off event its first one.
    Set(SimpleRecurrence),
    /// Stop repeating: every occurrence but the first goes.
    Clear,
}

/// What an event's repeat rule is, as far as a client needs to know.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventRecurrence {
    /// A rule this app can describe in full and offer for editing.
    Simple(SimpleRecurrence),
    /// The event repeats on a rule richer than the editor models. A client says that it
    /// repeats and offers no edit; the rule is never rewritten from a partial picture.
    Complex,
}

/// Projects a stored recurrence onto the editable subset, or reports it as
/// [`EventRecurrence::Complex`].
///
/// Returns `None` when there is nothing to describe: no rule at all.
#[must_use]
pub fn describe_recurrence(recurrence: &Recurrence) -> Option<EventRecurrence> {
    // More than one rule, or a rule whose instances are subtracted, is a set an editor built
    // from one frequency cannot state.
    let [rule] = recurrence.rules.as_slice() else {
        return (!recurrence.rules.is_empty()).then_some(EventRecurrence::Complex);
    };
    if !recurrence.excluded_rules.is_empty() {
        return Some(EventRecurrence::Complex);
    }
    Some(match simplify(rule) {
        Some(simple) if recurrence_rule_of(&simple).as_ref() == Some(rule) => {
            EventRecurrence::Simple(simple)
        }
        _ => EventRecurrence::Complex,
    })
}

/// Projects one rule onto the editable subset, dropping whatever it cannot hold.
///
/// Lossy by design; [`describe_recurrence`] is what decides whether anything was lost.
fn simplify(rule: &RecurrenceRule) -> Option<SimpleRecurrence> {
    Some(SimpleRecurrence {
        frequency: match rule.frequency {
            Frequency::Daily => RecurrenceFrequency::Daily,
            Frequency::Weekly => RecurrenceFrequency::Weekly,
            Frequency::Monthly => RecurrenceFrequency::Monthly,
            Frequency::Yearly => RecurrenceFrequency::Yearly,
            // A repeat measured in hours or finer is not a calendar repeat any editor offers.
            Frequency::Hourly | Frequency::Minutely | Frequency::Secondly => return None,
        },
        interval: rule.interval.get(),
        days: rule
            .by_day
            .iter()
            .map(|nday| RecurrenceDay {
                day: from_weekday(nday.day),
                nth: nday.nth_of_period.map(NonZeroI32::get),
            })
            .collect(),
        month_days: rule.by_month_day.clone(),
        // A leap month ("5L", RFC 7529) has no plain number and belongs to an `rscale` the
        // editor does not offer; leaving it out is what makes the round trip reject the rule.
        months: rule
            .by_month
            .iter()
            .filter_map(|m| m.parse().ok())
            .collect(),
        end: match &rule.bound {
            RecurrenceBound::Unbounded => RecurrenceEnd::Never,
            RecurrenceBound::Count(count) => RecurrenceEnd::AfterCount { count: count.get() },
            RecurrenceBound::Until(until) => RecurrenceEnd::OnDate {
                date: datetime_str(*until),
            },
        },
    })
}

/// The most periods a repeat may skip.
///
/// Orders of magnitude past anything an editor offers: the most permissive calendar app allows
/// a few hundred. Long before that a repeat stops being one: past roughly the horizon, a rule
/// produces the event's own start and then nothing, for ever.
///
/// A weekly interval above 1,043,497 asks for a span of days too large to build. That once
/// **aborted the process** rather than failing, so writing one was worse than useless; the
/// expander now refuses it and the event is reported as one that cannot be shown. This bound is
/// still the right place to stop, because a rule nothing can draw should not be written at all;
/// but it is no longer the only thing standing between a large number and a dead app.
const MAX_INTERVAL: u32 = 1_000;

/// The most of one weekday an `nth` may count into a month.
///
/// No month holds a sixth Monday, so a rule naming one produces its own start and then nothing,
/// for ever.
const MAX_NTH: u32 = 5;

/// Why this rule would produce an event nobody could see, or `None` when it can be drawn.
///
/// A rule the expander refuses materializes **zero** occurrences, so the event is stored, is
/// invisible to every range read, and the grid draws it nowhere. It does not look wrong; it is
/// absent. A rule that expands but matches nothing after its own start is the same failure with
/// one block on screen: an event that says it repeats and never does. Either way the user is
/// better served by a write that fails than by one that succeeds into nothing.
///
/// Every clause here was **measured** against the engine's own expander rather than read off its
/// source, and `mailcal_app`'s `an_undrawable_rule_really_cannot_be_drawn` re-measures them: if
/// the engine grows to cover one of these, that test goes red and this list should lose a clause.
///
/// The reason is a **shape** (it names a rule part, never the user's meeting) so it is safe to
/// log. It is not user-facing copy: a client renders the refusal through
/// `CalendarWriteStatus::Failed` in its own words.
#[must_use]
pub fn undrawable_reason(rule: &SimpleRecurrence) -> Option<&'static str> {
    if rule.interval > MAX_INTERVAL {
        return Some("the interval is beyond what a calendar can draw");
    }
    if rule.days.iter().any(|day| day.nth.is_some()) {
        match rule.frequency {
            // "The fourth Monday" needs a period with four Mondays in it. A week has one, and a
            // day has none, so the engine refuses both outright.
            RecurrenceFrequency::Daily | RecurrenceFrequency::Weekly => {
                return Some("a weekday's position only counts inside a month or a year");
            }
            // A year has fifty-odd Mondays and the expander counts them per month, so a yearly
            // rule has to say which month it means.
            RecurrenceFrequency::Yearly if rule.months.is_empty() => {
                return Some("a yearly rule counting a weekday's position must name its month");
            }
            _ => {}
        }
        if rule
            .days
            .iter()
            .any(|day| day.nth.is_some_and(|nth| nth.unsigned_abs() > MAX_NTH))
        {
            return Some("no month holds that many of one weekday");
        }
    }
    if rule.months.iter().any(|month| !(1..=12).contains(month)) {
        return Some("a month is 1 to 12");
    }
    if rule
        .month_days
        .iter()
        .any(|day| *day == 0 || day.unsigned_abs() > 31)
    {
        return Some("a day of the month is 1 to 31, or -1 to -31 counting back");
    }
    None
}

/// Rebuilds the engine rule a [`SimpleRecurrence`] stands for.
///
/// `None` when the projection does not describe a rule at all: a zero interval, a zero
/// `nth`, a count of zero, or an end date that is not a wall clock.
#[must_use]
pub fn recurrence_rule_of(simple: &SimpleRecurrence) -> Option<RecurrenceRule> {
    let mut rule = RecurrenceRule::new(match simple.frequency {
        RecurrenceFrequency::Daily => Frequency::Daily,
        RecurrenceFrequency::Weekly => Frequency::Weekly,
        RecurrenceFrequency::Monthly => Frequency::Monthly,
        RecurrenceFrequency::Yearly => Frequency::Yearly,
    });
    rule.interval = NonZeroU32::new(simple.interval)?;
    rule.by_day = simple
        .days
        .iter()
        .map(|day| {
            let nth_of_period = match day.nth {
                None => None,
                Some(nth) => Some(NonZeroI32::new(nth)?),
            };
            Some(NDay {
                day: to_weekday(day.day),
                nth_of_period,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    rule.by_month_day.clone_from(&simple.month_days);
    rule.by_month = simple.months.iter().map(u32::to_string).collect();
    rule.bound = match &simple.end {
        RecurrenceEnd::Never => RecurrenceBound::Unbounded,
        RecurrenceEnd::AfterCount { count } => RecurrenceBound::Count(NonZeroU32::new(*count)?),
        RecurrenceEnd::OnDate { date } => RecurrenceBound::Until(date.parse().ok()?),
    };
    Some(rule)
}

/// The editor's weekday for the engine's.
fn from_weekday(day: Weekday) -> RecurrenceWeekday {
    match day {
        Weekday::Mo => RecurrenceWeekday::Monday,
        Weekday::Tu => RecurrenceWeekday::Tuesday,
        Weekday::We => RecurrenceWeekday::Wednesday,
        Weekday::Th => RecurrenceWeekday::Thursday,
        Weekday::Fr => RecurrenceWeekday::Friday,
        Weekday::Sa => RecurrenceWeekday::Saturday,
        Weekday::Su => RecurrenceWeekday::Sunday,
    }
}

/// The engine's weekday for the editor's.
fn to_weekday(day: RecurrenceWeekday) -> Weekday {
    match day {
        RecurrenceWeekday::Monday => Weekday::Mo,
        RecurrenceWeekday::Tuesday => Weekday::Tu,
        RecurrenceWeekday::Wednesday => Weekday::We,
        RecurrenceWeekday::Thursday => Weekday::Th,
        RecurrenceWeekday::Friday => Weekday::Fr,
        RecurrenceWeekday::Saturday => Weekday::Sa,
        RecurrenceWeekday::Sunday => Weekday::Su,
    }
}

#[cfg(test)]
#[path = "recurrence_shape_tests.rs"]
mod recurrence_shape_tests;
