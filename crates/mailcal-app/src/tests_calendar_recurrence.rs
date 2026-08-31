//! What a repeat rule does once it is on the wire: the wiring the builders' own unit tests
//! (`mailcal_account::calendar`) cannot see.
//!
//! Two things are only assertable here. A delete of one occurrence is named by the very token
//! the grid handed the client, so what the core offers and what it accepts are proven to be
//! the same string, and a rule this app could not describe in full is refused *and said to
//! have failed*, rather than refused into silence.
//!
//! Split out of `tests_calendar_actions.rs`, which is near the 500-line limit.

use std::sync::{Arc, Mutex};

use engine_api::LocalDateTime;
use engine_provider::DeleteTarget;
use fakes::{CalendarFake, calendar_account, calendar_app, evt};
use mailcal_account::{
    RecurrenceChange, RecurrenceDay, RecurrenceEnd, RecurrenceFrequency, RecurrenceWeekday,
    SimpleRecurrence, recurrence_rule_of,
};

use super::{CalendarWriteStatus, Intent};

#[allow(clippy::duplicate_mod)]
#[path = "tests_fakes.rs"]
mod fakes;

/// A rule repeating every `interval` weeks, for ever.
fn every_weeks(interval: u32) -> SimpleRecurrence {
    SimpleRecurrence {
        frequency: RecurrenceFrequency::Weekly,
        interval,
        days: Vec::new(),
        month_days: Vec::new(),
        months: Vec::new(),
        end: RecurrenceEnd::Never,
    }
}

/// The calendar date `offset` days from today.
fn day_from_today(offset: i64) -> engine_api::CalendarDate {
    mailcal_viewmodel::calendar::days::date_at(
        i64::try_from(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("after the epoch")
                .as_secs()
                / 86_400,
        )
        .expect("in range")
            + offset,
    )
}

/// The occurrence token the grid offers for `event` on the page `offset` days out: the exact
/// string a client reads off `TimedSegment` and hands back on a delete.
fn occurrence_token(app: &super::App<CalendarFake>, offset: i64, event: &str) -> String {
    app.calendar_range(day_from_today(offset), 1)
        .grid
        .timed
        .into_iter()
        .find(|segment| segment.event == event)
        .expect("the occurrence is drawn")
        .occurrence_start
}

#[tokio::test]
async fn a_create_that_repeats_reaches_the_provider_with_its_rule() {
    // The interval is what the old frequency token could not carry, so it is what this asserts:
    // a fortnightly event must arrive as a fortnightly rule, not as a weekly one.
    let provider = CalendarFake::with_event("standup");
    let rules = provider.create_rules();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(vec![calendar_account("acct-a", provider)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;

    app.dispatch(Intent::CreateEvent {
        title: "Retro".to_owned(),
        start: "2026-09-01T09:00:00Z".to_owned(),
        end: "2026-09-01T09:30:00Z".to_owned(),
        account: None,
        calendar: None,
        all_day: false,
        timezone: None,
        notes: None,
        location: None,
        recurrence: Some(every_weeks(2)),
    })
    .await;

    let rules = rules.lock().unwrap();
    let [repeat] = rules.as_slice() else {
        panic!("expected exactly one create");
    };
    let repeat = repeat.as_ref().expect("the created event repeats");
    assert_eq!(
        repeat.rule,
        recurrence_rule_of(&every_weeks(2)).expect("a well-formed rule"),
        "the rule the user chose, interval and all"
    );
}

#[tokio::test]
async fn a_create_without_a_rule_still_makes_a_single_event() {
    // The default every existing client sends. It has to stay a one-off rather than acquiring
    // an empty rule on the way through.
    let provider = CalendarFake::with_event("standup");
    let rules = provider.create_rules();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(vec![calendar_account("acct-a", provider)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;

    app.dispatch(Intent::CreateEvent {
        title: "Dentist".to_owned(),
        start: "2026-09-01T09:00:00Z".to_owned(),
        end: "2026-09-01T09:30:00Z".to_owned(),
        account: None,
        calendar: None,
        all_day: false,
        timezone: None,
        notes: None,
        location: None,
        recurrence: None,
    })
    .await;

    assert_eq!(rules.lock().unwrap().as_slice(), [None]);
}

#[tokio::test]
async fn the_month_grid_names_the_same_occurrence_the_time_grid_does() {
    // A client asks "this one, or all of them?" from wherever the user opened the event, so
    // every surface that can open one has to name the occurrence it drew. The month grid could
    // reach a delete with nothing to hand back, and the only symptom would be a series the user
    // meant to thin out disappearing instead.
    //
    // The fixture is a series with one occurrence **moved**, so identity and position are
    // different instants: a chip naming where the block sits would pass against an unmoved one.
    let provider = CalendarFake::with_events(vec![fakes::weekly_event_with_a_moved_occurrence(
        "standup", 0, 9, 30, 1, 14,
    )]);
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(vec![calendar_account("acct-a", provider)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;

    let moved = day_from_today(7);
    let from_the_time_grid = occurrence_token(&app, 7, "standup");
    let from_the_month_grid = app
        .month_page(moved)
        .grid
        .cells
        .into_iter()
        .find(|cell| cell.date == moved.to_string())
        .expect("the day is on the page")
        .chips
        .into_iter()
        .find(|chip| chip.event == "standup")
        .expect("the occurrence is drawn")
        .occurrence_start;

    assert!(
        !from_the_time_grid.is_empty(),
        "the fixture has to be a series for this to prove anything"
    );
    assert_eq!(
        from_the_month_grid, from_the_time_grid,
        "the month grid names the occurrence the time grid names"
    );
}

#[tokio::test]
async fn deleting_one_occurrence_names_it_by_the_token_the_grid_offered() {
    // The round trip that matters: the client reads `occurrence_start` off the segment it drew
    // and hands that same string back. If the two ever disagree, a user cancelling next
    // Monday's standup cancels nothing, or cancels the standup.
    let provider =
        CalendarFake::with_events(vec![fakes::weekly_event_from_today("standup", 0, 9, 30)]);
    let targets = provider.delete_targets();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(vec![calendar_account("acct-a", provider)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;

    let token = occurrence_token(&app, 7, "standup");
    app.dispatch(Intent::DeleteEvent {
        event: evt("acct-a", "standup"),
        occurrence: Some(token.parse().expect("the grid's token is a wall clock")),
    })
    .await;

    let targets = targets.lock().unwrap();
    let [DeleteTarget::Occurrence { occurrence, .. }] = targets.as_slice() else {
        panic!("expected one delete, of one occurrence: {targets:?}");
    };
    assert_eq!(
        occurrence.start.local().map(|start| start.to_string()),
        Some(token),
        "the occurrence removed is the one the grid named"
    );
}

#[tokio::test]
async fn a_delete_that_names_no_occurrence_removes_the_whole_series() {
    // The other half of the same question, and the behaviour every client has today. There is
    // no default in the core precisely because these two are different requests.
    let provider =
        CalendarFake::with_events(vec![fakes::weekly_event_from_today("standup", 0, 9, 30)]);
    let targets = provider.delete_targets();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(vec![calendar_account("acct-a", provider)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;

    app.dispatch(Intent::DeleteEvent {
        event: evt("acct-a", "standup"),
        occurrence: None,
    })
    .await;

    assert_eq!(targets.lock().unwrap().as_slice(), [DeleteTarget::Series]);
}

#[tokio::test]
async fn an_edit_of_a_rule_we_could_not_describe_fails_out_loud() {
    // The guard, reached the way a client would reach it. What makes this worth a wiring test
    // rather than only a builder one: the refusal must arrive as `Failed`. A guard that
    // refuses into silence leaves the user looking at an unchanged event, which reads exactly
    // like a save that worked.
    let mut series = fakes::weekly_event_from_today("standup", 0, 9, 30);
    series
        .recurrence
        .as_mut()
        .expect("the fixture repeats")
        .rules[0]
        .by_set_position = vec![4];
    let provider = CalendarFake::with_events(vec![series]);
    let patches = provider.patches();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(vec![calendar_account("acct-a", provider)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;

    app.dispatch(Intent::UpdateEvent {
        event: evt("acct-a", "standup"),
        edit: mailcal_account::EventEdit {
            recurrence: Some(RecurrenceChange::Set(every_weeks(1))),
            ..mailcal_account::EventEdit::default()
        },
    })
    .await;

    assert!(
        patches.lock().unwrap().is_empty(),
        "nothing was sent: the rule we could not read is still the server's"
    );
    assert_eq!(app.calendar_write_status(), CalendarWriteStatus::Failed);
}

/// A wall clock one hour after the occurrence the grid drew for `event`: a time that reads
/// perfectly well and names no instance of the series.
fn an_hour_off_the_grid(app: &super::App<CalendarFake>, offset: i64, event: &str) -> LocalDateTime {
    let drawn: LocalDateTime = occurrence_token(app, offset, event)
        .parse()
        .expect("the grid's token is a wall clock");
    LocalDateTime::new(
        drawn.year(),
        drawn.month(),
        drawn.day(),
        drawn.hour() + 1,
        drawn.minute(),
        drawn.second(),
    )
    .expect("an hour later is a wall clock too")
}

#[tokio::test]
async fn a_delete_of_an_occurrence_the_series_does_not_have_is_refused() {
    // A near miss rather than nonsense: the right event, the right day, an hour out. The
    // transports do not agree about what that does; one removes nothing and reports success,
    // another rewrites the series document around a slot the rule never produces: so the
    // answer is decided here, before anything is sent.
    let provider =
        CalendarFake::with_events(vec![fakes::weekly_event_from_today("standup", 0, 9, 30)]);
    let targets = provider.delete_targets();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(vec![calendar_account("acct-a", provider)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;

    app.dispatch(Intent::DeleteEvent {
        event: evt("acct-a", "standup"),
        occurrence: Some(an_hour_off_the_grid(&app, 7, "standup")),
    })
    .await;

    assert!(
        targets.lock().unwrap().is_empty(),
        "nothing was deleted, and above all not the series, which is the one outcome nobody \
         can undo"
    );
    assert_eq!(app.calendar_write_status(), CalendarWriteStatus::Failed);
}

#[tokio::test]
async fn an_edit_of_an_occurrence_the_series_does_not_have_is_refused() {
    let provider =
        CalendarFake::with_events(vec![fakes::weekly_event_from_today("standup", 0, 9, 30)]);
    let patches = provider.patches();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(vec![calendar_account("acct-a", provider)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;

    app.dispatch(Intent::UpdateEvent {
        event: evt("acct-a", "standup"),
        edit: mailcal_account::EventEdit {
            title: Some("Retro".to_owned()),
            occurrence: Some(an_hour_off_the_grid(&app, 7, "standup")),
            ..mailcal_account::EventEdit::default()
        },
    })
    .await;

    assert!(
        patches.lock().unwrap().is_empty(),
        "nothing was patched: an override split at a slot the rule never produces is a \
         meeting drawn twice"
    );
    assert_eq!(app.calendar_write_status(), CalendarWriteStatus::Failed);
}

#[tokio::test]
async fn an_occurrence_named_on_an_event_that_does_not_repeat_is_refused() {
    // A one-off carries no token, so this is a client asking to split an override out of an
    // event with no series to split it from. CalDAV would write a `RECURRENCE-ID` into a
    // document that has no rule.
    let provider = CalendarFake::with_events(vec![fakes::event_from_today("dentist", 2, 11, 45)]);
    let targets = provider.delete_targets();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(vec![calendar_account("acct-a", provider)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;

    app.dispatch(Intent::DeleteEvent {
        event: evt("acct-a", "dentist"),
        occurrence: Some(LocalDateTime::new(2026, 9, 1, 11, 45, 0).unwrap()),
    })
    .await;

    assert!(targets.lock().unwrap().is_empty());
    assert_eq!(app.calendar_write_status(), CalendarWriteStatus::Failed);
}

/// How many blocks the grid draws for a series following `rule`, over a year and a bit.
///
/// The real engine expands it (`CalendarFake` only hands the event over) so this measures
/// what the app can actually show rather than what a table says it should.
async fn blocks_drawn(rule: &SimpleRecurrence) -> usize {
    let mut event = fakes::weekly_event_from_today("probe", 0, 9, 30);
    event.recurrence = Some(engine_api::Recurrence::from_rule(
        mailcal_account::recurrence_rule_of(rule).expect("the rule rebuilds"),
    ));
    let provider = CalendarFake::with_events(vec![event]);
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(vec![calendar_account("acct-a", provider)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;
    app.calendar_range(day_from_today(-30), 400)
        .grid
        .timed
        .into_iter()
        .filter(|segment| segment.event == "probe")
        .count()
}

/// A monthly rule on the fourth Monday, before the case under test spoils it.
fn monthly_fourth_monday() -> SimpleRecurrence {
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

#[tokio::test]
async fn an_undrawable_rule_really_cannot_be_drawn() {
    // The guard in `mailcal_account::undrawable_reason` is a hand-written list, and a list is
    // only as good as the last time somebody checked it. This checks it; against the engine
    // that ships, over the grid a user reads.
    //
    // **If a case here goes red, the engine has grown to cover it and the core should stop
    // refusing it.** That is the failure this test exists to produce.
    let over_a_year = 12;
    assert!(
        blocks_drawn(&monthly_fourth_monday()).await > over_a_year,
        "the control draws a real series, so a zero below means the rule and not the harness"
    );

    let cases = [
        (
            "an nth weekday of a week",
            SimpleRecurrence {
                frequency: RecurrenceFrequency::Weekly,
                ..monthly_fourth_monday()
            },
        ),
        (
            "an nth weekday of a whole year",
            SimpleRecurrence {
                frequency: RecurrenceFrequency::Yearly,
                ..monthly_fourth_monday()
            },
        ),
        (
            "a thirteenth month",
            SimpleRecurrence {
                days: Vec::new(),
                months: vec![13],
                ..monthly_fourth_monday()
            },
        ),
        (
            "the fortieth of the month",
            SimpleRecurrence {
                days: Vec::new(),
                month_days: vec![40],
                ..monthly_fourth_monday()
            },
        ),
        (
            "a sixth Monday",
            SimpleRecurrence {
                days: vec![RecurrenceDay {
                    day: RecurrenceWeekday::Monday,
                    nth: Some(6),
                }],
                ..monthly_fourth_monday()
            },
        ),
        (
            "an interval past the drawable span",
            SimpleRecurrence {
                interval: 1_000_001,
                days: Vec::new(),
                ..monthly_fourth_monday()
            },
        ),
    ];

    for (what, rule) in cases {
        assert!(
            mailcal_account::undrawable_reason(&rule).is_some(),
            "{what} is on the refused list"
        );
        assert!(
            blocks_drawn(&rule).await <= 1,
            "{what} draws no series; at most the event's own start, which is not a repeat"
        );
    }
}

#[tokio::test]
async fn a_create_of_a_rule_we_could_not_draw_is_refused() {
    // The point of the guard, from the user's side: a write that fails is recoverable, and an
    // event stored where nothing can ever show it is not; it is simply gone, and no amount of
    // looking finds it.
    let provider = CalendarFake::with_event("standup");
    let creations = provider.creations();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(vec![calendar_account("acct-a", provider)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;

    app.dispatch(Intent::CreateEvent {
        title: "Retro".to_owned(),
        start: "2026-09-01T09:00:00Z".to_owned(),
        end: "2026-09-01T09:30:00Z".to_owned(),
        account: None,
        calendar: None,
        all_day: false,
        timezone: None,
        notes: None,
        location: None,
        recurrence: Some(SimpleRecurrence {
            frequency: RecurrenceFrequency::Weekly,
            ..monthly_fourth_monday()
        }),
    })
    .await;

    assert!(
        creations.lock().unwrap().is_empty(),
        "nothing was created; better a failure the user can see than an event they cannot"
    );
}
