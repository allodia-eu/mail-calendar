//! What the editor may touch, and what it may only look at.
//!
//! The load-bearing assertion in this file is [`a_rule_the_projection_cannot_hold_is_complex`]:
//! every part the projection drops has to come back as read-only. A rule that reads as
//! `Simple` while missing a part is the one failure that loses a user's real series.

use std::num::{NonZeroI32, NonZeroU32};

use engine_api::{
    Frequency, LocalDateTime, NDay, Recurrence, RecurrenceBound, RecurrenceRule, Weekday,
};

use crate::recurrence_shape::{
    EventRecurrence, RecurrenceDay, RecurrenceEnd, RecurrenceFrequency, RecurrenceWeekday,
    SimpleRecurrence, describe_recurrence, recurrence_rule_of, undrawable_reason,
};

/// A weekly rule with nothing else set.
fn weekly() -> RecurrenceRule {
    RecurrenceRule::new(Frequency::Weekly)
}

/// The `SimpleRecurrence` a bare weekly rule projects to.
fn simple_weekly() -> SimpleRecurrence {
    SimpleRecurrence {
        frequency: RecurrenceFrequency::Weekly,
        interval: 1,
        days: Vec::new(),
        month_days: Vec::new(),
        months: Vec::new(),
        end: RecurrenceEnd::Never,
    }
}

/// Unwraps a recurrence expected to be simple.
fn simple_of(rule: RecurrenceRule) -> SimpleRecurrence {
    match describe_recurrence(&Recurrence::from_rule(rule)) {
        Some(EventRecurrence::Simple(simple)) => simple,
        other => panic!("expected a simple rule, got {other:?}"),
    }
}

#[test]
fn a_plain_weekly_rule_is_simple() {
    assert_eq!(simple_of(weekly()), simple_weekly());
}

#[test]
fn an_interval_is_carried_rather_than_flattened() {
    // The old frequency token said "WEEKLY" for this, so a fortnightly meeting claimed to be a
    // weekly one. The interval is the whole reason the rule is structured.
    let mut rule = weekly();
    rule.interval = NonZeroU32::new(2).unwrap();

    assert_eq!(simple_of(rule).interval, 2);
}

#[test]
fn the_weekdays_a_rule_names_are_carried() {
    let mut rule = weekly();
    rule.by_day = vec![
        NDay {
            day: Weekday::Mo,
            nth_of_period: None,
        },
        NDay {
            day: Weekday::Th,
            nth_of_period: None,
        },
    ];

    assert_eq!(
        simple_of(rule).days,
        vec![
            RecurrenceDay {
                day: RecurrenceWeekday::Monday,
                nth: None
            },
            RecurrenceDay {
                day: RecurrenceWeekday::Thursday,
                nth: None
            },
        ]
    );
}

#[test]
fn the_fourth_monday_of_the_month_keeps_its_position() {
    // One of the presets an editor builds from the event's own start, so the nth has to
    // survive the projection rather than collapsing to "every Monday".
    let mut rule = RecurrenceRule::new(Frequency::Monthly);
    rule.by_day = vec![NDay {
        day: Weekday::Mo,
        nth_of_period: Some(NonZeroI32::new(4).unwrap()),
    }];

    assert_eq!(
        simple_of(rule).days,
        vec![RecurrenceDay {
            day: RecurrenceWeekday::Monday,
            nth: Some(4)
        }]
    );
}

#[test]
fn an_end_date_and_an_end_count_are_each_carried() {
    let mut until = weekly();
    until.bound = RecurrenceBound::Until(LocalDateTime::new(2026, 12, 3, 9, 0, 0).unwrap());
    let mut count = weekly();
    count.bound = RecurrenceBound::Count(NonZeroU32::new(12).unwrap());

    assert_eq!(
        simple_of(until).end,
        RecurrenceEnd::OnDate {
            date: "2026-12-03T09:00:00".to_owned()
        }
    );
    assert_eq!(
        simple_of(count).end,
        RecurrenceEnd::AfterCount { count: 12 }
    );
}

#[test]
fn a_monthly_or_yearly_rule_that_names_its_days_stays_editable() {
    // Both providers emit these for perfectly ordinary rules ("monthly on the 24th", "yearly
    // on 24 August"). Dropping them from the projection would make the two commonest calendar
    // repeats read-only.
    let mut monthly = RecurrenceRule::new(Frequency::Monthly);
    monthly.by_month_day = vec![24];
    let mut yearly = RecurrenceRule::new(Frequency::Yearly);
    yearly.by_month = vec!["8".to_owned()];
    yearly.by_month_day = vec![24];

    assert_eq!(simple_of(monthly).month_days, vec![24]);
    let yearly = simple_of(yearly);
    assert_eq!(yearly.months, vec![8]);
    assert_eq!(yearly.month_days, vec![24]);
}

#[test]
fn a_rule_the_projection_cannot_hold_is_complex() {
    // Each of these sets a part the projection does not carry. None may read as `Simple`: the
    // editor seeds itself from the projection, so a rule that looks simple while missing a
    // part is a rule the next save silently rewrites without it.
    let mut by_set_position = weekly();
    by_set_position.by_set_position = vec![-1];
    let mut by_week_no = weekly();
    by_week_no.by_week_no = vec![3];
    let mut by_year_day = weekly();
    by_year_day.by_year_day = vec![100];
    let mut by_hour = weekly();
    by_hour.by_hour = vec![9];
    let mut by_minute = weekly();
    by_minute.by_minute = vec![30];
    let mut by_second = weekly();
    by_second.by_second = vec![15];
    let mut rscale = weekly();
    rscale.rscale = Some("hebrew".to_owned());
    let mut week_start = weekly();
    week_start.first_day_of_week = Weekday::Su;
    let mut leap_month = RecurrenceRule::new(Frequency::Yearly);
    leap_month.by_month = vec!["5L".to_owned()];
    let mut sub_daily = RecurrenceRule::new(Frequency::Hourly);
    sub_daily.interval = NonZeroU32::new(6).unwrap();

    for (part, rule) in [
        ("by_set_position", by_set_position),
        ("by_week_no", by_week_no),
        ("by_year_day", by_year_day),
        ("by_hour", by_hour),
        ("by_minute", by_minute),
        ("by_second", by_second),
        ("rscale", rscale),
        ("first_day_of_week", week_start),
        ("a leap month", leap_month),
        ("an hourly frequency", sub_daily),
    ] {
        assert_eq!(
            describe_recurrence(&Recurrence::from_rule(rule)),
            Some(EventRecurrence::Complex),
            "{part} is not carried, so its rule must be read-only"
        );
    }
}

#[test]
fn a_set_of_several_rules_is_complex() {
    // One frequency cannot state a union of two, and an editor that saved from this picture
    // would drop the second rule entirely.
    let two = Recurrence {
        rules: vec![weekly(), RecurrenceRule::new(Frequency::Monthly)],
        excluded_rules: Vec::new(),
        overrides: std::collections::BTreeMap::new(),
    };
    let subtracted = Recurrence {
        rules: vec![weekly()],
        excluded_rules: vec![RecurrenceRule::new(Frequency::Monthly)],
        overrides: std::collections::BTreeMap::new(),
    };

    assert_eq!(
        describe_recurrence(&two),
        Some(EventRecurrence::Complex),
        "two rules cannot be stated as one"
    );
    assert_eq!(
        describe_recurrence(&subtracted),
        Some(EventRecurrence::Complex),
        "a subtracted rule cannot be stated at all"
    );
}

#[test]
fn a_series_with_a_moved_occurrence_is_still_editable() {
    // An override is not part of the rule. Treating one as "too complex to edit" would make a
    // series read-only the moment somebody dragged a single instance of it.
    let mut recurrence = Recurrence::from_rule(weekly());
    recurrence.overrides.insert(
        LocalDateTime::new(2026, 1, 12, 9, 0, 0).unwrap(),
        engine_api::RecurrenceOverride::Excluded,
    );

    assert_eq!(
        describe_recurrence(&recurrence),
        Some(EventRecurrence::Simple(simple_weekly()))
    );
}

#[test]
fn no_rule_at_all_describes_nothing() {
    let none = Recurrence {
        rules: Vec::new(),
        excluded_rules: Vec::new(),
        overrides: std::collections::BTreeMap::new(),
    };

    assert_eq!(describe_recurrence(&none), None);
}

#[test]
fn every_simple_rule_rebuilds_the_rule_it_came_from() {
    // The round trip is the guard itself, so state it directly rather than trusting that the
    // cases above happened to exercise it.
    let mut rule = RecurrenceRule::new(Frequency::Monthly);
    rule.interval = NonZeroU32::new(3).unwrap();
    rule.by_day = vec![NDay {
        day: Weekday::Fr,
        nth_of_period: Some(NonZeroI32::new(-1).unwrap()),
    }];
    rule.by_month_day = vec![13];
    rule.bound = RecurrenceBound::Count(NonZeroU32::new(6).unwrap());

    let simple = simple_of(rule.clone());

    assert_eq!(recurrence_rule_of(&simple), Some(rule));
}

#[test]
fn a_projection_that_describes_no_rule_rebuilds_nothing() {
    // Reachable from a client, not from `describe_recurrence`: these are the values an editor
    // can put on the wire. Each must fail to rebuild rather than be coerced into a rule.
    for (what, simple) in [
        (
            "a zero interval",
            SimpleRecurrence {
                interval: 0,
                ..simple_weekly()
            },
        ),
        (
            "a zero nth",
            SimpleRecurrence {
                days: vec![RecurrenceDay {
                    day: RecurrenceWeekday::Monday,
                    nth: Some(0),
                }],
                ..simple_weekly()
            },
        ),
        (
            "a count of zero",
            SimpleRecurrence {
                end: RecurrenceEnd::AfterCount { count: 0 },
                ..simple_weekly()
            },
        ),
        (
            "an end date that is not a wall clock",
            SimpleRecurrence {
                end: RecurrenceEnd::OnDate {
                    date: "next Tuesday".to_owned(),
                },
                ..simple_weekly()
            },
        ),
    ] {
        assert_eq!(
            recurrence_rule_of(&simple),
            None,
            "{what} describes no rule"
        );
    }
}

/// A rule an editor would plausibly build, before the case under test spoils it.
fn drawable() -> SimpleRecurrence {
    SimpleRecurrence {
        frequency: RecurrenceFrequency::Monthly,
        interval: 1,
        days: vec![RecurrenceDay {
            day: RecurrenceWeekday::Monday,
            nth: Some(4),
        }],
        month_days: Vec::new(),
        months: Vec::new(),
        end: RecurrenceEnd::Never,
    }
}

/// The same rule with its weekdays replaced.
fn on_days(frequency: RecurrenceFrequency, nth: Option<i32>) -> SimpleRecurrence {
    SimpleRecurrence {
        frequency,
        days: vec![RecurrenceDay {
            day: RecurrenceWeekday::Monday,
            nth,
        }],
        ..drawable()
    }
}

#[test]
fn the_rules_an_editor_builds_are_drawable() {
    // The presets an editor builds from an event's own start. If any of these were refused the
    // guard would be worse than none at all.
    let yearly = SimpleRecurrence {
        frequency: RecurrenceFrequency::Yearly,
        days: Vec::new(),
        month_days: vec![24],
        months: vec![8],
        ..drawable()
    };
    let fortnightly = SimpleRecurrence {
        frequency: RecurrenceFrequency::Weekly,
        interval: 2,
        days: vec![RecurrenceDay {
            day: RecurrenceWeekday::Monday,
            nth: None,
        }],
        ..drawable()
    };
    let last_friday = SimpleRecurrence {
        days: vec![RecurrenceDay {
            day: RecurrenceWeekday::Friday,
            nth: Some(-1),
        }],
        ..drawable()
    };

    for (what, rule) in [
        ("monthly on the fourth Monday", drawable()),
        ("yearly on 24 August", yearly),
        ("every second week on Monday", fortnightly),
        ("monthly on the last Friday", last_friday),
    ] {
        assert_eq!(undrawable_reason(&rule), None, "{what} must stay writable");
    }
}

#[test]
fn a_rule_the_grid_could_not_draw_is_refused() {
    // Every case here was measured against the engine's expander: the first three materialize
    // **zero** occurrences, so the event is stored and drawn nowhere; the rest expand and then
    // match nothing after the event's own start, so it says it repeats and never does.
    // `mailcal_app`'s `an_undrawable_rule_really_cannot_be_drawn` re-measures them.
    let yearly_nth_no_month = SimpleRecurrence {
        months: Vec::new(),
        ..on_days(RecurrenceFrequency::Yearly, Some(2))
    };
    let month_thirteen = SimpleRecurrence {
        days: Vec::new(),
        months: vec![13],
        ..drawable()
    };
    let no_such_day = SimpleRecurrence {
        days: Vec::new(),
        month_days: vec![40],
        ..drawable()
    };
    let day_zero = SimpleRecurrence {
        days: Vec::new(),
        month_days: vec![0],
        ..drawable()
    };
    let sixth_monday = on_days(RecurrenceFrequency::Monthly, Some(6));
    let far_interval = SimpleRecurrence {
        interval: 1_000_001,
        days: Vec::new(),
        ..drawable()
    };

    for (what, rule) in [
        (
            "an nth weekday of a week",
            on_days(RecurrenceFrequency::Weekly, Some(4)),
        ),
        (
            "an nth weekday of a day",
            on_days(RecurrenceFrequency::Daily, Some(4)),
        ),
        ("an nth weekday of a whole year", yearly_nth_no_month),
        ("a thirteenth month", month_thirteen),
        ("the fortieth of the month", no_such_day),
        ("the zeroth of the month", day_zero),
        ("a sixth Monday", sixth_monday),
        ("an interval past the drawable span", far_interval),
    ] {
        assert!(
            undrawable_reason(&rule).is_some(),
            "{what} produces an event nobody would ever see"
        );
    }
}

#[test]
fn a_refusal_names_a_rule_part_and_nothing_else() {
    // The reason reaches the diagnostic log, so it must describe the rule rather than the
    // meeting; `docs/logging.md`. Every one is a fixed string chosen here; none is assembled
    // from the event.
    let reason = undrawable_reason(&on_days(RecurrenceFrequency::Weekly, Some(4)))
        .expect("this one is refused");

    assert!(reason.is_ascii() && !reason.is_empty());
    assert!(
        !reason.contains('@') && !reason.contains('/'),
        "a reason carries no address and no resource path"
    );
}
