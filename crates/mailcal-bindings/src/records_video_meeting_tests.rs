use engine_api::ConferenceService;

use super::*;
use crate::{EventIntent, convert_events::event_intent};

fn words() -> MeetingWords {
    MeetingWords {
        meeting: "Videovergadering".into(),
        join: "Deelnemen".into(),
        meeting_id: "Vergadering-ID".into(),
        passcode: "Wachtwoordcode".into(),
        dial_in: "Inbellen".into(),
        dial_in_code: "Conferentie-ID".into(),
    }
}

#[test]
fn a_choice_reaches_the_core_with_the_clients_words() {
    let change = meeting_change(Some(MeetingChange::Set {
        choice: "server:teams".into(),
        words: words(),
    }))
    .unwrap();
    let Some(AppMeetingChange::Set { choice, words }) = change else {
        panic!("a choice is a Set: {change:?}");
    };
    assert_eq!(choice, MeetingChoice::Server(ConferenceService::Teams));
    assert_eq!(words.join, "Deelnemen");
    assert_eq!(words.dial_in_code, "Conferentie-ID");
}

#[test]
fn a_choice_no_calendar_offers_is_refused() {
    let refused = meeting_change(Some(MeetingChange::Set {
        choice: "zoom".into(),
        words: words(),
    }));
    assert!(refused.is_err());
    assert_eq!(meeting_change(None), Ok(None));
    assert_eq!(
        meeting_change(Some(MeetingChange::Remove)),
        Ok(Some(AppMeetingChange::Remove))
    );
}

#[test]
fn a_new_event_has_no_meeting_to_take_off() {
    let create = |meeting| EventIntent::Create {
        title: "Kickoff".into(),
        start: "2026-09-01T09:00:00Z".into(),
        end: "2026-09-01T09:30:00Z".into(),
        account: None,
        calendar: None,
        all_day: false,
        timezone: None,
        notes: None,
        location: None,
        recurrence: None,
        invitees: None,
        meeting,
    };
    let Ok(mailcal_app::EventIntent::Create { meeting, .. }) =
        event_intent(create(Some(MeetingChange::Remove)))
    else {
        panic!("a create converts");
    };
    assert_eq!(meeting, None);
    let Ok(mailcal_app::EventIntent::Create { meeting, .. }) =
        event_intent(create(Some(MeetingChange::Set {
            choice: "jitsi".into(),
            words: words(),
        })))
    else {
        panic!("a create converts");
    };
    assert!(matches!(
        meeting,
        Some(AppMeetingChange::Set {
            choice: MeetingChoice::Jitsi,
            ..
        })
    ));
}

#[test]
fn the_editor_opens_on_the_choice_that_made_the_meeting() {
    let shown: EditorMeeting = AppEditorMeeting {
        name: Some("Microsoft Teams".into()),
        choice: Some(MeetingChoice::Server(ConferenceService::Teams)),
    }
    .into();
    assert_eq!(shown.choice.as_deref(), Some("server:teams"));
    let pasted: EditorMeeting = AppEditorMeeting {
        name: Some("Zoom".into()),
        choice: None,
    }
    .into();
    assert_eq!(pasted.choice, None);
}
