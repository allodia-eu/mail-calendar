use engine_core::{
    calendar::{ConferenceKey, ConferenceUri, EntryPoint, NativeState},
    ids::{CalendarId, EventId, Uid},
    membership::Memberships,
    time::{CalendarDateTime, LocalDateTime, TimeZoneId},
};

use super::*;

fn event(conferences: Vec<Conference>) -> Event {
    let mut event = Event::new(
        EventId::try_from("evt-1").unwrap(),
        Uid::new("uid-1").unwrap(),
        Memberships::of_one(CalendarId::try_from("cal-1").unwrap()),
        CalendarDateTime::Zoned {
            local: "2026-06-01T09:00:00".parse::<LocalDateTime>().unwrap(),
            zone: TimeZoneId::iana("Europe/Amsterdam").unwrap(),
        },
    );
    event.conferences = conferences;
    event
}

fn conference(origin: ConferenceOrigin, link: &str) -> Conference {
    let uri = ConferenceUri::parse(link).unwrap();
    Conference {
        key: ConferenceKey::new(link),
        service: ConferenceService::recognise(&uri),
        origin,
        entry_points: vec![EntryPoint::at(uri)],
        label: None,
        meeting_id: None,
        notes: None,
    }
}

fn native() -> ConferenceOrigin {
    ConferenceOrigin::Native {
        removable: true,
        state: NativeState::Ready,
    }
}

const TEAMS: &str = "https://teams.microsoft.com/l/meetup-join/19%3ameeting_x%40thread.v2/0";

#[test]
fn a_choice_round_trips_through_its_id() {
    for choice in [
        MeetingChoice::Server(ConferenceService::Teams),
        MeetingChoice::Server(ConferenceService::GoogleMeet),
        MeetingChoice::Jitsi,
    ] {
        assert_eq!(MeetingChoice::from_id(&choice.id()), Some(choice.clone()));
    }
    assert_eq!(
        MeetingChoice::Server(ConferenceService::Teams).id(),
        "server:teams"
    );
    assert_eq!(MeetingChoice::Jitsi.id(), "jitsi");
}

#[test]
fn an_id_no_row_offers_names_no_choice() {
    for id in [
        "",
        "teams",
        "server:",
        "server:not-a-service",
        "jitsi:x",
        "Jitsi",
    ] {
        assert_eq!(MeetingChoice::from_id(id), None, "{id}");
    }
}

#[test]
fn a_choice_is_named_by_its_brand() {
    assert_eq!(
        MeetingChoice::Server(ConferenceService::Teams).name(),
        "Microsoft Teams"
    );
    assert_eq!(
        MeetingChoice::Server(ConferenceService::GoogleMeet).name(),
        "Google Meet"
    );
    assert_eq!(MeetingChoice::Jitsi.name(), "Jitsi Meet");
}

#[test]
fn an_editor_opens_on_the_meeting_the_server_made() {
    let teams = event(vec![conference(native(), TEAMS)]);
    assert_eq!(
        editor_meeting(&teams),
        Some(EditorMeeting {
            name: Some("Microsoft Teams".into()),
            choice: Some(MeetingChoice::Server(ConferenceService::Teams)),
        })
    );
}

#[test]
fn an_editor_opens_on_a_jitsi_meeting_put_on_from_elsewhere() {
    let jitsi = event(vec![conference(
        ConferenceOrigin::Attached,
        "https://meet.jit.si/k5x2q7m4n9b3v8c1z6l0p2w4r7",
    )]);
    assert_eq!(
        editor_meeting(&jitsi).and_then(|meeting| meeting.choice),
        Some(MeetingChoice::Jitsi)
    );
}

#[test]
fn a_meeting_no_choice_makes_is_named_but_not_chosen() {
    let zoom = event(vec![conference(
        ConferenceOrigin::Detected {
            place: engine_api::DetectedIn::Description,
        },
        "https://example.zoom.us/j/123456789",
    )]);
    assert_eq!(
        editor_meeting(&zoom),
        Some(EditorMeeting {
            name: Some("Zoom".into()),
            choice: None,
        })
    );
    // A Teams link pasted into the notes is not a meeting the server made.
    let pasted = event(vec![conference(
        ConferenceOrigin::Detected {
            place: engine_api::DetectedIn::Description,
        },
        TEAMS,
    )]);
    assert_eq!(editor_meeting(&pasted).and_then(|m| m.choice), None);
}

#[test]
fn a_self_hosted_link_has_no_name() {
    let unknown = event(vec![conference(
        ConferenceOrigin::Attached,
        "https://video.example.org/room",
    )]);
    assert_eq!(
        editor_meeting(&unknown),
        Some(EditorMeeting {
            name: None,
            choice: None,
        })
    );
}

#[test]
fn the_editor_works_on_the_first_shown_meeting() {
    let sip_only = Conference {
        entry_points: vec![EntryPoint::at(
            ConferenceUri::parse("sip:room@example.org").unwrap(),
        )],
        ..conference(ConferenceOrigin::Attached, "https://video.example.org/a")
    };
    let both = event(vec![sip_only, conference(native(), TEAMS)]);
    assert_eq!(current_meeting(&both).map(|c| c.key.as_str()), Some(TEAMS));
    assert_eq!(editor_meeting(&event(vec![])), None);
}
