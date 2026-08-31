//! Which sentence a repeat rule gets, and which rules get none.
//!
//! Every case here used to be a client's to get right, four times over. The two with teeth are
//! the empty list (which means "take it from the event's start", not "nothing named") and the
//! refusals, which are what keeps a summary from stating a series the user does not have.

use engine_core::time::CalendarDate;
use mailcal_viewmodel::calendar::days::{date_at, from_civil};

use super::{RepeatRhythm, RepeatStop, summarize_repeat};
use crate::recurrence_shape::{
    RecurrenceDay, RecurrenceEnd, RecurrenceFrequency, RecurrenceWeekday, SimpleRecurrence,
};

/// Tuesday 25 August 2026: the start every rule below is read against.
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
fn a_rule_that_names_no_weekday_takes_the_starts() {
    let summary = summarize_repeat(&rule(RecurrenceFrequency::Weekly), start()).expect("stateable");
    assert_eq!(
        summary.rhythm,
        RepeatRhythm::Weekly {
            interval: 1,
            days: vec![RecurrenceWeekday::Tuesday],
        }
    );
}

#[test]
fn named_weekdays_are_listed_in_week_order_whatever_order_they_arrived_in() {
    let mut weekly = rule(RecurrenceFrequency::Weekly);
    weekly.interval = 2;
    weekly.days = vec![
        on(RecurrenceWeekday::Friday, None),
        on(RecurrenceWeekday::Monday, None),
    ];
    let summary = summarize_repeat(&weekly, start()).expect("stateable");
    assert_eq!(
        summary.rhythm,
        RepeatRhythm::Weekly {
            interval: 2,
            days: vec![RecurrenceWeekday::Monday, RecurrenceWeekday::Friday],
        }
    );
}

#[test]
fn a_monthly_rule_that_names_nothing_repeats_on_the_starts_day_of_the_month() {
    let summary =
        summarize_repeat(&rule(RecurrenceFrequency::Monthly), start()).expect("stateable");
    assert_eq!(
        summary.rhythm,
        RepeatRhythm::MonthlyOnDay {
            interval: 1,
            day: 25,
        }
    );
}

#[test]
fn a_monthly_rule_can_count_a_weekdays_position_forwards_and_from_the_end() {
    let mut fourth = rule(RecurrenceFrequency::Monthly);
    fourth.days = vec![on(RecurrenceWeekday::Monday, Some(4))];
    assert_eq!(
        summarize_repeat(&fourth, start())
            .expect("stateable")
            .rhythm,
        RepeatRhythm::MonthlyOnWeekday {
            interval: 1,
            nth: 4,
            day: RecurrenceWeekday::Monday,
        }
    );

    let mut last = rule(RecurrenceFrequency::Monthly);
    last.days = vec![on(RecurrenceWeekday::Friday, Some(-1))];
    assert_eq!(
        summarize_repeat(&last, start()).expect("stateable").rhythm,
        RepeatRhythm::MonthlyOnWeekday {
            interval: 1,
            nth: -1,
            day: RecurrenceWeekday::Friday,
        }
    );
}

#[test]
fn the_last_day_of_the_month_is_its_own_shape_not_a_day_number() {
    let mut monthly = rule(RecurrenceFrequency::Monthly);
    monthly.month_days = vec![-1];
    assert_eq!(
        summarize_repeat(&monthly, start())
            .expect("stateable")
            .rhythm,
        RepeatRhythm::MonthlyOnLastDay { interval: 1 }
    );
}

#[test]
fn a_yearly_rule_that_names_nothing_repeats_on_the_starts_date() {
    let summary = summarize_repeat(&rule(RecurrenceFrequency::Yearly), start()).expect("stateable");
    assert_eq!(
        summary.rhythm,
        RepeatRhythm::YearlyOnDate {
            interval: 1,
            month: 8,
            day: 25,
        }
    );
}

#[test]
fn a_yearly_rule_can_count_a_weekdays_position_inside_a_named_month() {
    let mut yearly = rule(RecurrenceFrequency::Yearly);
    yearly.days = vec![on(RecurrenceWeekday::Thursday, Some(4))];
    yearly.months = vec![11];
    assert_eq!(
        summarize_repeat(&yearly, start())
            .expect("stateable")
            .rhythm,
        RepeatRhythm::YearlyOnWeekday {
            interval: 1,
            nth: 4,
            day: RecurrenceWeekday::Thursday,
            month: 11,
        }
    );
}

#[test]
fn an_end_is_carried_as_a_date_and_one_we_cannot_read_takes_the_rule_with_it() {
    let mut until = rule(RecurrenceFrequency::Daily);
    until.end = RecurrenceEnd::OnDate {
        date: "2027-06-03T09:00:00".to_owned(),
    };
    assert_eq!(
        summarize_repeat(&until, start()).expect("stateable").stop,
        RepeatStop::OnDate {
            date: "2027-06-03".to_owned(),
        }
    );

    let mut times = rule(RecurrenceFrequency::Daily);
    times.end = RecurrenceEnd::AfterCount { count: 12 };
    assert_eq!(
        summarize_repeat(&times, start()).expect("stateable").stop,
        RepeatStop::AfterCount { count: 12 }
    );

    // Dropping an end we cannot read would describe a series that never stops.
    let mut broken = rule(RecurrenceFrequency::Daily);
    broken.end = RecurrenceEnd::OnDate {
        date: "whenever".to_owned(),
    };
    assert!(summarize_repeat(&broken, start()).is_none());
}

#[test]
fn a_rule_mixing_parts_this_cannot_state_exactly_is_refused_rather_than_approximated() {
    // Each of these would be summarised wrongly by one of the shapes above, and nothing on
    // screen would tell the user which series they actually have.
    let mut daily_with_weekdays = rule(RecurrenceFrequency::Daily);
    daily_with_weekdays.days = vec![on(RecurrenceWeekday::Monday, None)];
    assert!(summarize_repeat(&daily_with_weekdays, start()).is_none());

    let mut two_month_days = rule(RecurrenceFrequency::Monthly);
    two_month_days.month_days = vec![1, 15];
    assert!(summarize_repeat(&two_month_days, start()).is_none());

    let mut sixth_monday = rule(RecurrenceFrequency::Monthly);
    sixth_monday.days = vec![on(RecurrenceWeekday::Monday, Some(6))];
    assert!(summarize_repeat(&sixth_monday, start()).is_none());

    let mut bad_month = rule(RecurrenceFrequency::Yearly);
    bad_month.months = vec![13];
    assert!(summarize_repeat(&bad_month, start()).is_none());

    // A yearly position with no month names nothing a year has one of.
    let mut yearly_position_no_month = rule(RecurrenceFrequency::Yearly);
    yearly_position_no_month.days = vec![on(RecurrenceWeekday::Thursday, Some(4))];
    assert!(summarize_repeat(&yearly_position_no_month, start()).is_none());
}
