//! What an editor's controls may open, what they rebuild, and what they must not quietly drop.
//!
//! The two with teeth are the parts no control models (a monthly rule pinned to the last day of
//! the month has to survive an edit that never touched it), and the answer that is not an answer:
//! a save that changed nothing about the repeat sends nothing about the repeat.

use engine_core::time::CalendarDate;
use mailcal_viewmodel::calendar::days::{date_at, from_civil};

use super::{RepeatDraft, recurrence_change_of, repeat_draft_of, rule_from_draft};
use crate::{
    recurrence_shape::{
        RecurrenceChange, RecurrenceDay, RecurrenceEnd, RecurrenceFrequency, RecurrenceWeekday,
        SimpleRecurrence,
    },
    repeat_summary::summarize_repeat,
};

/// Tuesday 25 August 2026, the start every rule below is read against.
fn start() -> CalendarDate {
    date_at(from_civil(2026, 8, 25))
}

fn rule(frequency: RecurrenceFrequency) -> SimpleRecurrence {
    SimpleRecurrence {
        frequency,
        interval: 1,
        days: Vec::new(),
        month_days: Vec::new(),
        months: Vec::new(),
        end: RecurrenceEnd::Never,
    }
}

fn on(day: RecurrenceWeekday, nth: Option<i32>) -> RecurrenceDay {
    RecurrenceDay { day, nth }
}

#[test]
fn a_weekly_rule_naming_no_weekday_opens_with_the_starts_ticked() {
    let draft = repeat_draft_of(&rule(RecurrenceFrequency::Weekly), start()).expect("editable");
    assert_eq!(draft.weekdays, vec![RecurrenceWeekday::Tuesday]);
}

#[test]
fn the_weekday_row_is_populated_for_every_frequency_so_switching_to_weekly_has_a_day() {
    let draft = repeat_draft_of(&rule(RecurrenceFrequency::Daily), start()).expect("editable");
    assert_eq!(draft.weekdays, vec![RecurrenceWeekday::Tuesday]);
}

#[test]
fn a_rule_the_core_cannot_state_is_not_offered_for_editing() {
    // A weekday's position on a weekly rule: no sentence states it, so no editor opens it.
    let mut weekly = rule(RecurrenceFrequency::Weekly);
    weekly.days = vec![on(RecurrenceWeekday::Monday, Some(2))];
    assert!(summarize_repeat(&weekly, start()).is_none());
    assert!(repeat_draft_of(&weekly, start()).is_none());
}

#[test]
fn a_month_days_rule_survives_an_edit_that_never_touched_it() {
    // "The last day of the month" is a rule no control here offers. Changing only what ends it
    // must not turn it into "the 31st".
    let mut monthly = rule(RecurrenceFrequency::Monthly);
    monthly.month_days = vec![-1];
    let mut draft = repeat_draft_of(&monthly, start()).expect("editable");

    draft.end = RecurrenceEnd::AfterCount { count: 10 };
    let rebuilt = rule_from_draft(&draft);

    assert_eq!(rebuilt.month_days, vec![-1]);
    assert_eq!(rebuilt.end, RecurrenceEnd::AfterCount { count: 10 });
}

#[test]
fn a_weekdays_position_survives_an_edit_that_never_touched_it() {
    let mut monthly = rule(RecurrenceFrequency::Monthly);
    monthly.days = vec![on(RecurrenceWeekday::Monday, Some(2))];
    let mut draft = repeat_draft_of(&monthly, start()).expect("editable");

    draft.interval = 3;
    let rebuilt = rule_from_draft(&draft);

    assert_eq!(rebuilt.days, vec![on(RecurrenceWeekday::Monday, Some(2))]);
    assert_eq!(rebuilt.interval, 3);
}

#[test]
fn changing_the_frequency_drops_the_parts_that_belonged_to_the_old_one() {
    // A day of the month means nothing in a week, so carrying it over would write a rule the
    // user did not ask for, and one the expander answers with nothing at all.
    let mut monthly = rule(RecurrenceFrequency::Monthly);
    monthly.month_days = vec![-1];
    let mut draft = repeat_draft_of(&monthly, start()).expect("editable");

    draft.frequency = RecurrenceFrequency::Weekly;
    let rebuilt = rule_from_draft(&draft);

    assert!(rebuilt.month_days.is_empty());
    assert_eq!(rebuilt.days, vec![on(RecurrenceWeekday::Tuesday, None)]);
}

#[test]
fn a_weekly_rule_is_rebuilt_from_the_row_in_week_order_without_repeats() {
    let mut draft = repeat_draft_of(&rule(RecurrenceFrequency::Weekly), start()).expect("editable");
    draft.weekdays = vec![
        RecurrenceWeekday::Friday,
        RecurrenceWeekday::Monday,
        RecurrenceWeekday::Friday,
    ];
    assert_eq!(
        rule_from_draft(&draft).days,
        vec![
            on(RecurrenceWeekday::Monday, None),
            on(RecurrenceWeekday::Friday, None),
        ]
    );
}

#[test]
fn a_save_that_changed_nothing_about_the_repeat_sends_nothing_about_the_repeat() {
    let mut monthly = rule(RecurrenceFrequency::Monthly);
    monthly.month_days = vec![15];
    let draft = repeat_draft_of(&monthly, start()).expect("editable");

    assert_eq!(recurrence_change_of(Some(&draft), true), None);
}

/// A weekly rule naming no weekday means the start's, and the row opens with that day ticked.
/// A save that touched nothing must still say nothing: writing the implicit day back out is a
/// change the user did not make.
#[test]
fn an_untouched_weekly_rule_that_named_no_weekday_sends_nothing_either() {
    let draft = repeat_draft_of(&rule(RecurrenceFrequency::Weekly), start()).expect("editable");
    assert_eq!(draft.weekdays, vec![RecurrenceWeekday::Tuesday]);
    assert_eq!(recurrence_change_of(Some(&draft), true), None);
}

#[test]
fn a_repeat_typed_and_typed_back_is_not_a_change() {
    let draft = repeat_draft_of(&rule(RecurrenceFrequency::Daily), start()).expect("editable");
    let mut edited = draft.clone();
    edited.interval = 4;
    assert!(recurrence_change_of(Some(&edited), true).is_some());

    edited.interval = 1;
    assert_eq!(recurrence_change_of(Some(&edited), true), None);
}

#[test]
fn choosing_does_not_repeat_clears_a_series_and_says_nothing_about_a_one_off() {
    assert_eq!(
        recurrence_change_of(None, true),
        Some(RecurrenceChange::Clear)
    );
    assert_eq!(recurrence_change_of(None, false), None);
}

#[test]
fn a_first_rule_on_an_event_that_had_none_is_a_set() {
    let draft = RepeatDraft {
        frequency: RecurrenceFrequency::Weekly,
        interval: 1,
        weekdays: vec![RecurrenceWeekday::Tuesday],
        end: RecurrenceEnd::Never,
        stored: None,
    };
    let change = recurrence_change_of(Some(&draft), false).expect("a rule was chosen");
    let RecurrenceChange::Set(rule) = change else {
        panic!("a first rule is a Set");
    };
    assert_eq!(rule.frequency, RecurrenceFrequency::Weekly);
    assert_eq!(rule.days, vec![on(RecurrenceWeekday::Tuesday, None)]);
    assert_eq!(rule.end, RecurrenceEnd::Never);
}
