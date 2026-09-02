//! What the repeat controls offer, and the arithmetic behind the weekday row.
//!
//! The rebuild itself is the core's and is tested there. What is this client's is which choice a
//! draft reads as, the sentence a spinner shows for it, and the row that must never empty: a
//! weekly rule naming no day is one the core refuses, which reads in the app as a save that
//! quietly did nothing.

use mailcal_bindings::{RecurrenceEnd, RecurrenceFrequency, RecurrenceWeekday};

use super::{RepeatChoice, RepeatEndChoice, WEEK, toggled, weekday_of};

#[test]
fn every_frequency_round_trips_through_the_picker() {
    for choice in RepeatChoice::ALL {
        assert_eq!(RepeatChoice::of(choice.frequency().as_ref()), choice);
    }
}

#[test]
fn the_absence_of_a_rule_is_a_choice_of_its_own() {
    assert_eq!(RepeatChoice::of(None), RepeatChoice::Never);
    assert!(RepeatChoice::Never.frequency().is_none());
}

#[test]
fn every_end_round_trips_through_its_picker() {
    assert_eq!(
        RepeatEndChoice::of(&RecurrenceEnd::Never),
        RepeatEndChoice::Never
    );
    assert_eq!(
        RepeatEndChoice::of(&RecurrenceEnd::OnDate {
            date: "2027-03-01T00:00:00".to_owned()
        }),
        RepeatEndChoice::OnDate
    );
    assert_eq!(
        RepeatEndChoice::of(&RecurrenceEnd::AfterCount { count: 10 }),
        RepeatEndChoice::AfterCount
    );
}

/// The picker directly above the spinner already shows the frequency word, so the spinner never
/// repeats it; it states the period it sets.
#[test]
fn the_interval_spinner_never_repeats_the_frequency_word() {
    for choice in [
        RepeatChoice::Daily,
        RepeatChoice::Weekly,
        RepeatChoice::Monthly,
        RepeatChoice::Yearly,
    ] {
        assert_ne!(choice.interval_label(1), choice.label());
        assert_ne!(choice.interval_label(3), choice.label());
        assert_ne!(choice.interval_label(1), choice.interval_label(3));
    }
}

#[test]
fn the_weekday_row_never_empties() {
    let one = vec![RecurrenceWeekday::Wednesday];
    assert_eq!(toggled(&one, RecurrenceWeekday::Wednesday), one);
}

#[test]
fn ticking_a_weekday_returns_the_row_in_week_order() {
    assert_eq!(
        toggled(&[RecurrenceWeekday::Friday], RecurrenceWeekday::Monday),
        vec![RecurrenceWeekday::Monday, RecurrenceWeekday::Friday]
    );
}

#[test]
fn unticking_one_of_several_leaves_the_rest() {
    let both = vec![RecurrenceWeekday::Monday, RecurrenceWeekday::Friday];
    assert_eq!(
        toggled(&both, RecurrenceWeekday::Monday),
        vec![RecurrenceWeekday::Friday]
    );
}

/// `WEEK` is Monday-first because that is the order the core counts weekdays in; indexing it from
/// a `time` weekday counted any other way renames every day and still draws a plausible row.
#[test]
fn a_rule_first_chosen_falls_on_the_events_own_weekday() {
    // 26 August 2026 is a Wednesday; 30 August is a Sunday.
    assert_eq!(
        weekday_of("2026-08-26T09:00:00"),
        RecurrenceWeekday::Wednesday
    );
    assert_eq!(weekday_of("2026-08-30"), RecurrenceWeekday::Sunday);
    assert_eq!(WEEK[0], RecurrenceWeekday::Monday);
    assert_eq!(WEEK[6], RecurrenceWeekday::Sunday);
}

#[test]
fn an_unreadable_start_still_yields_a_weekday_rather_than_nothing() {
    assert_eq!(weekday_of("not a date"), RecurrenceWeekday::Monday);
}

#[test]
fn a_daily_frequency_is_the_one_the_picker_reports() {
    assert_eq!(
        RepeatChoice::of(Some(&RecurrenceFrequency::Daily)),
        RepeatChoice::Daily
    );
}
