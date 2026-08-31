//! The invitation path driven over **real captures**, one table, one row per sender shape.
//!
//! Every other invitation test builds its iCalendar in Rust, which means it can only prove the
//! parser handles what the test author already believed. These eight fixtures were taken off the
//! wire from servers that actually send invitations, so a shape nobody anticipated fails here
//! rather than on a user's screen. Their provenance and what each one is *for* is in
//! [`fixtures/imip/README.md`](../tests/fixtures/imip/README.md); the short version:
//!
//! | Fixture | Captured from | The thing it pins |
//! |---|---|---|
//! | `caldav-autoschedule-request` | Stalwart's RFC 6638 auto-schedule, over the dev harness | `REQUEST` arriving as a **dispositioned attachment**, three levels of MIME nesting, a `mailto:` folded mid-token |
//! | `caldav-autoschedule-cancel` | the same, after the organiser deleted the event | `CANCEL` in that same shape |
//! | `gmail-request` | Google Calendar, sent to a live mailbox | the iMIP body part **and** the duplicate `invite.ics`, `VTIMEZONE`-relative times, a `mailto:` folded mid-token |
//! | `gmail-cancel` | the same, after the organiser cancelled | `CANCEL` in Google's shape |
//! | `outlook-request` | Outlook.com (Exchange), sent to a live mailbox | a **Windows** `TZID` with spaces, a **Windows-1252** subject, an `OPT-PARTICIPANT`, **no** attachment beside the card |
//! | `outlook-cancel` | the same, after the organiser cancelled | `CANCEL` in Exchange's shape, with a rewritten `SUMMARY` |
//! | `exchange-internal-request` | one M365 tenant, organiser and invitee both inside it | a `REQUEST` with **no delivery header at all**; `To:` is the only thing that identifies the recipient |
//! | `publish-attachment` | hand-written (no server emits one on request) | `PUBLISH` with **no** `ATTENDEE`: the case that must produce no card |
//!
//! **The account's identity is `me@test.local` and matches none of them.** Every fixture is
//! recognised through its own recipient headers instead (§4 source 2), so these rows also prove
//! the zero-configuration alias path against real headers rather than a constructed `To:`.
//! `exchange-internal-request` is the one that pins the *bottom* of that fallback chain:
//! Exchange writes no `Delivered-To`, `X-Original-To` or `Envelope-To` on mail that never
//! leaves the tenant, so a reader that stopped at the MTA headers would tell every invitee in
//! every M365 company that they are not invited to their own meeting.

use fakes::{InvitationFake, MESSAGE_KEY, invitation_app};
use mailcal_viewmodel::{InvitationKind, ResponseStatus};

use super::MessageRef;

#[allow(clippy::duplicate_mod)]
#[path = "tests_fakes.rs"]
mod fakes;

const CALDAV_REQUEST: &[u8] =
    include_bytes!("../tests/fixtures/imip/caldav-autoschedule-request.eml");
const CALDAV_CANCEL: &[u8] =
    include_bytes!("../tests/fixtures/imip/caldav-autoschedule-cancel.eml");
const GMAIL_REQUEST: &[u8] = include_bytes!("../tests/fixtures/imip/gmail-request.eml");
const GMAIL_CANCEL: &[u8] = include_bytes!("../tests/fixtures/imip/gmail-cancel.eml");
const OUTLOOK_REQUEST: &[u8] = include_bytes!("../tests/fixtures/imip/outlook-request.eml");
const OUTLOOK_CANCEL: &[u8] = include_bytes!("../tests/fixtures/imip/outlook-cancel.eml");
const EXCHANGE_INTERNAL: &[u8] =
    include_bytes!("../tests/fixtures/imip/exchange-internal-request.eml");
const PUBLISH: &[u8] = include_bytes!("../tests/fixtures/imip/publish-attachment.eml");

/// What one fixture must produce. `None` for `card` means the RSVP gate rejected it.
struct Expected {
    /// The fixture's file name, so a failure names the file rather than an index.
    name: &'static str,
    raw: &'static [u8],
    card: Option<Card>,
    /// Attachment rows the reading view still shows. The iMIP payload is consumed into the
    /// card and must not appear; a file the sender *attached* must.
    attachments: &'static [&'static str],
}

struct Card {
    kind: InvitationKind,
    organizer: &'static str,
    summary: &'static str,
    location: &'static str,
    /// The instant the meeting starts, resolved to UTC by the core.
    starts_at: &'static str,
    /// Total attendees, and how many have yet to answer.
    attendees: (u32, u32),
    my_response: ResponseStatus,
}

fn table() -> Vec<Expected> {
    vec![
        Expected {
            name: "caldav-autoschedule-request.eml",
            raw: CALDAV_REQUEST,
            card: Some(Card {
                kind: InvitationKind::Rsvp,
                // Folded mid-token across a continuation line as `mai` / ` lto:bob@…`. An
                // unfolding bug reads this as `to:bob@test.local`: a plausible-looking
                // address that matches no one.
                organizer: "Bob Tester <bob@test.local>",
                summary: "Quarterly planning",
                location: "Meeting room 2",
                starts_at: "2026-08-03T10:00:00Z",
                attendees: (2, 1),
                my_response: ResponseStatus::NeedsAction,
            }),
            // Stalwart dispositions its payload `attachment; filename="event.ics"`, so the
            // sender did attach a file and the chip stays: the same reading as Gmail's
            // duplicate. The inline logo is a `multipart/related` body part, not a file.
            attachments: &["event.ics"],
        },
        Expected {
            name: "caldav-autoschedule-cancel.eml",
            raw: CALDAV_CANCEL,
            card: Some(Card {
                kind: InvitationKind::Cancelled,
                organizer: "Bob Tester <bob@test.local>",
                summary: "Quarterly planning",
                location: "Meeting room 2",
                starts_at: "2026-08-03T10:00:00Z",
                attendees: (2, 1),
                my_response: ResponseStatus::NeedsAction,
            }),
            attachments: &["event.ics"],
        },
        Expected {
            name: "gmail-request.eml",
            raw: GMAIL_REQUEST,
            card: Some(Card {
                kind: InvitationKind::Rsvp,
                organizer: "Bob Tester <bob.tester@test.local>",
                summary: "Quarterly planning",
                location: "Meeting room 2",
                // Written `DTSTART;TZID=Europe/Amsterdam:20260804T090000`: a wall clock plus a
                // zone, not an instant. 09:00 CEST is 07:00 UTC; a fixture that stored an
                // instant could not tell a correct resolution from a dropped `TZID`.
                starts_at: "2026-08-04T07:00:00Z",
                attendees: (2, 1),
                my_response: ResponseStatus::NeedsAction,
            }),
            // Gmail is the belt-and-braces case: the invitation is an alternative body part
            // (hidden) *and* a duplicate `application/ics` the sender attached (shown).
            attachments: &["invite.ics"],
        },
        Expected {
            name: "gmail-cancel.eml",
            raw: GMAIL_CANCEL,
            card: Some(Card {
                kind: InvitationKind::Cancelled,
                organizer: "Bob Tester <bob.tester@test.local>",
                summary: "Quarterly planning",
                location: "Meeting room 2",
                starts_at: "2026-08-04T07:00:00Z",
                attendees: (2, 1),
                my_response: ResponseStatus::NeedsAction,
            }),
            attachments: &["invite.ics"],
        },
        Expected {
            name: "outlook-request.eml",
            raw: OUTLOOK_REQUEST,
            card: Some(Card {
                kind: InvitationKind::Rsvp,
                organizer: "Bob Tester <bob.tester@test.local>",
                // The subject is RFC 2047 in **Windows-1252** (`=?Windows-1252?Q?…=97…?=`),
                // not UTF-8; Exchange still reaches for a legacy charset. The em dash here
                // comes from the base64 UTF-8 calendar part, so the two disagree on encoding
                // within one message and both have to be read correctly.
                summary: "Kwartaaloverleg — Q3",
                location: "Vergaderzaal 2",
                // `DTSTART;TZID=W. Europe Standard Time:20260812T140000`: a Windows zone id,
                // not IANA, unquoted and containing spaces and a full stop. There is no
                // `Europe/…` to look up: the offset can only come from the message's own
                // `VTIMEZONE` (whose `STANDARD`/`DAYLIGHT` parts start in the year **1601**).
                // August is CEST, so 14:00 local is 12:00Z: a fixture that stored an instant
                // could not tell a resolved zone from an ignored one.
                starts_at: "2026-08-12T12:00:00Z",
                // Three participants: the organiser plus a REQ- and an OPT-PARTICIPANT.
                // Exchange marks both invitees `NEEDS-ACTION`, and the organiser; named only
                // in `ORGANIZER`; attends by definition (RFC 5546 §3.2.1).
                attendees: (3, 2),
                my_response: ResponseStatus::NeedsAction,
            }),
            // The only fixture with **no** attachment row: Exchange sends the invitation as a
            // `multipart/alternative` body part and attaches nothing beside it. The iMIP part
            // is consumed into the card, so a paperclip here would be an invented file.
            attachments: &[],
        },
        Expected {
            name: "outlook-cancel.eml",
            raw: OUTLOOK_CANCEL,
            card: Some(Card {
                kind: InvitationKind::Cancelled,
                organizer: "Bob Tester <bob.tester@test.local>",
                // Exchange rewrites the `SUMMARY` on cancellation rather than relying on
                // `METHOD` alone, and in the *mailbox's* language, not the organiser's.
                summary: "Geannuleerd: Kwartaaloverleg — Q3",
                location: "Vergaderzaal 2",
                starts_at: "2026-08-12T12:00:00Z",
                attendees: (3, 2),
                my_response: ResponseStatus::NeedsAction,
            }),
            attachments: &[],
        },
        Expected {
            name: "exchange-internal-request.eml",
            raw: EXCHANGE_INTERNAL,
            card: Some(Card {
                kind: InvitationKind::Rsvp,
                organizer: "Bob Tester <bob.tester@test.local>",
                summary: "Allodia iMIP intern — fixture",
                location: "Vergaderzaal 2",
                // The same Windows `TZID` as `outlook-request`, on a different date: 14:00 CEST
                // is 12:00Z. Kept as a wall clock so this row also fails if the `VTIMEZONE` is
                // ignored, rather than trusting the sibling fixture to have caught it.
                starts_at: "2026-08-19T12:00:00Z",
                // Two participants: the organiser, who attends by definition (RFC 5546
                // §3.2.1), and the one invitee, who has yet to answer.
                attendees: (2, 1),
                my_response: ResponseStatus::NeedsAction,
            }),
            // `multipart/alternative` with no `text/html` part and nothing attached: inside a
            // tenant Exchange does not bother rendering the HTML alternative it sends outward.
            attachments: &[],
        },
        Expected {
            name: "publish-attachment.eml",
            raw: PUBLISH,
            // Fails both halves of the gate: `PUBLISH` expects no reply, and there is no
            // `ATTENDEE` to be. Either alone is enough; this fixture is the one that would
            // catch a gate rewritten to check only the attendee.
            card: None,
            attachments: &["agm.ics"],
        },
    ]
}

fn invite() -> MessageRef {
    MessageRef {
        account: engine_api::AccountId::try_from("acct-a").unwrap(),
        key: engine_api::ProviderKey::new(MESSAGE_KEY).unwrap(),
    }
}

#[tokio::test]
async fn every_captured_sender_shape_reads_the_same_way() {
    for case in table() {
        let surfaces = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let app = invitation_app(InvitationFake::new().with_source(case.raw), &surfaces);
        app.dispatch(super::Intent::RefreshMail).await;
        let snapshot = app.fetch_reading(invite()).await;

        let rows: Vec<&str> = snapshot
            .attachments
            .iter()
            .map(|a| a.file_name.as_str())
            .collect();
        assert_eq!(
            rows, case.attachments,
            "{}: wrong attachment rows: an iMIP body part shown as a file puts a paperclip on \
             an invitation nobody sent, and hiding a file the sender attached loses it entirely",
            case.name
        );

        let Some(expected) = case.card else {
            assert!(
                snapshot.invitation.is_none(),
                "{}: the RSVP gate must reject this, and it did not",
                case.name
            );
            continue;
        };

        let card = snapshot
            .invitation
            .unwrap_or_else(|| panic!("{}: no invitation card", case.name));
        assert_eq!(card.kind, expected.kind, "{}: kind", case.name);
        assert_eq!(
            card.organizer, expected.organizer,
            "{}: organizer",
            case.name
        );
        assert_eq!(card.summary, expected.summary, "{}: summary", case.name);
        assert_eq!(card.location, expected.location, "{}: location", case.name);
        assert_eq!(
            card.starts_at, expected.starts_at,
            "{}: starts_at",
            case.name
        );
        assert_eq!(
            (card.attendees.total, card.attendees.needs_action),
            expected.attendees,
            "{}: attendee tally",
            case.name
        );
        assert_eq!(
            card.my_response, expected.my_response,
            "{}: my own answer",
            case.name
        );
    }
}

#[tokio::test]
async fn a_request_can_be_answered_and_a_cancellation_cannot() {
    // The two `METHOD`s differ in exactly one affordance, and getting it backwards is a card
    // offering to accept a meeting that no longer exists.
    for (name, raw, expected) in [
        ("caldav-autoschedule-request.eml", CALDAV_REQUEST, true),
        ("caldav-autoschedule-cancel.eml", CALDAV_CANCEL, false),
        ("gmail-request.eml", GMAIL_REQUEST, true),
        ("gmail-cancel.eml", GMAIL_CANCEL, false),
        ("outlook-request.eml", OUTLOOK_REQUEST, true),
        ("outlook-cancel.eml", OUTLOOK_CANCEL, false),
        ("exchange-internal-request.eml", EXCHANGE_INTERNAL, true),
    ] {
        let surfaces = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let app = invitation_app(InvitationFake::new().with_source(raw), &surfaces);
        app.dispatch(super::Intent::RefreshMail).await;
        let card = app
            .fetch_reading(invite())
            .await
            .invitation
            .unwrap_or_else(|| panic!("{name}: no invitation card"));
        assert_eq!(card.can_respond, expected, "{name}: can_respond");
    }
}

#[tokio::test]
async fn the_attendee_is_matched_through_the_delivery_headers_alone() {
    // No capture names `me@test.local` anywhere. `caldav-…` was delivered to `alice@`,
    // Gmail's to a `+invite` alias; both recognised because the message *arrived here*,
    // with nothing configured. This is the case a user hits first and can least diagnose:
    // without it the card silently says "you are not invited to this".
    //
    // `outlook-request` is the sharpest of the three: its `To:` names the *required*
    // attendee, but it was delivered to the **optional** one's alias, and `Delivered-To`
    // is the only header that says so. Matching on `To:` would answer as the wrong person.
    for (name, raw) in [
        ("caldav-autoschedule-request.eml", CALDAV_REQUEST),
        ("gmail-request.eml", GMAIL_REQUEST),
        ("outlook-request.eml", OUTLOOK_REQUEST),
    ] {
        let surfaces = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let app = invitation_app(InvitationFake::new().with_source(raw), &surfaces);
        app.dispatch(super::Intent::RefreshMail).await;
        let snapshot = app.fetch_reading(invite()).await;
        assert!(
            snapshot.invitation.is_some_and(|card| card.can_respond),
            "{name}: the attendee match fell through to the account identity"
        );
    }
}

#[tokio::test]
async fn an_invitation_with_no_delivery_header_is_still_matched_through_to() {
    // Exchange writes **no** `Delivered-To`, `X-Original-To` or `Envelope-To` on mail that
    // never leaves the tenant; there was no MTA hop to write one. `To:` is the only header
    // that names the recipient, and `engine_mime::extract_delivery_recipients` falls through
    // to it for exactly this reason.
    //
    // This is the bottom of that fallback chain, and it is the *common* case rather than an
    // exotic one: colleagues in one M365 company inviting each other is most corporate
    // meeting mail. A reader that stopped at the MTA headers would show every one of those
    // invitations as somebody else's, with the buttons disabled and no way to tell why, and
    // the other three captures could not catch it, because all three carry a delivery header.
    let surfaces = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let app = invitation_app(
        InvitationFake::new().with_source(EXCHANGE_INTERNAL),
        &surfaces,
    );
    app.dispatch(super::Intent::RefreshMail).await;
    let snapshot = app.fetch_reading(invite()).await;

    assert!(
        !EXCHANGE_INTERNAL
            .windows(b"Delivered-To".len())
            .any(|w| w.eq_ignore_ascii_case(b"Delivered-To")),
        "the fixture grew a delivery header, so it no longer pins the `To:` fallback"
    );
    assert!(
        snapshot.invitation.is_some_and(|card| card.can_respond),
        "an invitation delivered inside one Exchange tenant was read as somebody else's"
    );
}
