//! One sign-in with several calendars: every calendar syncs from the one provider, a quiet
//! refresh asks only for the calendar list, and a new event is written to the calendar it was
//! created in and re-read from that calendar alone.

use std::sync::{Arc, Mutex};

use engine_core::{ids::CalendarId, membership::Memberships};

use super::{
    EventIntent, Intent,
    fakes::{CalendarFake, calendar_account, calendar_app, event_from_today},
};

/// An event from today's fixtures, filed on `calendar`.
fn on(calendar: &str, key: &str, day_offset: i64) -> engine_api::Event {
    let mut event = event_from_today(key, day_offset, 9, 30);
    event.calendars = Memberships::of_one(CalendarId::try_from(calendar).unwrap());
    event
}

fn two_calendars() -> CalendarFake {
    CalendarFake::on_dav(
        &[("home", "home-1"), ("work", "work-1")],
        vec![on("home", "evt-home", 1), on("work", "evt-work", 2)],
    )
}

#[tokio::test]
async fn every_calendar_of_one_sign_in_syncs() {
    let provider = two_calendars();
    let fetches = provider.event_fetches();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(vec![calendar_account("acct", provider)], &surfaces);

    app.dispatch(Intent::RefreshCalendar).await;

    let mut asked = fetches.lock().unwrap().clone();
    asked.sort();
    assert_eq!(asked, ["home", "work"], "both calendars were read");
    let id = engine_api::AccountId::try_from("acct").unwrap();
    let mut keys: Vec<String> = app
        .engine
        .events(&id)
        .await
        .unwrap()
        .into_iter()
        .map(|event| event.id.as_str().to_owned())
        .collect();
    keys.sort();
    assert_eq!(keys, ["evt-home", "evt-work"]);
}

#[tokio::test]
async fn a_quiet_refresh_asks_no_calendar_for_its_events() {
    let provider = two_calendars();
    let fetches = provider.event_fetches();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(vec![calendar_account("acct", provider)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;
    fetches.lock().unwrap().clear();

    app.dispatch(Intent::RefreshCalendar).await;

    assert!(
        fetches.lock().unwrap().is_empty(),
        "neither calendar's tag moved, so neither was asked"
    );
}

#[tokio::test]
async fn a_new_event_in_the_second_calendar_is_written_and_reread_there() {
    let provider = two_calendars();
    let targets = provider.create_targets();
    let fetches = provider.event_fetches();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(vec![calendar_account("acct", provider)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;
    fetches.lock().unwrap().clear();

    app.dispatch(Intent::Events(EventIntent::Create {
        title: "Planning".to_owned(),
        start: "2026-09-01T09:00:00Z".to_owned(),
        end: "2026-09-01T09:30:00Z".to_owned(),
        account: Some("acct".to_owned()),
        calendar: Some("work".to_owned()),
        all_day: false,
        timezone: None,
        notes: None,
        location: None,
        recurrence: None,
        invitees: None,
    }))
    .await;

    assert_eq!(targets.lock().unwrap().as_slice(), ["work"]);
    assert_eq!(
        fetches.lock().unwrap().as_slice(),
        ["work"],
        "the reconcile re-read the calendar written to, and no other"
    );
}

/// A create with no calendar named, as from a "New event" with the default untouched.
fn create_unnamed() -> Intent {
    Intent::Events(EventIntent::Create {
        title: "Planning".to_owned(),
        start: "2026-09-01T09:00:00Z".to_owned(),
        end: "2026-09-01T09:30:00Z".to_owned(),
        account: Some("acct".to_owned()),
        calendar: None,
        all_day: false,
        timezone: None,
        notes: None,
        location: None,
        recurrence: None,
        invitees: None,
    })
}

/// A calendar shared read-only (Google's holidays and birthdays are two) is listed beside the
/// account's own, on an account whose provider writes. It is neither offered for writing nor the
/// default, and a new event does not go to it.
#[tokio::test]
async fn a_read_only_calendar_of_a_writable_account_is_not_written_to() {
    let provider = CalendarFake::on_dav(&[("birthdays", "b-1"), ("work", "w-1")], Vec::new())
        .read_only_calendar("birthdays");
    let targets = provider.create_targets();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(vec![calendar_account("acct", provider)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;

    let rows = app.calendars();
    let row = |id: &str| rows.iter().find(|row| row.id == id).unwrap();
    assert!(!row("birthdays").can_write, "{rows:?}");
    assert!(row("work").can_write && row("work").is_default, "{rows:?}");

    app.dispatch(create_unnamed()).await;
    assert_eq!(targets.lock().unwrap().as_slice(), ["work"]);
}

/// With nothing chosen, a new event goes to the calendar the server names its default (Google's
/// primary, Graph's default), not to whichever calendar the store lists first.
#[tokio::test]
async fn the_servers_default_calendar_takes_new_events_when_nothing_is_chosen() {
    let provider = CalendarFake::on_dav(&[("family", "f-1"), ("primary", "p-1")], Vec::new())
        .server_default("primary");
    let targets = provider.create_targets();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(vec![calendar_account("acct", provider)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;

    let defaulted: Vec<String> = app
        .calendars()
        .into_iter()
        .filter(|row| row.is_default)
        .map(|row| row.id)
        .collect();
    assert_eq!(defaulted, ["primary"]);

    app.dispatch(create_unnamed()).await;
    assert_eq!(targets.lock().unwrap().as_slice(), ["primary"]);
}
