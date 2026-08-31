//! Which sentence a repeat rule gets.
//!
//! Nothing here matches a literal English string: the suite runs in whatever language the machine
//! is in, so every expectation is built from the same catalog frame the code reaches for, and the
//! weekday and month names from the same platform call. What is actually being pinned is the
//! *choice*; which frame, and what goes in it; because that is what silently produces a
//! plausible sentence about a series the user does not have.

use mailcal_bindings::{RecurrenceWeekday, RepeatRhythm, RepeatStop, RepeatSummary};

use super::{
    super::date::{month_full, weekday_full},
    sentence,
};
use crate::l10n;

fn summary(rhythm: RepeatRhythm, stop: RepeatStop) -> RepeatSummary {
    RepeatSummary { rhythm, stop }
}

fn weekly(interval: u32, days: Vec<RecurrenceWeekday>) -> RepeatRhythm {
    RepeatRhythm::Weekly { interval, days }
}

#[test]
fn an_event_with_no_rule_says_so() {
    assert_eq!(sentence(None, false), l10n::event_repeat_none());
}

#[test]
fn a_rule_the_core_would_not_state_exactly_says_only_that_it_repeats() {
    // Approximating states a series the user does not have, and nothing on screen would tell the
    // two apart; so the summary is withheld rather than guessed at.
    assert_eq!(sentence(None, true), l10n::event_repeat_other());
}

#[test]
fn a_weekly_rule_names_its_weekdays() {
    let rule = summary(
        weekly(
            1,
            vec![RecurrenceWeekday::Monday, RecurrenceWeekday::Friday],
        ),
        RepeatStop::Never,
    );
    let named = format!("{}, {}", weekday_full(1), weekday_full(5));
    assert_eq!(
        sentence(Some(&rule), true),
        l10n::event_repeat_sum_weekly(&named)
    );
}

#[test]
fn a_rule_that_skips_periods_does_not_claim_the_every_period_frame() {
    // The reason the core sends a structure rather than a frequency word at all: "Weekly" for a
    // fortnightly meeting is a plain untruth, and the old one-word summary could not tell them
    // apart.
    let days = vec![RecurrenceWeekday::Tuesday];
    let every = summary(weekly(1, days.clone()), RepeatStop::Never);
    let other = summary(weekly(2, days), RepeatStop::Never);
    let named = weekday_full(2);
    assert_eq!(
        sentence(Some(&other), true),
        l10n::event_repeat_sum_weekly_n(2, &named)
    );
    assert_ne!(sentence(Some(&every), true), sentence(Some(&other), true));
}

#[test]
fn a_daily_rule_reads_as_the_bare_word_only_when_it_skips_nothing() {
    let every = summary(RepeatRhythm::Daily { interval: 1 }, RepeatStop::Never);
    let third = summary(RepeatRhythm::Daily { interval: 3 }, RepeatStop::Never);
    assert_eq!(sentence(Some(&every), true), l10n::event_repeat_daily());
    assert_eq!(
        sentence(Some(&third), true),
        l10n::event_repeat_sum_daily_n(3)
    );
}

#[test]
fn a_weekdays_position_in_the_month_carries_its_own_article() {
    let rule = summary(
        RepeatRhythm::MonthlyOnWeekday {
            interval: 1,
            nth: 3,
            day: RecurrenceWeekday::Wednesday,
        },
        RepeatStop::Never,
    );
    let at = l10n::event_repeat_nth_third(&weekday_full(3));
    assert_eq!(
        sentence(Some(&rule), true),
        l10n::event_repeat_sum_monthly_nth(&at)
    );
}

#[test]
fn a_negative_position_is_the_last_one_not_the_nth() {
    // -1 is the core's "last", and every other negative is the same request. Falling through to
    // an ordinal here would name a week of the month that may not exist.
    let rule = summary(
        RepeatRhythm::MonthlyOnWeekday {
            interval: 1,
            nth: -1,
            day: RecurrenceWeekday::Sunday,
        },
        RepeatStop::Never,
    );
    let at = l10n::event_repeat_nth_last(&weekday_full(7));
    assert_eq!(
        sentence(Some(&rule), true),
        l10n::event_repeat_sum_monthly_nth(&at)
    );
}

#[test]
fn a_yearly_rule_names_its_month() {
    let rule = summary(
        RepeatRhythm::YearlyOnDate {
            interval: 1,
            month: 6,
            day: 3,
        },
        RepeatStop::Never,
    );
    assert_eq!(
        sentence(Some(&rule), true),
        l10n::event_repeat_sum_yearly("3", &month_full(6))
    );
}

#[test]
fn what_ends_the_rule_wraps_the_rule_rather_than_replacing_it() {
    let rhythm = RepeatRhythm::Daily { interval: 1 };
    let counted = summary(rhythm.clone(), RepeatStop::AfterCount { count: 10 });
    let plain = l10n::event_repeat_daily();
    assert_eq!(
        sentence(Some(&counted), true),
        l10n::event_repeat_sum_times(plain, 10)
    );
    let dated = summary(
        rhythm,
        RepeatStop::OnDate {
            date: "2027-06-03".to_owned(),
        },
    );
    let said = sentence(Some(&dated), true);
    assert!(
        said.starts_with(plain),
        "the rule survives its end clause: {said}"
    );
    assert_ne!(said, plain);
}

#[test]
fn an_end_date_the_client_cannot_parse_is_shown_as_it_arrived() {
    // A wrong-looking date says more than a missing clause; and silently dropping the bound
    // would state a series that never stops.
    let rule = summary(
        RepeatRhythm::Daily { interval: 1 },
        RepeatStop::OnDate {
            date: "not-a-date".to_owned(),
        },
    );
    assert_eq!(
        sentence(Some(&rule), true),
        l10n::event_repeat_sum_until(l10n::event_repeat_daily(), "not-a-date")
    );
}
