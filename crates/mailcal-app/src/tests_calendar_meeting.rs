//! Calendar meeting command tests.

use std::sync::{Arc, Mutex};

use engine_api::{
    CalendarAddress, Invitee, InviteePatch, Participant, ParticipantRole, SchedulingIdentity,
};
use mailcal_account::InviteeEditability;

use super::{
    CalendarWriteStatus, Intent,
    fakes::{CalendarFake, calendar_account, calendar_app, evt, stored_event},
};

fn required(email: &str) -> Invitee {
    Invitee::required(SchedulingIdentity::new(
        CalendarAddress::parse(email).unwrap(),
    ))
}

fn create(invitees: Option<Vec<Invitee>>) -> Intent {
    Intent::CreateEvent {
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
        invitees,
    }
}

#[tokio::test]
async fn creating_a_meeting_uses_the_calendar_accounts_identity() {
    let provider = CalendarFake::with_events(Vec::new());
    let meetings = provider.create_meetings();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(vec![calendar_account("acct-a", provider)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;
    app.set_account_sender_name("acct-a", "Ada").await;

    app.dispatch(create(Some(vec![required("guest@example.com")])))
        .await;

    let meetings = meetings.lock().unwrap();
    let meeting = meetings[0].as_ref().expect("the create is a meeting");
    assert_eq!(meeting.organiser().address().as_str(), "me@acct-a.local");
    assert_eq!(meeting.organiser().name(), Some("Ada"));
    assert_eq!(
        meeting.invitees()[0].identity().address().as_str(),
        "guest@example.com"
    );
}

#[tokio::test]
async fn an_empty_roster_is_refused_before_the_provider() {
    let provider = CalendarFake::with_events(Vec::new());
    let meetings = provider.create_meetings();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(vec![calendar_account("acct-a", provider)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;

    app.dispatch(create(Some(Vec::new()))).await;

    assert!(meetings.lock().unwrap().is_empty());
    assert_eq!(app.calendar_write_status(), CalendarWriteStatus::Failed);
}

#[tokio::test]
async fn inviting_the_organiser_is_refused_before_the_provider() {
    let provider = CalendarFake::with_events(Vec::new());
    let meetings = provider.create_meetings();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(vec![calendar_account("acct-a", provider)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;

    app.dispatch(create(Some(vec![required("me@acct-a.local")])))
        .await;

    assert!(meetings.lock().unwrap().is_empty());
    assert_eq!(app.calendar_write_status(), CalendarWriteStatus::Failed);
}

#[tokio::test]
async fn an_invitee_cannot_change_the_meeting_roster() {
    let mut event = stored_event("planning", "\"v7\"");
    let mut organiser = Participant::attendee("boss@example.com");
    organiser.roles.insert(ParticipantRole::Owner);
    event.participants.push(organiser);
    event
        .participants
        .push(Participant::attendee("me@acct-a.local"));
    let provider = CalendarFake::with_events(vec![event]);
    let patches = provider.patches();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(vec![calendar_account("acct-a", provider)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;

    let detail = app
        .event_detail(&evt("acct-a", "planning"), None)
        .await
        .expect("the meeting has detail");
    assert_eq!(detail.invitee_editability, InviteeEditability::ReadOnly);

    app.dispatch(Intent::UpdateEvent {
        event: evt("acct-a", "planning"),
        edit: mailcal_account::EventEdit {
            invitees: Some(InviteePatch::new().upsert(required("new@example.com"))),
            ..mailcal_account::EventEdit::default()
        },
    })
    .await;

    assert!(patches.lock().unwrap().is_empty());
    assert_eq!(app.calendar_write_status(), CalendarWriteStatus::Failed);
}

#[tokio::test]
async fn an_organiser_can_change_the_meeting_roster() {
    let mut event = stored_event("planning", "\"v7\"");
    let mut organiser = Participant::attendee("me@acct-a.local");
    organiser.roles.insert(ParticipantRole::Owner);
    event.participants.push(organiser);
    let provider = CalendarFake::with_events(vec![event]);
    let patches = provider.patches();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(vec![calendar_account("acct-a", provider)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;

    let detail = app
        .event_detail(&evt("acct-a", "planning"), None)
        .await
        .expect("the meeting has detail");
    assert_eq!(detail.invitee_editability, InviteeEditability::Editable);

    app.dispatch(Intent::UpdateEvent {
        event: evt("acct-a", "planning"),
        edit: mailcal_account::EventEdit {
            invitees: Some(InviteePatch::new().upsert(required("new@example.com"))),
            ..mailcal_account::EventEdit::default()
        },
    })
    .await;

    let patches = patches.lock().unwrap();
    let changes = patches[0].invitees.as_ref().expect("the roster changes");
    assert_eq!(
        changes.upserts()[0].identity().address().as_str(),
        "new@example.com"
    );
}

#[tokio::test]
async fn a_jmap_chair_can_change_the_meeting_roster_when_no_owner_exists() {
    let mut event = stored_event("planning", "\"v7\"");
    let mut organiser = Participant::attendee("me@acct-a.local");
    organiser.roles.insert(ParticipantRole::Chair);
    event.participants.push(organiser);
    let provider = CalendarFake::with_events(vec![event]);
    let patches = provider.patches();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(vec![calendar_account("acct-a", provider)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;

    app.dispatch(Intent::UpdateEvent {
        event: evt("acct-a", "planning"),
        edit: mailcal_account::EventEdit {
            invitees: Some(InviteePatch::new().upsert(required("new@example.com"))),
            ..mailcal_account::EventEdit::default()
        },
    })
    .await;

    assert_eq!(patches.lock().unwrap().len(), 1);
}
