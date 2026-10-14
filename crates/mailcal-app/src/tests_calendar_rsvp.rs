//! Answering an invitation from its event in the calendar: on both routes the invitation card
//! has, and only for an account the meeting invites and does not organise.
//!
//! The stored meeting invites `info@test.local`, an alias of the account; the calendar has no
//! message to read a `To:` header from, so the alias comes from the account's own settings.

use std::sync::{Arc, Mutex};

use engine_api::{ReplyDelivery, ScheduleMethod};
use engine_provider::RsvpResponse;
use mailcal_viewmodel::ResponseStatus;

use super::{
    CalendarWriteStatus, EventIntent, Intent,
    fakes::{
        ALIAS, CalendarFake, EVENT_KEY, InvitationFake, calendar_account, calendar_app, evt,
        invitation_app_with_prefs, stored_event,
    },
};
use crate::{App, InvitationResponse};

/// A preferences file naming the alias the stored meeting invites.
fn with_alias(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("mailcal-calendar-rsvp-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("preferences.toml");
    let mut prefs = mailcal_account::load_preferences(&path);
    prefs.set_account_aliases("acct-a", vec![ALIAS.to_owned()]);
    mailcal_account::save_preferences(&path, &prefs).unwrap();
    path
}

fn respond(response: InvitationResponse) -> Intent {
    Intent::Events(EventIntent::Respond {
        event: evt("acct-a", EVENT_KEY),
        response,
        reply_subject: Some("Maybe: Planning".to_owned()),
    })
}

async fn booted(provider: InvitationFake, name: &str) -> App<InvitationFake> {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = invitation_app_with_prefs(provider, &surfaces, Some(with_alias(name)));
    app.dispatch(Intent::RefreshMail).await;
    app.dispatch(Intent::RefreshCalendar).await;
    app
}

#[tokio::test]
async fn an_invitee_answers_from_the_calendar_where_the_server_schedules() {
    let provider = InvitationFake::new();
    let (rsvps, sends) = (provider.rsvps(), provider.sends());
    let app = booted(provider, "server").await;

    let detail = app
        .event_detail(&evt("acct-a", EVENT_KEY), None)
        .await
        .expect("the meeting is on the calendar");
    assert_eq!(detail.my_answer, Some(ResponseStatus::NeedsAction));

    app.dispatch(respond(InvitationResponse::Accept)).await;

    let rsvps = rsvps.lock().unwrap();
    assert_eq!(rsvps.len(), 1, "the answer was written once");
    assert_eq!(rsvps[0].0, ALIAS, "as the address the meeting invites");
    assert_eq!(rsvps[0].1, RsvpResponse::Accepted);
    assert!(rsvps[0].3, "and the server is asked to tell the organiser");
    assert!(sends.lock().unwrap().is_empty(), "storing it is sending it");
    assert_ne!(app.calendar_write_status(), CalendarWriteStatus::Failed);
}

#[tokio::test]
async fn where_nobody_schedules_the_answer_is_written_and_emailed_from_the_event() {
    let mut provider = InvitationFake::new().without_server_scheduling();
    provider.materialized = true;
    let (rsvps, sends) = (provider.rsvps(), provider.sends());
    let app = booted(provider, "imip").await;

    app.dispatch(respond(InvitationResponse::Tentative)).await;

    assert_eq!(rsvps.lock().unwrap()[0].1, RsvpResponse::Tentative);
    let drafts = sends.lock().unwrap();
    let draft = drafts.first().expect("the reply was emailed");
    let calendar = draft.calendar.as_ref().expect("with an iTIP part");
    assert_eq!(calendar.method, ScheduleMethod::Reply);
    assert!(calendar.ical.contains("PARTSTAT=TENTATIVE"));
    assert!(calendar.ical.contains("ORGANIZER:mailto:boss@test.local"));
    assert!(calendar.ical.contains(ALIAS));
    assert!(
        !calendar.ical.contains("RECURRENCE-ID"),
        "an answer from the calendar is for the whole meeting"
    );
    assert_eq!(draft.subject, "Maybe: Planning");
}

#[tokio::test]
async fn a_reply_the_server_could_not_deliver_is_offered_by_email_from_the_event() {
    let provider = InvitationFake::new().reporting_delivery(ReplyDelivery::Failed {
        status: "5.2".to_owned(),
    });
    let sends = provider.sends();
    let app = booted(provider, "fallback").await;

    app.dispatch(respond(InvitationResponse::Decline)).await;

    let prompt = app.reply_prompt().expect("the user is asked");
    assert_eq!(prompt.organizer, "boss@test.local");
    assert_eq!(prompt.response, InvitationResponse::Decline);
    app.dispatch(Intent::AnswerReplyPrompt {
        send: true,
        remember: false,
        reply_subject: None,
    })
    .await;
    let drafts = sends.lock().unwrap();
    let calendar = drafts[0].calendar.as_ref().expect("an iTIP part");
    assert!(calendar.ical.contains("PARTSTAT=DECLINED"));
}

#[tokio::test]
async fn an_account_with_no_way_to_tell_the_organiser_is_offered_no_answer() {
    let mut provider = InvitationFake::new().with_no_route();
    provider.materialized = true;
    let rsvps = provider.rsvps();
    let app = booted(provider, "no-route").await;

    let detail = app
        .event_detail(&evt("acct-a", EVENT_KEY), None)
        .await
        .expect("the meeting is on the calendar");
    assert_eq!(detail.my_answer, None);
    app.dispatch(respond(InvitationResponse::Accept)).await;
    assert!(rsvps.lock().unwrap().is_empty());
    assert_eq!(app.calendar_write_status(), CalendarWriteStatus::Failed);
}

#[tokio::test]
async fn the_organiser_and_an_appointment_are_offered_no_answer() {
    let mut meeting = stored_event("planning", "\"v7\"");
    let mut organiser = engine_api::Participant::attendee("me@acct-a.local");
    organiser.roles.insert(engine_api::ParticipantRole::Owner);
    meeting.participants.push(organiser);
    meeting
        .participants
        .push(engine_api::Participant::attendee("guest@example.com"));
    let appointment = stored_event("lunch", "\"v1\"");
    let provider = CalendarFake::scheduling(vec![meeting, appointment]);
    let rsvps = provider.rsvps();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(vec![calendar_account("acct-a", provider)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;

    for key in ["planning", "lunch"] {
        let detail = app.event_detail(&evt("acct-a", key), None).await.unwrap();
        assert_eq!(detail.my_answer, None, "{key}");
    }
    app.dispatch(Intent::Events(EventIntent::Respond {
        event: evt("acct-a", "planning"),
        response: InvitationResponse::Accept,
        reply_subject: None,
    }))
    .await;
    assert!(rsvps.lock().unwrap().is_empty());
    assert_eq!(app.calendar_write_status(), CalendarWriteStatus::Failed);
}
