//! Where a meeting may be organised: only on a calendar whose server sends the invitations, and
//! only as an address that server schedules as. Anywhere else the meeting would be stored and
//! nobody on it told.

use std::sync::{Arc, Mutex};

use engine_api::{
    CalendarAddress, CalendarDate, CalendarUserAddresses, Invitee, InviteePatch, Participant,
    ParticipantRole, SchedulingIdentity,
};
use mailcal_account::InviteeEditability;

use super::{
    CalendarWriteStatus, EventIntent, Intent,
    fakes::{CalendarFake, calendar_account, calendar_app, evt, stored_event},
};

fn guest() -> Invitee {
    Invitee::required(SchedulingIdentity::new(
        CalendarAddress::parse("guest@example.com").unwrap(),
    ))
}

fn create_meeting() -> Intent {
    Intent::Events(EventIntent::Create {
        title: "Planning".to_owned(),
        start: "2026-09-14T09:00:00Z".to_owned(),
        end: "2026-09-14T10:00:00Z".to_owned(),
        account: Some("acct-a".to_owned()),
        calendar: None,
        all_day: false,
        timezone: None,
        notes: None,
        location: None,
        recurrence: None,
        invitees: Some(vec![guest()]),
    })
}

fn organised_by_me() -> engine_core::calendar::Event {
    let mut event = stored_event("planning", "\"v7\"");
    let mut organiser = Participant::attendee("me@acct-a.local");
    organiser.roles.insert(ParticipantRole::Owner);
    event.participants.push(organiser);
    event
}

#[tokio::test]
async fn a_calendar_row_offers_invitees_only_where_the_server_schedules() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(
        vec![
            calendar_account("schedules", CalendarFake::scheduling(Vec::new())),
            calendar_account("stores", CalendarFake::with_events(Vec::new())),
            calendar_account("reads", CalendarFake::read_only(Vec::new())),
        ],
        &surfaces,
    );
    app.dispatch(Intent::RefreshCalendar).await;

    let page = app.calendar_range(
        app.week_start_date(CalendarDate::new(2026, 9, 14).unwrap()),
        7,
    );
    let can_invite = |account: &str| {
        page.calendars
            .iter()
            .find(|row| row.account == account)
            .expect("the account's calendar row")
            .can_invite
    };
    assert!(can_invite("schedules"));
    assert!(
        !can_invite("stores"),
        "a server that stores a meeting without sending it"
    );
    assert!(!can_invite("reads"), "a calendar that cannot be written");
}

#[tokio::test]
async fn a_meeting_on_a_calendar_that_does_not_schedule_is_refused_before_the_provider() {
    let provider = CalendarFake::with_events(Vec::new());
    let meetings = provider.create_meetings();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(vec![calendar_account("acct-a", provider)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;

    app.dispatch(create_meeting()).await;

    assert!(meetings.lock().unwrap().is_empty());
    assert_eq!(app.calendar_write_status(), CalendarWriteStatus::NotSaved);
}

#[tokio::test]
async fn an_organiser_the_server_does_not_schedule_as_is_refused_before_the_provider() {
    for users in [
        CalendarUserAddresses::Known(vec!["someone-else@acct-a.local".to_owned()]),
        CalendarUserAddresses::NotEnabled,
    ] {
        let provider = CalendarFake::scheduling_as(Vec::new(), users);
        let meetings = provider.create_meetings();
        let surfaces = Arc::new(Mutex::new(Vec::new()));
        let app = calendar_app(vec![calendar_account("acct-a", provider)], &surfaces);
        app.dispatch(Intent::RefreshCalendar).await;

        app.dispatch(create_meeting()).await;

        assert!(meetings.lock().unwrap().is_empty());
        assert_eq!(app.calendar_write_status(), CalendarWriteStatus::NotSaved);
    }
}

#[tokio::test]
async fn an_organiser_the_server_schedules_as_is_written() {
    let provider = CalendarFake::scheduling_as(
        Vec::new(),
        CalendarUserAddresses::Known(vec!["ME@acct-a.local".to_owned()]),
    );
    let meetings = provider.create_meetings();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(vec![calendar_account("acct-a", provider)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;

    app.dispatch(create_meeting()).await;

    assert_eq!(meetings.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn an_organiser_a_partial_set_does_not_name_is_written() {
    // Graph and Google name the primary address alone and schedule as the aliases too.
    let provider = CalendarFake::scheduling_as(
        Vec::new(),
        CalendarUserAddresses::Including(vec!["someone-else@acct-a.local".to_owned()]),
    );
    let meetings = provider.create_meetings();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(vec![calendar_account("acct-a", provider)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;

    app.dispatch(create_meeting()).await;

    assert_eq!(meetings.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn an_appointment_needs_no_scheduling_server() {
    let provider = CalendarFake::with_events(Vec::new());
    let meetings = provider.create_meetings();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(vec![calendar_account("acct-a", provider)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;

    let Intent::Events(EventIntent::Create {
        title,
        start,
        end,
        account,
        calendar,
        all_day,
        timezone,
        notes,
        location,
        recurrence,
        ..
    }) = create_meeting()
    else {
        unreachable!()
    };
    app.dispatch(Intent::Events(EventIntent::Create {
        title,
        start,
        end,
        account,
        calendar,
        all_day,
        timezone,
        notes,
        location,
        recurrence,
        invitees: None,
    }))
    .await;

    assert_eq!(meetings.lock().unwrap().as_slice(), [None]);
}

#[tokio::test]
async fn the_roster_is_read_only_where_the_server_does_not_schedule() {
    let provider = CalendarFake::with_events(vec![organised_by_me()]);
    let patches = provider.patches();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(vec![calendar_account("acct-a", provider)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;

    let detail = app
        .event_detail(&evt("acct-a", "planning"), None)
        .await
        .expect("the meeting has detail");
    assert_eq!(detail.invitee_editability, InviteeEditability::ReadOnly);

    app.dispatch(Intent::Events(EventIntent::Update {
        event: evt("acct-a", "planning"),
        edit: mailcal_account::EventEdit {
            invitees: Some(InviteePatch::new().upsert(guest())),
            ..mailcal_account::EventEdit::default()
        },
    }))
    .await;

    assert!(patches.lock().unwrap().is_empty());
    assert_eq!(app.calendar_write_status(), CalendarWriteStatus::NotSaved);
}

#[tokio::test]
async fn a_roster_change_as_an_organiser_the_server_does_not_schedule_as_is_refused() {
    let provider = CalendarFake::scheduling_as(
        vec![organised_by_me()],
        CalendarUserAddresses::Known(vec!["someone-else@acct-a.local".to_owned()]),
    );
    let patches = provider.patches();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(vec![calendar_account("acct-a", provider)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;

    app.dispatch(Intent::Events(EventIntent::Update {
        event: evt("acct-a", "planning"),
        edit: mailcal_account::EventEdit {
            invitees: Some(InviteePatch::new().upsert(guest())),
            ..mailcal_account::EventEdit::default()
        },
    }))
    .await;

    assert!(patches.lock().unwrap().is_empty());
    assert_eq!(app.calendar_write_status(), CalendarWriteStatus::NotSaved);
}
