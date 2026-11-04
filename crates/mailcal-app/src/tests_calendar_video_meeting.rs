//! The editor's video meeting, end to end through the app (`docs/conferencing.md`, rules 9 to
//! 13): what a calendar offers, the Jitsi server it depends on, and the write a choice makes.

use std::sync::{Arc, Mutex};

use engine_api::{
    ConferenceEdit, ConferenceKey, ConferenceOrigin, ConferenceService, ConferenceText,
    ConferenceUri, EntryPoint, NativeState, NewConference,
};
use mailcal_account::{EventEdit, MeetingChange, MeetingChoice};

use super::fakes::{CalendarFake, calendar_account, calendar_app, event_from_today, evt};
use crate::{
    CalendarWriteStatus, EventIntent, Intent, calendar_ops::video_meeting::JitsiServerRefused,
};

const TEAMS: &str = "https://teams.microsoft.com/l/meetup-join/19%3ameeting_x%40thread.v2/0";

fn words() -> ConferenceText {
    ConferenceText {
        meeting: "Video meeting".into(),
        join: "Join".into(),
        meeting_id: "Meeting ID".into(),
        passcode: "Passcode".into(),
        dial_in: "Dial in".into(),
        dial_in_code: "Conference ID".into(),
    }
}

fn set(choice: MeetingChoice) -> MeetingChange {
    MeetingChange::Set {
        choice,
        words: Box::new(words()),
    }
}

fn create(meeting: Option<MeetingChange>) -> Intent {
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
        meeting,
    })
}

/// An event on a Microsoft calendar carrying the Teams meeting its server made.
fn teams_event() -> engine_api::Event {
    let mut event = event_from_today("call", 1, 9, 30);
    event.conferences = vec![engine_api::Conference {
        key: ConferenceKey::new("onlineMeeting"),
        service: Some(ConferenceService::Teams),
        origin: ConferenceOrigin::Native {
            removable: true,
            state: NativeState::Ready,
        },
        entry_points: vec![EntryPoint::at(ConferenceUri::parse(TEAMS).unwrap())],
        label: None,
        meeting_id: None,
        notes: None,
    }];
    event
}

fn ids(app: &crate::App<CalendarFake>) -> Vec<String> {
    let page = app.calendar_range(super::today(), 7);
    page.calendars
        .iter()
        .flat_map(|row| row.meeting_choices.iter().map(|choice| choice.id.clone()))
        .collect()
}

#[tokio::test]
async fn a_calendar_offers_its_servers_meeting_and_jitsi_once_a_server_is_set() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let fake = CalendarFake::writing_meetings(vec![], &[ConferenceService::Teams]);
    let app = calendar_app(vec![calendar_account("acct", fake)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;
    assert_eq!(ids(&app), ["server:teams"]);

    assert_eq!(
        app.set_jitsi_server(Some("http://meet.example.org")),
        Err(JitsiServerRefused)
    );
    assert_eq!(app.jitsi_server(), None);
    app.set_jitsi_server(Some(" https://meet.example.org/ "))
        .unwrap();
    assert_eq!(
        app.jitsi_server().as_deref(),
        Some("https://meet.example.org")
    );
    assert_eq!(ids(&app), ["server:teams", "jitsi"]);
    let names: Vec<String> = app.calendar_range(super::today(), 7).calendars[0]
        .meeting_choices
        .iter()
        .map(|choice| choice.name.clone())
        .collect();
    assert_eq!(names, ["Microsoft Teams", "Jitsi Meet"]);

    app.set_jitsi_server(Some("")).unwrap();
    assert_eq!(ids(&app), ["server:teams"]);
}

#[tokio::test]
async fn a_calendar_whose_provider_writes_no_meeting_offers_none() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(
        vec![calendar_account("acct", CalendarFake::with_events(vec![]))],
        &surfaces,
    );
    app.set_jitsi_server(Some("https://meet.example.org"))
        .unwrap();
    app.dispatch(Intent::RefreshCalendar).await;
    assert_eq!(ids(&app), Vec::<String>::new());
}

#[tokio::test]
async fn a_new_event_asks_its_server_for_the_meeting_it_makes() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let fake = CalendarFake::writing_meetings(vec![], &[ConferenceService::Teams]);
    let made = fake.create_conferences();
    let app = calendar_app(vec![calendar_account("acct", fake)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;
    app.dispatch(create(Some(set(MeetingChoice::Server(
        ConferenceService::Teams,
    )))))
    .await;
    assert_eq!(
        *made.lock().unwrap(),
        [Some(NewConference::Native(ConferenceService::Teams))]
    );
}

#[tokio::test]
async fn a_new_event_gets_a_jitsi_room_with_a_secret_name_on_the_persons_server() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let fake = CalendarFake::writing_meetings(vec![], &[]);
    let made = fake.create_conferences();
    let app = calendar_app(vec![calendar_account("acct", fake)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;
    app.set_jitsi_server(Some("https://meet.example.org"))
        .unwrap();
    app.dispatch(create(Some(set(MeetingChoice::Jitsi)))).await;
    app.dispatch(create(Some(set(MeetingChoice::Jitsi)))).await;

    let made = made.lock().unwrap();
    let links: Vec<String> = made
        .iter()
        .map(|new| match new {
            Some(NewConference::Attach(attached)) => {
                assert_eq!(attached.text, words());
                attached.conference.join().unwrap().as_str().to_owned()
            }
            other => panic!("a Jitsi meeting is put on from elsewhere: {other:?}"),
        })
        .collect();
    for link in &links {
        let room = link.strip_prefix("https://meet.example.org/").unwrap();
        assert_eq!(room.len(), 26, "{room}");
        assert!(
            room.bytes()
                .all(|b| b.is_ascii_lowercase() || (b'2'..=b'7').contains(&b))
        );
    }
    assert_ne!(links[0], links[1], "each meeting has a room of its own");
}

#[tokio::test]
async fn a_meeting_the_calendar_cannot_have_is_not_saved_at_all() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let fake = CalendarFake::writing_meetings(vec![], &[]);
    let created = fake.creations();
    let app = calendar_app(vec![calendar_account("acct", fake)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;

    // No Jitsi server set.
    app.dispatch(create(Some(set(MeetingChoice::Jitsi)))).await;
    // A service this calendar's server does not make.
    app.dispatch(create(Some(set(MeetingChoice::Server(
        ConferenceService::GoogleMeet,
    )))))
    .await;

    assert!(created.lock().unwrap().is_empty());
    assert_eq!(app.calendar_write_status(), CalendarWriteStatus::NotSaved);
}

#[tokio::test]
async fn an_edit_replaces_or_takes_off_the_meeting_the_event_has() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let fake = CalendarFake::writing_meetings(vec![teams_event()], &[ConferenceService::Teams]);
    let patches = fake.patches();
    let app = calendar_app(vec![calendar_account("acct", fake)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;
    app.set_jitsi_server(Some("https://meet.example.org"))
        .unwrap();

    let detail = app.event_detail(&evt("acct", "call"), None).await.unwrap();
    assert_eq!(
        detail.editor_meeting.and_then(|meeting| meeting.choice),
        Some(MeetingChoice::Server(ConferenceService::Teams))
    );

    let edit = |meeting| EventEdit {
        meeting: Some(meeting),
        ..EventEdit::default()
    };
    app.update_event(&evt("acct", "call"), &edit(set(MeetingChoice::Jitsi)))
        .await
        .unwrap();
    app.update_event(&evt("acct", "call"), &edit(MeetingChange::Remove))
        .await
        .unwrap();

    let patches = patches.lock().unwrap();
    match &patches[0].conference {
        Some(ConferenceEdit::Set {
            replacing,
            new: NewConference::Attach(_),
        }) => assert_eq!(
            replacing.as_ref().map(ConferenceKey::as_str),
            Some("onlineMeeting")
        ),
        other => panic!("a choice replaces the meeting: {other:?}"),
    }
    assert_eq!(
        patches[1].conference,
        Some(ConferenceEdit::Remove(ConferenceKey::new("onlineMeeting")))
    );
}

#[tokio::test]
async fn an_edit_of_one_occurrence_may_not_change_the_meeting() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let fake = CalendarFake::writing_meetings(vec![teams_event()], &[ConferenceService::Teams]);
    let patches = fake.patches();
    let app = calendar_app(vec![calendar_account("acct", fake)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;
    let stored = app.stored_event(&evt("acct", "call")).await.unwrap();
    let occurrence = super::fakes::occurrence_wall_clock_of(&stored, 0);
    let edit = EventEdit {
        meeting: Some(MeetingChange::Remove),
        occurrence: Some(occurrence),
        ..EventEdit::default()
    };
    assert!(app.update_event(&evt("acct", "call"), &edit).await.is_err());
    assert!(patches.lock().unwrap().is_empty());
    assert_eq!(app.calendar_write_status(), CalendarWriteStatus::NotSaved);
}
