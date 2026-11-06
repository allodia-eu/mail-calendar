//! A video meeting made through an account the person connected (`docs/conferencing.md`, rules
//! 14 to 16): the choice it adds to every writable calendar, the meeting made before the write,
//! and the meeting deleted again when the write fails.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use engine_api::{
    Conference, ConferenceEdit, ConferenceKey, ConferenceOrigin, ConferenceService, ConferenceText,
    ConferenceUri, EntryPoint, MeetingRequest, MeetingService, NewConference,
};
use engine_provider::{ProviderError, ProviderResult};
use mailcal_account::{EventEdit, MeetingChange, MeetingChoice};

use super::fakes::{CalendarFake, calendar_account, calendar_app, event_from_today, evt};
use crate::{Account, CalendarWriteStatus, EventIntent, Intent};

const TALK: &str = "https://cloud.example.org/index.php/call/83h5s7dz";

/// A meeting service that records what it was asked, answering with a Talk conversation, or
/// refusing every creation.
#[derive(Default)]
struct Talk {
    refuses: bool,
    made: Mutex<Vec<MeetingRequest>>,
    deleted: Mutex<Vec<String>>,
}

#[async_trait]
impl MeetingService for Talk {
    fn service(&self) -> ConferenceService {
        ConferenceService::NextcloudTalk
    }

    async fn create_meeting(&self, request: &MeetingRequest) -> ProviderResult<Conference> {
        if self.refuses {
            return Err(ProviderError::permanent("this account may not start one"));
        }
        self.made.lock().unwrap().push(request.clone());
        Ok(talk_conference(ConferenceOrigin::Attached))
    }

    async fn delete_meeting(&self, meeting: &Conference) -> ProviderResult<()> {
        let link = meeting.join().unwrap().as_str().to_owned();
        self.deleted.lock().unwrap().push(link);
        Ok(())
    }
}

fn talk_conference(origin: ConferenceOrigin) -> Conference {
    Conference {
        key: ConferenceKey::new(TALK),
        service: Some(ConferenceService::NextcloudTalk),
        origin,
        entry_points: vec![EntryPoint::at(ConferenceUri::parse(TALK).unwrap())],
        label: Some("Nextcloud Talk".to_owned()),
        meeting_id: None,
        notes: None,
    }
}

fn words() -> Box<ConferenceText> {
    Box::new(ConferenceText {
        meeting: "Video meeting".into(),
        join: "Join".into(),
        meeting_id: "Meeting ID".into(),
        passcode: "Passcode".into(),
        dial_in: "Dial in".into(),
        dial_in_code: "Conference ID".into(),
    })
}

fn through(account: &str) -> MeetingChange {
    MeetingChange::Set {
        choice: MeetingChoice::Account {
            service: ConferenceService::NextcloudTalk,
            account: account.try_into().unwrap(),
        },
        words: words(),
    }
}

/// `account` used for video meetings through `talk`.
fn meeting_account(id: &str, fake: CalendarFake, talk: &Arc<Talk>) -> Account<CalendarFake> {
    Account {
        meetings: Some(Arc::clone(talk) as Arc<dyn MeetingService>),
        ..calendar_account(id, fake)
    }
}

fn create(meeting: MeetingChange) -> Intent {
    Intent::Events(EventIntent::Create {
        title: "Kickoff".to_owned(),
        start: "2026-09-01T09:00:00Z".to_owned(),
        end: "2026-09-01T09:30:00Z".to_owned(),
        account: Some("work".to_owned()),
        calendar: None,
        all_day: false,
        timezone: None,
        notes: Some("Agenda attached".to_owned()),
        location: Some("Room 4".to_owned()),
        recurrence: None,
        invitees: None,
        meeting: Some(meeting),
    })
}

fn choices(app: &crate::App<CalendarFake>, account: &str) -> Vec<(String, Option<String>)> {
    app.calendar_range(super::today(), 7)
        .calendars
        .into_iter()
        .filter(|row| row.account == account)
        .flat_map(|row| row.meeting_choices)
        .map(|choice| (choice.id, choice.through))
        .collect()
}

#[tokio::test]
async fn an_account_used_for_meetings_offers_them_on_every_writable_calendar() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let talk = Arc::new(Talk::default());
    let cloud = meeting_account("cloud", CalendarFake::writing_meetings(vec![], &[]), &talk);
    let work = calendar_account(
        "work",
        CalendarFake::writing_meetings(vec![], &[ConferenceService::Teams]),
    );
    let app = calendar_app(vec![cloud, work], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;
    app.set_jitsi_server(Some("https://meet.example.org"))
        .unwrap();

    let via = |account: &str| Some(format!("me@{account}.local"));
    assert_eq!(
        choices(&app, "work"),
        [
            ("server:teams".to_owned(), None),
            ("account:nextcloud-talk:cloud".to_owned(), via("cloud")),
            ("jitsi".to_owned(), None),
        ],
        "the server's own first, then each account's, then Jitsi"
    );
    assert_eq!(
        choices(&app, "cloud"),
        [
            ("account:nextcloud-talk:cloud".to_owned(), via("cloud")),
            ("jitsi".to_owned(), None),
        ]
    );
}

#[tokio::test]
async fn a_service_the_calendars_server_makes_is_offered_as_the_servers_only() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let talk = Arc::new(Talk::default());
    let fake = CalendarFake::writing_meetings(vec![], &[ConferenceService::NextcloudTalk]);
    let app = calendar_app(vec![meeting_account("cloud", fake, &talk)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;
    assert_eq!(
        choices(&app, "cloud"),
        [("server:nextcloud-talk".to_owned(), None)]
    );
}

#[tokio::test]
async fn the_meeting_is_made_from_the_title_and_times_and_put_on_the_event() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let talk = Arc::new(Talk::default());
    let cloud = calendar_account("cloud", CalendarFake::writing_meetings(vec![], &[]));
    let fake = CalendarFake::writing_meetings(vec![], &[]);
    let attached = fake.create_conferences();
    let work = meeting_account("work", fake, &talk);
    let app = calendar_app(vec![work, cloud], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;
    app.dispatch(create(through("work"))).await;

    let made = talk.made.lock().unwrap();
    assert_eq!(made.len(), 1);
    assert_eq!(made[0].title, "Kickoff");
    assert_eq!(made[0].start.to_string(), "2026-09-01T09:00:00Z");
    assert_eq!(made[0].end.to_string(), "2026-09-01T09:30:00Z");
    let attached = attached.lock().unwrap();
    let Some(Some(NewConference::Attach(meeting))) = attached.first() else {
        panic!("the meeting is put on as one made elsewhere: {attached:?}");
    };
    assert_eq!(meeting.conference.join().unwrap().as_str(), TALK);
    assert!(talk.deleted.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_meeting_the_account_could_not_make_saves_nothing() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let talk = Arc::new(Talk {
        refuses: true,
        ..Talk::default()
    });
    let fake = CalendarFake::writing_meetings(vec![], &[]);
    let created = fake.creations();
    let app = calendar_app(vec![meeting_account("work", fake, &talk)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;
    app.dispatch(create(through("work"))).await;
    // An account that makes no meetings is no way to make one either.
    app.dispatch(create(through("nobody"))).await;

    assert!(created.lock().unwrap().is_empty());
    assert_eq!(app.calendar_write_status(), CalendarWriteStatus::NotSaved);
}

#[tokio::test]
async fn a_meeting_made_for_an_event_that_was_not_saved_is_deleted_again() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let talk = Arc::new(Talk::default());
    let fake =
        CalendarFake::writing_meetings(vec![event_from_today("call", 1, 9, 30)], &[]).refusing();
    let app = calendar_app(vec![meeting_account("work", fake, &talk)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;

    app.dispatch(create(through("work"))).await;
    assert_eq!(*talk.deleted.lock().unwrap(), [TALK]);

    let edit = EventEdit {
        meeting: Some(through("work")),
        ..EventEdit::default()
    };
    assert!(app.update_event(&evt("work", "call"), &edit).await.is_err());
    assert_eq!(*talk.deleted.lock().unwrap(), [TALK, TALK]);
    assert_eq!(talk.made.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn taking_a_meeting_off_leaves_it_at_its_service() {
    let mut event = event_from_today("call", 1, 9, 30);
    event.conferences = vec![talk_conference(ConferenceOrigin::Attached)];
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let talk = Arc::new(Talk::default());
    let fake = CalendarFake::writing_meetings(vec![event], &[]);
    let patches = fake.patches();
    let app = calendar_app(vec![meeting_account("work", fake, &talk)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;

    let detail = app.event_detail(&evt("work", "call"), None).await.unwrap();
    assert_eq!(
        detail.editor_meeting.and_then(|meeting| meeting.choice),
        Some(MeetingChoice::Account {
            service: ConferenceService::NextcloudTalk,
            account: "work".try_into().unwrap(),
        }),
        "the one account making Talk is the choice the editor opens on"
    );

    let edit = EventEdit {
        meeting: Some(MeetingChange::Remove),
        ..EventEdit::default()
    };
    app.update_event(&evt("work", "call"), &edit).await.unwrap();
    assert!(matches!(
        patches.lock().unwrap()[0].conference,
        Some(ConferenceEdit::Remove(_))
    ));
    assert!(talk.deleted.lock().unwrap().is_empty());
}
