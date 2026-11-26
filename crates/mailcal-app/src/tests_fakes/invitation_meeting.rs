//! The meeting itself: the bytes the mailbox serves, and the objects the calendar syncs.
//!
//! Split from the provider next door (`invitation.rs`) so each stays under the 500-line limit,
//! and because they answer different questions. This file is the **fixture**; one meeting,
//! expressed twice: as the iMIP message an organiser sent, and as the event a calendar holds.
//! The provider is the *behaviour* around it. Keeping the two apart is what makes it obvious
//! that the email and the calendar copy agree only by sharing a `UID`, which is exactly the
//! seam the RSVP lookup has to bridge.

use std::time::{SystemTime, UNIX_EPOCH};

use engine_core::{
    calendar::Event,
    ids::{CalendarId, EventId, MailboxId, MessageId, Uid},
    mail::Message,
    membership::Memberships,
    raw::RawIcal,
    time::{CalendarDate, CalendarDateTime, LocalDateTime},
    version::{ETag, RevisionTokens},
};
use mailcal_viewmodel::calendar::days::date_at;

use super::{ALIAS, EVENT_KEY, MEETING_UID, MESSAGE_KEY};

/// How far ahead the meeting falls by default: inside every window the calendar reads.
pub(super) const SOON: i64 = 3;

/// How far ahead [`InvitationFake::far_ahead`](super::InvitationFake::far_ahead) puts it:
/// beyond the calendar's rolling horizon (400 days ahead, `calendar_cache`), so no expanded
/// occurrence holds it.
pub(super) const FAR_AHEAD: i64 = 600;

/// The day the meeting falls on: `days_ahead` days out, wherever "today" is.
///
/// **Deliberately not a fixed date.** The card's conflict count and its day preview are gated on
/// the calendar's rolling horizon (120 days back, `calendar_cache`), so a pinned date eventually
/// falls outside it and every assertion about the preview starts reading "we have not looked";
/// a suite that turns red on a date rather than on a change.
pub(super) fn meeting_date(days_ahead: i64) -> CalendarDate {
    let today = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("the clock is after 1970")
        .as_secs()
        / 86_400;
    date_at(i64::try_from(today).expect("a day number fits") + days_ahead)
}

/// A minimal iMIP `REQUEST` as a `multipart/alternative` body part; no
/// `Content-Disposition`, which is what makes it an invitation rather than a file.
pub(super) fn imip_source(days_ahead: i64) -> Vec<u8> {
    format!(
        "From: Boss <boss@test.local>\r\n\
         To: {ALIAS}\r\n\
         Subject: Sprint planning\r\n\
         MIME-Version: 1.0\r\n\
         Content-Type: multipart/alternative; boundary=\"bnd\"\r\n\
         \r\n\
         --bnd\r\n\
         Content-Type: text/plain; charset=utf-8\r\n\
         \r\n\
         You are invited.\r\n\
         --bnd\r\n\
         Content-Type: text/calendar; charset=utf-8; method=REQUEST\r\n\
         \r\n\
         {}\r\n\
         --bnd--\r\n",
        ical(true, days_ahead)
    )
    .into_bytes()
}

/// The same invitation addressed the way Exchange addresses one: every attendee in `To:`, in the
/// order of the `ATTENDEE` lines, and the account's own identity (`me@`) last of them. Nothing
/// else names the recipient, so `To:` says only that the user is one of three.
pub(super) fn imip_source_to_every_attendee() -> Vec<u8> {
    let attendees = ["board@test.local", "colleague@test.local", "me@test.local"];
    let line = |address: &str| {
        format!(
            "ATTENDEE;ROLE=REQ-PARTICIPANT;PARTSTAT=NEEDS-ACTION;RSVP=TRUE:mailto:{address}\r\n"
        )
    };
    String::from_utf8(imip_source(SOON))
        .expect("the invitation is ASCII")
        .replace(
            &format!("To: {ALIAS}\r\n"),
            &format!("To: {}\r\n", attendees.join(", ")),
        )
        .replace(&line(ALIAS), &attendees.map(line).concat())
        .into_bytes()
}

/// The meeting, as iCalendar. `with_method` distinguishes the transit copy (the email, which
/// carries `METHOD:REQUEST`) from the stored resource (the calendar, which must not; RFC
/// 4791 §4.1).
pub(super) fn ical(with_method: bool, days_ahead: i64) -> String {
    let method = if with_method {
        "METHOD:REQUEST\r\n"
    } else {
        ""
    };
    let date = meeting_date(days_ahead);
    let (year, month, day) = (date.year(), date.month(), date.day());
    format!(
        "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//T//EN\r\n{method}\
         BEGIN:VEVENT\r\nUID:{MEETING_UID}\r\nDTSTAMP:20260801T080000Z\r\n\
         DTSTART:{year:04}{month:02}{day:02}T090000Z\r\n\
         DTEND:{year:04}{month:02}{day:02}T100000Z\r\nSUMMARY:Sprint planning\r\n\
         ORGANIZER;CN=Boss:mailto:boss@test.local\r\n\
         ATTENDEE;CN=Boss;ROLE=CHAIR;PARTSTAT=ACCEPTED:mailto:boss@test.local\r\n\
         ATTENDEE;ROLE=REQ-PARTICIPANT;PARTSTAT=NEEDS-ACTION;RSVP=TRUE:mailto:{ALIAS}\r\n\
         END:VEVENT\r\nEND:VCALENDAR\r\n"
    )
}

/// The invitation as the mail sync hands it back.
pub(super) fn invitation_message() -> Message {
    let mut message = Message::new(
        MessageId::try_from(MESSAGE_KEY).unwrap(),
        Memberships::of_one(MailboxId::try_from("INBOX").unwrap()),
    );
    message.envelope.subject = Some("Sprint planning".to_owned());
    message
}

/// The meeting as the calendar holds it: my `PARTSTAT` still `NEEDS-ACTION`, and the raw the
/// CalDAV RSVP would rewrite.
pub(super) fn invited_event(sequence: u32, days_ahead: i64) -> Event {
    let date = meeting_date(days_ahead);
    let mut event = Event::new(
        EventId::try_from(EVENT_KEY).unwrap(),
        Uid::new(MEETING_UID).unwrap(),
        Memberships::of_one(CalendarId::try_from("/cal/").unwrap()),
        CalendarDateTime::utc(
            LocalDateTime::new(date.year(), date.month(), date.day(), 9, 0, 0).unwrap(),
        ),
    );
    event.title = "Sprint planning".to_owned();
    event.sequence = sequence;
    event.duration = "PT1H".parse().unwrap();
    event.participants = engine_ical_participants();
    event.raw_ical = Some(RawIcal::new(ical(false, days_ahead)));
    event.revisions = RevisionTokens::from_etag(ETag::new("\"v1\""));
    event
}

/// Boss accepted; the alias has not answered.
pub(super) fn engine_ical_participants() -> Vec<engine_core::calendar::Participant> {
    use engine_core::calendar::{Participant, ParticipantRole, ParticipationStatus};
    let mut boss = Participant::attendee("boss@test.local");
    boss.roles.insert(ParticipantRole::Owner);
    boss.participation_status = ParticipationStatus::Accepted;
    let mut me = Participant::attendee(ALIAS);
    me.participation_status = ParticipationStatus::NeedsAction;
    vec![boss, me]
}
