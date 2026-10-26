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
