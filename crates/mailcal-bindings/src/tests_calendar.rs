//! The calendar-grid FFI surface, driven exactly as a client drives it.
//!
//! The showcase app's events sit around *today*, so the assertions here hold whenever the
//! suite runs rather than expiring on a date nobody chose.

use std::{
    sync::mpsc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use super::*;
use crate::tests::{ChannelObserver, NullLogger};

/// Today's `YYYY-MM-DD` in UTC: the anchor a client would pass.
fn today() -> String {
    let days = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("after the epoch")
        .as_secs()
        / 86_400;
    mailcal_viewmodel::calendar::days::date_at(i64::try_from(days).expect("in range")).to_string()
}

/// A client asks for a page and gets a drawable grid: day columns, positioned blocks,
/// resolved colours, and the calendars to key them against.
///
/// This is the whole read path across the FFI: no snapshot slot, no observer in the loop.
#[test]
fn a_client_pulls_a_drawable_week_straight_through_the_ffi() {
    let (tx, rx) = mpsc::channel();
    let app = MailcalApp::new_showcase(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        "Europe/Amsterdam".to_owned(),
        ShowcaseLocale::En,
    );

    // A page always hands back a full set of day columns, so a client can draw the grid's
    // chrome (and a loading state over it) rather than flashing a blank screen.
    //
    // It used to assert `!is_materialized` here too; "before any sync the page is unknown, not
    // empty". The showcase boot now primes the calendar, so this app is no longer a cold fixture
    // and that assertion would only be re-testing the priming. The property itself is owned by
    // `mailcal_app::tests_calendar::before_the_first_sync_nothing_is_materialized`, over an app
    // that really has not synced.
    assert_eq!(
        app.calendar_range(app.week_start_date(today()), 7)
            .days
            .len(),
        7
    );

    app.dispatch(Intent::RefreshCalendar);
    let mut settled = false;
    while let Ok(surface) = rx.recv_timeout(Duration::from_secs(5)) {
        if matches!(surface, Surface::Calendar) {
            settled = true;
            break;
        }
    }
    assert!(settled, "the refresh signalled Surface::Calendar");

    let page = app.calendar_range(app.week_start_date(today()), 7);
    assert!(page.is_materialized);
    assert_eq!(page.days.len(), 7);
    assert_eq!(page.timezone, "Europe/Amsterdam");

    // The showcase's three calendars (Work, Personal, Family), each with a resolved colour the
    // client paints with directly; including the label colour, so it never computes contrast
    // itself. The three colours are distinct: a work-and-private-life calendar reads by colour.
    assert_eq!(page.calendars.len(), 3);
    for calendar in &page.calendars {
        let color = &calendar.color;
        assert!(color.hex.starts_with('#'));
        assert!(color.light.text.starts_with('#') && color.dark.text.starts_with('#'));
        assert_ne!(color.light.background, color.dark.background);
    }
    let distinct: std::collections::HashSet<_> =
        page.calendars.iter().map(|c| &c.color.hex).collect();
    assert_eq!(distinct.len(), 3, "each calendar has its own colour");

    // The week's events came through as positioned blocks, each keyed to a calendar the
    // client can look the colour up in.
    assert!(!page.timed.is_empty(), "the showcase week has meetings");
    for block in &page.timed {
        assert!(block.day < 7);
        assert!(
            block.end_minutes > block.start_minutes,
            "blocks have height"
        );
        assert!(block.column < block.columns, "a lane fits inside its split");
        assert!(
            page.calendars.iter().any(|c| c.id == block.calendar),
            "every block names a calendar the page also lists"
        );
    }
    // Today's standup is at 09:30 local: the grid positions by wall clock, not by the UTC
    // instant, so a client places it on the 09:30 row without doing any zone maths.
    assert!(
        page.timed
            .iter()
            .any(|block| block.title == "Team standup" && block.start_minutes == 570),
        "got {:?}",
        page.timed
            .iter()
            .map(|b| (&b.title, b.start_minutes))
            .collect::<Vec<_>>()
    );
}

/// Every zoom level reads the same query with a different column count; day, three-day and week
/// are one grid at three zooms, not three features.
#[test]
fn the_zoom_picks_the_columns() {
    let (tx, _rx) = mpsc::channel();
    let app = MailcalApp::new_showcase(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        "Europe/Amsterdam".to_owned(),
        ShowcaseLocale::En,
    );
    let columns = |n| app.calendar_range(today(), n).days.len();
    assert_eq!(columns(1), 1);
    assert_eq!(columns(3), 3);
    assert_eq!(columns(5), 5);
    assert_eq!(columns(7), 7);

    // And widening keeps the SAME first day: a zoom must never relocate the grid. Snapping to a
    // Monday-aligned week instead would have to: it cannot contain an arbitrary three-day window.
    let three = app.calendar_range(today(), 3);
    let seven = app.calendar_range(today(), 7);
    assert_eq!(three.days[0].date, seven.days[0].date);
}

/// A malformed anchor falls back to today rather than failing the draw. A host bug should
/// cost the user the wrong week, not a blank screen.
#[test]
fn a_junk_anchor_still_returns_a_drawable_page() {
    let (tx, _rx) = mpsc::channel();
    let app = MailcalApp::new_showcase(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        "Europe/Amsterdam".to_owned(),
        ShowcaseLocale::En,
    );
    let page = app.calendar_range("not-a-date".to_owned(), 7);
    assert_eq!(page.days.len(), 7);
    // It fell back to *today's* week, so the page a client draws is a real one.
    assert!(page.days.iter().any(|day| day.date == today()));
}

/// A client taps an event and pulls its full detail straight through the FFI: the read the
/// detail view opens on. A missing event returns `None`, not an error.
#[test]
fn a_client_pulls_one_events_detail_through_the_ffi() {
    let (tx, rx) = mpsc::channel();
    let app = MailcalApp::new_showcase(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        "Europe/Amsterdam".to_owned(),
        ShowcaseLocale::En,
    );
    app.dispatch(Intent::RefreshCalendar);
    while let Ok(surface) = rx.recv_timeout(Duration::from_secs(5)) {
        if matches!(surface, Surface::Calendar) {
            break;
        }
    }

    let page = app.calendar_range(app.week_start_date(today()), 7);
    let block = page
        .timed
        .iter()
        .find(|block| block.title == "Team standup")
        .expect("the showcase week has the standup");

    let detail = app
        .event_detail(block.account.clone(), block.event.clone(), None)
        .expect("a real event has detail");
    assert_eq!(detail.title, "Team standup");
    assert_eq!(detail.account, block.account);
    assert_eq!(detail.key, block.event);
    assert_eq!(detail.calendar, block.calendar);
    assert!(!detail.all_day);
    assert!(
        !detail.start.is_empty() && !detail.end.is_empty(),
        "a timed event has a start and end wall clock"
    );

    assert!(
        app.event_detail(block.account.clone(), "no-such-event".to_owned(), None)
            .is_none(),
        "a stale reference is a closed sheet, not a crash"
    );
}

/// The editor intent crosses the FFI as strings and is parsed into a typed `EventEdit` at the
/// boundary: wall-clocks become `LocalDateTime`s, an empty notes/location is a *clear*, and a
/// value sets. This is the seam a client's editor dispatches through.
#[test]
fn an_update_event_intent_parses_into_a_typed_edit() {
    let converted = mailcal_app::Intent::try_from(Intent::UpdateEvent {
        account: "acct-a".to_owned(),
        key: "/cal/e.ics".to_owned(),
        title: Some("Standup (kort)".to_owned()),
        start: Some("2026-01-05T10:00:00".to_owned()),
        end: Some("2026-01-05T10:30:00".to_owned()),
        notes: Some(String::new()), // clear
        location: Some("Room 2".to_owned()),
        occurrence: Some("2026-01-05T09:30:00".to_owned()),
        recurrence: None,
        times_from_occurrence: None,
    })
    .expect("a well-formed edit converts");

    let mailcal_app::Intent::UpdateEvent { edit, .. } = converted else {
        panic!("expected an UpdateEvent");
    };
    assert_eq!(edit.title.as_deref(), Some("Standup (kort)"));
    assert!(edit.start.is_some() && edit.end.is_some());
    assert_eq!(
        edit.notes.as_deref(),
        Some(""),
        "empty notes preserved as a clear"
    );
    assert_eq!(edit.location.as_deref(), Some("Room 2"));
    assert!(
        edit.occurrence.is_some(),
        "a single-occurrence edit keeps its anchor"
    );
}

/// Empty `title`/`start`/`end` mean "leave unchanged" (an event must keep them, so they can't
/// be cleared); a malformed wall-clock drops the whole intent rather than editing a wrong time.
#[test]
fn an_update_event_leaves_empty_required_fields_and_rejects_a_bad_time() {
    let left_alone = mailcal_app::Intent::try_from(Intent::UpdateEvent {
        account: "acct-a".to_owned(),
        key: "/cal/e.ics".to_owned(),
        title: Some(String::new()),
        start: Some(String::new()),
        end: None,
        notes: None,
        location: None,
        occurrence: None,
        recurrence: None,
        times_from_occurrence: None,
    })
    .expect("empty required fields are valid; they change nothing");
    let mailcal_app::Intent::UpdateEvent { edit, .. } = left_alone else {
        panic!("expected an UpdateEvent");
    };
    assert!(edit.title.is_none() && edit.start.is_none() && edit.end.is_none());

    let rejected = mailcal_app::Intent::try_from(Intent::UpdateEvent {
        account: "acct-a".to_owned(),
        key: "/cal/e.ics".to_owned(),
        title: None,
        start: Some("not-a-time".to_owned()),
        end: None,
        notes: None,
        location: None,
        occurrence: None,
        recurrence: None,
        times_from_occurrence: None,
    });
    assert!(rejected.is_err(), "a malformed wall-clock drops the intent");
}

/// A weekly rule as a client would build it over the FFI.
fn weekly_rule() -> SimpleRecurrence {
    SimpleRecurrence {
        frequency: RecurrenceFrequency::Weekly,
        interval: 2,
        days: vec![RecurrenceDay {
            day: RecurrenceWeekday::Thursday,
            nth: None,
        }],
        month_days: Vec::new(),
        months: Vec::new(),
        end: RecurrenceEnd::AfterCount { count: 10 },
    }
}

/// A repeat rule crosses the FFI as a structure and arrives as one: not as a string a layer
/// underneath has to parse. Every field is checked, because a dropped one would show up as a
/// series quietly repeating on the wrong rhythm rather than as an error.
#[test]
fn a_create_carries_its_repeat_rule_across_the_boundary() {
    let converted = mailcal_app::Intent::try_from(Intent::CreateEvent {
        title: "Retro".to_owned(),
        start: "2026-09-03T15:00:00".to_owned(),
        end: "2026-09-03T16:00:00".to_owned(),
        account: None,
        calendar: None,
        all_day: false,
        timezone: Some("Europe/Amsterdam".to_owned()),
        notes: None,
        location: None,
        recurrence: Some(weekly_rule()),
    })
    .expect("a well-formed create converts");

    let mailcal_app::Intent::CreateEvent { recurrence, .. } = converted else {
        panic!("expected a CreateEvent");
    };
    let rule = recurrence.expect("the new event repeats");
    assert_eq!(rule.frequency, mailcal_account::RecurrenceFrequency::Weekly);
    assert_eq!(rule.interval, 2);
    assert_eq!(
        rule.days,
        vec![mailcal_account::RecurrenceDay {
            day: mailcal_account::RecurrenceWeekday::Thursday,
            nth: None,
        }]
    );
    assert_eq!(
        rule.end,
        mailcal_account::RecurrenceEnd::AfterCount { count: 10 }
    );
}

/// The three states of a recurrence edit stay three across the boundary. Collapsing "leave it
/// alone" into "clear it" would turn every save of a repeating event into a save that stops it
/// repeating.
#[test]
fn an_edit_keeps_leaving_a_rule_alone_distinct_from_removing_it() {
    let edit_of = |change| {
        let converted = mailcal_app::Intent::try_from(Intent::UpdateEvent {
            account: "acct-a".to_owned(),
            key: "e.ics".to_owned(),
            title: None,
            start: None,
            end: None,
            notes: None,
            location: None,
            occurrence: None,
            recurrence: change,
            times_from_occurrence: None,
        })
        .expect("a well-formed edit converts");
        let mailcal_app::Intent::UpdateEvent { edit, .. } = converted else {
            panic!("expected an UpdateEvent");
        };
        edit.recurrence
    };

    assert_eq!(
        edit_of(None),
        None,
        "an edit that says nothing changes none"
    );
    assert_eq!(
        edit_of(Some(RecurrenceChange::Clear)),
        Some(mailcal_account::RecurrenceChange::Clear)
    );
    let Some(mailcal_account::RecurrenceChange::Set(rule)) = edit_of(Some(RecurrenceChange::Set {
        rule: weekly_rule(),
    })) else {
        panic!("expected the rule to be replaced");
    };
    assert_eq!(rule.interval, 2);
}

/// A delete names its occurrence with the same token the grid handed out, parsed on the same
/// terms as the editor's. A value that is not a wall clock drops the whole intent rather than
/// deleting the entire series when the user asked for one Tuesday.
#[test]
fn a_delete_names_one_occurrence_or_the_whole_series() {
    let occurrence_of = |token: Option<&str>| {
        mailcal_app::Intent::try_from(Intent::DeleteEvent {
            account: "acct-a".to_owned(),
            key: "e.ics".to_owned(),
            occurrence: token.map(str::to_owned),
        })
        .map(|converted| {
            let mailcal_app::Intent::DeleteEvent { occurrence, .. } = converted else {
                panic!("expected a DeleteEvent");
            };
            occurrence
        })
    };

    assert_eq!(
        occurrence_of(Some("2026-01-05T09:30:00")).expect("a wall clock converts"),
        Some("2026-01-05T09:30:00".parse().unwrap()),
    );
    assert_eq!(
        occurrence_of(None).expect("naming no occurrence converts"),
        None,
        "no occurrence named means the whole series"
    );
    assert!(
        occurrence_of(Some("next Tuesday")).is_err(),
        "a malformed token drops the intent rather than deleting the series"
    );
}

/// A draft the editor never touched asks for no write at all, and one whose frequency moved on
/// leaves behind the part that belonged to the old frequency. Both decisions are the core's, and
/// this is the boundary they have to survive: a client sends the draft it holds and nothing else.
#[test]
fn a_repeat_draft_decides_the_same_thing_on_both_sides_of_the_boundary() {
    let stored = SimpleRecurrence {
        frequency: RecurrenceFrequency::Monthly,
        interval: 1,
        days: Vec::new(),
        // The month's last day: a rule no control models, so it has to be carried.
        month_days: vec![-1],
        months: Vec::new(),
        end: RecurrenceEnd::Never,
    };
    let draft = RepeatDraft {
        frequency: RecurrenceFrequency::Monthly,
        interval: 1,
        weekdays: vec![RecurrenceWeekday::Tuesday],
        end: RecurrenceEnd::Never,
        stored: Some(stored.clone()),
    };

    assert_eq!(
        crate::repeat_change_of(Some(draft.clone()), true),
        None,
        "an untouched repeat asks for no write"
    );

    let mut ended = draft.clone();
    ended.end = RecurrenceEnd::AfterCount { count: 10 };
    let Some(RecurrenceChange::Set { rule }) = crate::repeat_change_of(Some(ended), true) else {
        panic!("a changed repeat is a Set");
    };
    assert_eq!(
        rule.month_days,
        vec![-1],
        "the last day survives the crossing"
    );

    let mut weekly = draft;
    weekly.frequency = RecurrenceFrequency::Weekly;
    let Some(RecurrenceChange::Set { rule }) = crate::repeat_change_of(Some(weekly), true) else {
        panic!("a changed repeat is a Set");
    };
    assert!(
        rule.month_days.is_empty(),
        "a day of the month means nothing in a week"
    );
    assert_eq!(
        rule.days,
        vec![RecurrenceDay {
            day: RecurrenceWeekday::Tuesday,
            nth: None
        }]
    );
}
