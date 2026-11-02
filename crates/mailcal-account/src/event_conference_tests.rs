use engine_core::{
    calendar::{
        AccessCodes, ConferenceKey, ConferenceUri, DetectedIn, FlatEntry, Location, group_flat,
    },
    ids::{CalendarId, EventId, Uid},
    membership::Memberships,
    time::{CalendarDateTime, LocalDateTime, TimeZoneId},
};

use super::*;
use crate::{InviteeEditability, project_event_detail};

/// The line Google Calendar writes above and below the meeting block of an invitation.
const GOOGLE_RULE: &str = "-::~:~::~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~::~:~::-";

fn event() -> Event {
    let mut event = Event::new(
        EventId::try_from("evt-1").unwrap(),
        Uid::new("uid-1").unwrap(),
        Memberships::of_one(CalendarId::try_from("cal-1").unwrap()),
        CalendarDateTime::Zoned {
            local: "2026-06-01T09:00:00".parse::<LocalDateTime>().unwrap(),
            zone: TimeZoneId::iana("Europe/Amsterdam").unwrap(),
        },
    );
    event.duration = "PT1H".parse().unwrap();
    event
}

fn uri(value: &str) -> ConferenceUri {
    ConferenceUri::parse(value).unwrap()
}

fn entry(value: &str) -> EntryPoint {
    EntryPoint::at(uri(value))
}

fn conference(origin: ConferenceOrigin, entry_points: Vec<EntryPoint>) -> Conference {
    Conference {
        key: ConferenceKey::new("k"),
        service: entry_points
            .first()
            .and_then(|entry| ConferenceService::recognise(&entry.uri)),
        origin,
        entry_points,
        label: None,
        meeting_id: None,
        notes: None,
    }
}

#[test]
fn a_teams_meeting_offers_its_join_link_id_and_every_number() {
    let mut phone = entry("tel:+14255550100");
    phone.label = Some("+1 425-555-0100".into());
    phone.access.access_code = Some("177513992#".into());
    let mut teams = conference(
        ConferenceOrigin::Native {
            removable: false,
            state: NativeState::Ready,
        },
        vec![
            entry("https://teams.microsoft.com/l/meetup-join/19%3ameeting_x/0"),
            phone,
        ],
    );
    teams.meeting_id = Some("177513992".into());
    let mut event = event();
    event.conferences = vec![teams];
    assert_eq!(
        event_conferences(&event),
        [EventConference {
            service: Some("Microsoft Teams".into()),
            join_url: Some("https://teams.microsoft.com/l/meetup-join/19%3ameeting_x/0".into()),
            meeting_id: Some("177513992".into()),
            passcode: None,
            dial_in: vec![DialIn {
                number: "+1 425-555-0100".into(),
                uri: "tel:+14255550100".into(),
                pin: Some("177513992#".into()),
            }],
            notes: None,
            state: ConferenceState::Ready,
        }]
    );
}

#[test]
fn a_meet_conference_carries_its_codes_and_a_number_reads_as_dialled_without_what_follows() {
    let mut video = entry("https://meet.google.com/abc-defg-hij");
    video.access = AccessCodes {
        meeting_code: Some("abc-defg-hij".into()),
        passcode: Some("xyz".into()),
        ..AccessCodes::default()
    };
    let mut phone = entry("tel:+31201234567,,123456#");
    phone.label = Some("Dial in".into());
    phone.access.pin = Some("123456".into());
    let mut event = event();
    event.conferences = vec![conference(
        ConferenceOrigin::Native {
            removable: true,
            state: NativeState::Ready,
        },
        vec![video, phone],
    )];
    let meet = &event_conferences(&event)[0];
    assert_eq!(meet.service.as_deref(), Some("Google Meet"));
    assert_eq!(meet.meeting_id.as_deref(), Some("abc-defg-hij"));
    assert_eq!(meet.passcode.as_deref(), Some("xyz"));
    assert_eq!(meet.dial_in[0].number, "+31201234567");
    assert_eq!(meet.dial_in[0].pin.as_deref(), Some("123456"));
}

#[test]
fn a_meeting_being_prepared_is_shown_and_one_with_no_way_in_is_not() {
    let mut event = event();
    event.conferences = vec![
        conference(
            ConferenceOrigin::Native {
                removable: false,
                state: NativeState::Pending,
            },
            Vec::new(),
        ),
        // A plain `http` link is not one a client opens, and there is nothing else to offer.
        conference(
            ConferenceOrigin::Attached,
            vec![entry("http://meet.example.com/a")],
        ),
    ];
    let shown = event_conferences(&event);
    assert_eq!(shown.len(), 1);
    assert_eq!(shown[0].state, ConferenceState::Preparing);
    assert_eq!(shown[0].join_url, None);
}

#[test]
fn an_unrecognised_link_names_no_service() {
    let mut event = event();
    event.conferences = group_flat(vec![FlatEntry::new(uri(
        "https://video.example.com/room-12",
    ))]);
    let shown = event_conferences(&event);
    assert_eq!(shown[0].service, None);
    assert_eq!(
        shown[0].join_url.as_deref(),
        Some("https://video.example.com/room-12")
    );
}

#[test]
fn the_detail_shows_neither_the_join_link_as_a_place_nor_the_block_as_notes() {
    let mut event = event();
    event.locations = vec![
        Location::named("https://zoom.us/j/987"),
        Location::named("Room 4"),
    ];
    event.description = Some(format!(
        "Bring the figures.\n\n{GOOGLE_RULE}\nJoin Zoom Meeting\nhttps://zoom.us/j/987?pwd=x\n{GOOGLE_RULE}\n"
    ));
    let mut zoom = group_flat(vec![FlatEntry::new(uri("https://zoom.us/j/987?pwd=x"))]).remove(0);
    zoom.origin = ConferenceOrigin::Detected {
        place: DetectedIn::Location,
    };
    event.conferences = vec![zoom];
    let detail = project_event_detail("acct", &event, true, None, InviteeEditability::ReadOnly);
    assert_eq!(detail.display_location.as_deref(), Some("Room 4"));
    assert_eq!(
        detail.location.as_deref(),
        Some("https://zoom.us/j/987"),
        "the editor edits the server's location"
    );
    assert_eq!(detail.display_notes.as_deref(), Some("Bring the figures."));
    assert_eq!(
        detail.notes, event.description,
        "the editor edits the server's text"
    );
    assert_eq!(detail.conferences.len(), 1);
}

#[test]
fn the_block_of_a_meeting_the_detail_does_not_show_stays_in_the_notes() {
    let mut event = event();
    event.locations = vec![Location::named("https://zoom.us/j/987")];
    let rule = "_".repeat(80);
    let teams = format!(
        "{rule}\nMicrosoft Teams meeting\nJoin: http://teams.microsoft.com/meet/123?p=x\n{rule}"
    );
    event.description = Some(format!(
        "{GOOGLE_RULE}\nJoin Zoom Meeting\nhttps://zoom.us/j/987\n{GOOGLE_RULE}\n\n{teams}\n"
    ));
    let mut zoom = group_flat(vec![FlatEntry::new(uri("https://zoom.us/j/987"))]).remove(0);
    zoom.origin = ConferenceOrigin::Detected {
        place: DetectedIn::Location,
    };
    let mut plain = group_flat(vec![FlatEntry::new(uri(
        "http://teams.microsoft.com/meet/123?p=x",
    ))])
    .remove(0);
    plain.origin = ConferenceOrigin::Detected {
        place: DetectedIn::Description,
    };
    event.conferences = vec![zoom, plain];
    let detail = project_event_detail("acct", &event, true, None, InviteeEditability::ReadOnly);
    assert_eq!(
        detail.conferences.len(),
        1,
        "a plain http link is not offered"
    );
    assert_eq!(
        detail.display_notes.as_deref(),
        Some(teams.as_str()),
        "the Zoom block repeats the meeting shown; the Teams block is the only way into its own"
    );
}

#[test]
fn join_opens_the_https_link_when_a_plain_http_one_comes_first() {
    let mut event = event();
    event.conferences = vec![conference(
        ConferenceOrigin::Attached,
        vec![
            entry("http://meet.example.com/a"),
            entry("https://meet.example.com/a"),
        ],
    )];
    assert_eq!(
        event_conferences(&event)[0].join_url.as_deref(),
        Some("https://meet.example.com/a")
    );
}

#[test]
fn with_no_meeting_shown_the_detail_keeps_the_link_as_a_place_and_the_block_as_notes() {
    let mut event = event();
    event.locations = vec![Location::named("http://meet.example.com/a")];
    event.description = Some(format!(
        "Bring the figures.\n\n{GOOGLE_RULE}\nJoin: http://meet.example.com/a\n{GOOGLE_RULE}"
    ));
    event.conferences = vec![conference(
        ConferenceOrigin::Attached,
        vec![entry("http://meet.example.com/a")],
    )];
    let detail = project_event_detail("acct", &event, true, None, InviteeEditability::ReadOnly);
    assert_eq!(detail.conferences, []);
    assert_eq!(
        detail.display_location.as_deref(),
        Some("http://meet.example.com/a")
    );
    assert_eq!(detail.display_notes, event.description);
}
