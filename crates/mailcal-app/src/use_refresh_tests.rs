//! A calendar that connected and then stops answering: its refreshes say so, the week takes no
//! change while they do, and a refresh that reaches the server again takes it away.

use std::sync::{Arc, Mutex, atomic::Ordering};

use engine_api::AccountId;
use fakes::{CalendarFake, calendar_account, calendar_app, event_from_today};
use mailcal_viewmodel::calendar::days::date_at;

use super::RefreshedUse;
use crate::{CalendarWriteStatus, EventIntent, Intent, Surface, UseProblem, UseProblems};

#[allow(clippy::duplicate_mod)]
#[path = "tests_fakes.rs"]
mod fakes;

fn today() -> engine_api::CalendarDate {
    date_at(
        i64::try_from(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("after the epoch")
                .as_secs()
                / 86_400,
        )
        .expect("in range"),
    )
}

fn create() -> Intent {
    Intent::Events(EventIntent::Create {
        title: "Kickoff".to_owned(),
        start: "2026-09-01T09:00:00Z".to_owned(),
        end: "2026-09-01T09:30:00Z".to_owned(),
        account: None,
        calendar: None,
        all_day: false,
        timezone: None,
        notes: None,
        location: None,
        recurrence: None,
        invitees: None,
        meeting: None,
    })
}

const UNREACHABLE_CALENDAR: UseProblems = UseProblems {
    calendar: Some(UseProblem::Unreachable),
    contacts: None,
};

/// Whether every calendar row and event on this week can take a change.
fn writable(app: &crate::App<CalendarFake>) -> bool {
    let page = app.calendar_range(app.week_start_date(today()), 7);
    assert!(
        !page.calendars.is_empty(),
        "the stored calendar stays listed"
    );
    assert!(!page.grid.timed.is_empty(), "the stored event stays drawn");
    let rows = page.calendars.iter().all(|row| row.can_write);
    let events = page.grid.timed.iter().all(|event| event.can_write);
    assert_eq!(rows, events, "the rows and their events agree");
    rows
}

/// The account's mail is connected, its calendar connected and synced, and then its server
/// stops answering. Two refreshes that reach nothing record the calendar unreachable, as a dial
/// that could not reach it would; while that stands nothing on the week takes a change, and a
/// create that arrives anyway is not saved and sends nothing. A refresh that reaches the server
/// takes it all away.
#[tokio::test]
async fn a_calendar_that_stops_answering_says_so_and_clears_when_a_refresh_reaches_it() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let provider = CalendarFake::with_events(vec![event_from_today("standup", 0, 9, 30)]);
    let outage = provider.outage();
    let creations = provider.creations();
    let app = calendar_app(vec![calendar_account("acct", provider)], &surfaces);
    let id = AccountId::try_from("acct").unwrap();
    app.dispatch(Intent::RefreshCalendar).await;
    assert!(writable(&app));
    assert_eq!(app.use_problems(&id), UseProblems::default());

    outage.store(true, Ordering::Relaxed);
    surfaces.lock().unwrap().clear();
    app.refresh_calendar_in_background().await;
    assert_eq!(
        app.use_problems(&id),
        UseProblems::default(),
        "one refresh that reached nothing is not an outage"
    );
    assert!(!surfaces.lock().unwrap().contains(&Surface::Connectivity));
    assert!(writable(&app));

    app.refresh_calendar_in_background().await;
    assert_eq!(app.use_problems(&id), UNREACHABLE_CALENDAR);
    assert!(surfaces.lock().unwrap().contains(&Surface::Connectivity));
    assert!(
        !writable(&app),
        "a calendar that is not answering takes no change"
    );

    app.dispatch(create()).await;
    assert_eq!(app.calendar_write_status(), CalendarWriteStatus::NotSaved);
    assert!(creations.lock().unwrap().is_empty(), "nothing was sent");

    app.refresh_calendar_in_background().await;
    assert_eq!(
        app.use_problems(&id),
        UNREACHABLE_CALENDAR,
        "it stays while the server does not answer"
    );

    outage.store(false, Ordering::Relaxed);
    surfaces.lock().unwrap().clear();
    app.refresh_calendar_in_background().await;
    assert_eq!(app.use_problems(&id), UseProblems::default());
    assert!(surfaces.lock().unwrap().contains(&Surface::Connectivity));
    assert!(writable(&app));

    app.dispatch(create()).await;
    assert_eq!(
        creations.lock().unwrap().len(),
        1,
        "the calendar takes a create again"
    );
}

/// A refresh says nothing about a problem only a dial gets: a server that refused the account's
/// sign-in is not answered by a refresh that reached it, and its record stays.
#[tokio::test]
async fn a_refresh_leaves_a_problem_only_a_dial_gets() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(
        vec![calendar_account(
            "acct",
            CalendarFake::with_events(Vec::new()),
        )],
        &surfaces,
    );
    let id = AccountId::try_from("acct").unwrap();
    let refused = UseProblems {
        calendar: Some(UseProblem::SignInRefused),
        contacts: None,
    };
    app.note_use_problems(&id, refused);

    assert!(!app.note_use_refreshed(&id, RefreshedUse::Calendar, Some(true)));
    assert!(!app.note_use_refreshed(&id, RefreshedUse::Calendar, Some(false)));
    assert!(!app.note_use_refreshed(&id, RefreshedUse::Calendar, Some(false)));
    assert_eq!(app.use_problems(&id), refused);
}

/// Offline, every refresh fails, and the fault is the device's: it counts towards nothing.
#[tokio::test]
async fn a_refresh_while_offline_counts_for_nothing() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(
        vec![calendar_account(
            "acct",
            CalendarFake::with_events(Vec::new()),
        )],
        &surfaces,
    );
    let id = AccountId::try_from("acct").unwrap();
    app.note_network_reachable(false);

    for _ in 0..3 {
        app.note_use_refreshed(&id, RefreshedUse::Calendar, Some(false));
    }
    assert_eq!(app.use_problems(&id), UseProblems::default());

    app.note_network_reachable(true);
    app.note_use_refreshed(&id, RefreshedUse::Calendar, Some(false));
    assert_eq!(
        app.use_problems(&id),
        UseProblems::default(),
        "the offline refreshes left no miss behind"
    );
}

/// The misses are counted in a row: a refresh that reached the server, or a dial, which answers
/// for the use afresh, starts the count again. A pass that says nothing leaves it.
#[tokio::test]
async fn only_misses_in_a_row_record_the_use_unreachable() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(
        vec![calendar_account(
            "acct",
            CalendarFake::with_events(Vec::new()),
        )],
        &surfaces,
    );
    let id = AccountId::try_from("acct").unwrap();
    let contacts = RefreshedUse::Contacts;

    app.note_use_refreshed(&id, contacts, Some(false));
    app.note_use_refreshed(&id, contacts, Some(true));
    app.note_use_refreshed(&id, contacts, Some(false));
    assert_eq!(app.use_problems(&id), UseProblems::default());

    app.note_use_problems(&id, UseProblems::default());
    app.note_use_refreshed(&id, contacts, Some(false));
    assert_eq!(app.use_problems(&id), UseProblems::default());

    app.note_use_refreshed(&id, contacts, None);
    app.note_use_refreshed(&id, RefreshedUse::Calendar, Some(false));
    assert!(app.note_use_refreshed(&id, contacts, Some(false)));
    assert_eq!(
        app.use_problems(&id),
        UseProblems {
            calendar: None,
            contacts: Some(UseProblem::Unreachable),
        },
        "each use counts its own refreshes"
    );
}
