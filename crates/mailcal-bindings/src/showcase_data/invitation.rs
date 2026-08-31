//! The showcase dataset's **meeting invitation** — the one message that shows mail and calendar
//! working as one product.
//!
//! Everything about the invitation that decides *shape* lives here, in one file, because the
//! invitation is really three artefacts that must agree exactly or the card is nonsense:
//!
//! 1. an **inbox message** carrying an iTIP `REQUEST` ([`ics`]),
//! 2. the **calendar hold** the account's auto-scheduling server already put in the diary
//!    ([`hold`]) — same `UID`, same start, same attendees, and
//! 3. an existing **commitment it clashes with**, which the seed's own board meeting supplies.
//!
//! Only (1) and (2) sharing a `UID` makes the card read its answer off the *calendar* rather than
//! off the frozen mail (`crate::App::invitation_card`); only their starts matching makes the
//! conflict count and the day preview describe the meeting the card is showing. So both are built
//! from [`starts_at`] here, never written out twice.
//!
//! The language-dependent half — the meeting's title, room and notes — is [`InviteText`], which
//! each locale seed supplies. Everything else (weekday, time, duration, the roster, the
//! addresses) is identical in every locale, exactly as the rest of the showcase calendar is.

use engine_core::{
    calendar::{Event, Location, Participant, ParticipantRole, ParticipationStatus},
    ids::{EventId, Uid},
    membership::Memberships,
    time::{CalendarDateTime, Duration},
};
use time::OffsetDateTime;

use super::calendar::{Cal, THU, zoned_wd};

/// The invitation message's provider key, in every locale — what a host opens to reach the card.
pub(crate) const MESSAGE_KEY: &str = "p-invite";

/// The calendar hold's provider key. Its `UID` is derived from it the way every showcase event's
/// is, so [`ics`] and [`hold`] name the same meeting.
const EVENT_KEY: &str = "ev-thu-kickoff";

/// The meeting's `UID` — the identity the mail and the calendar copy share.
fn uid() -> Uid {
    Uid::new(format!("{EVENT_KEY}@allodia.local")).expect("valid uid")
}

/// How long the meeting runs.
const MINUTES: u64 = 60;

/// The meeting's start: **Thursday 14:30** of the current week, Europe/Amsterdam.
///
/// Chosen so it lands *inside* the seed's Thursday board meeting (14:00–16:00) rather than beside
/// it. A showcase invitation with a free day around it demonstrates a card; one that collides with
/// something the user has already committed to demonstrates the point of the card — the conflict
/// line reads "1 other thing", and the day preview draws the two side by side in packed columns.
/// Thursday also carries an all-day band (the team offsite), so the preview shows its all-day
/// banner too rather than only the hour grid.
pub(crate) fn starts_at(now: OffsetDateTime) -> CalendarDateTime {
    zoned_wd(now, THU, 14, 30)
}

/// The meeting's language-dependent text, supplied by each locale's seed.
///
/// Kept beside the seed's other translated strings rather than in the message bodies, because the
/// **calendar hold** needs the title and room too — and a hold titled in English under a Dutch
/// invitation is exactly the half-translated screenshot the per-locale seeds exist to prevent.
pub(crate) struct InviteText {
    /// The meeting's title (`SUMMARY`).
    pub(crate) summary: &'static str,
    /// Where it is (`LOCATION`).
    pub(crate) location: &'static str,
    /// The organiser's notes (`DESCRIPTION`), one or two sentences.
    pub(crate) description: &'static str,
}

/// One person on the invitation: display name, address, and how they have answered.
///
/// The roster is the same in every locale — these are people's names, not copy — and it is
/// deliberately mixed, so the card's tally reads "2 accepted · 1 tentative · 1 awaiting" rather
/// than exercising a single counter.
struct Invitee {
    name: &'static str,
    email: &'static str,
    status: ParticipationStatus,
    /// The organiser (iTIP `ORGANIZER`), of whom there is exactly one.
    organizer: bool,
}

/// Who is invited. Eva is the account reading the mail, so hers is the `ATTENDEE` line the RSVP
/// gate matches and the one an answer patches.
const INVITEES: [Invitee; 4] = [
    Invitee {
        name: "Sofia Ruiz",
        email: "sofia.ruiz@northwind.example",
        status: ParticipationStatus::Accepted,
        organizer: true,
    },
    Invitee {
        name: "Eva Jansen",
        email: super::REPLY_ACCOUNT,
        status: ParticipationStatus::NeedsAction,
        organizer: false,
    },
    Invitee {
        name: "Tom de Vries",
        email: "tom.devries@northwind.example",
        status: ParticipationStatus::Accepted,
        organizer: false,
    },
    Invitee {
        name: "Priya Nair",
        email: "priya.nair@northwind.example",
        status: ParticipationStatus::Tentative,
        organizer: false,
    },
];

/// The organiser's `Name <address>`, for the message's `From:` header.
pub(crate) fn organizer() -> (&'static str, &'static str) {
    let organizer = INVITEES
        .iter()
        .find(|invitee| invitee.organizer)
        .expect("the showcase invitation has an organizer");
    (organizer.name, organizer.email)
}

/// The calendar hold the account's server already scheduled — the meeting as it sits in the diary
/// while the user has not answered.
///
/// Unanswered (`NEEDS-ACTION`), which is what makes it draw as a dashed hold on the grid and in the
/// card's preview, and what keeps it *out* of its own conflict count (`crate::invitations`: two
/// unanswered holds are not yet a clash).
pub(crate) fn hold(text: &InviteText, now: OffsetDateTime) -> Event {
    let mut event = Event::new(
        EventId::try_from(EVENT_KEY).expect("valid event id"),
        uid(),
        Memberships::of_one(Cal::Work.id()),
        starts_at(now),
    );
    event.title = text.summary.to_owned();
    event.duration =
        Duration::from_parts(0, 0, 0, MINUTES, 0, 0).expect("a valid showcase duration");
    event.description = Some(text.description.to_owned());
    event.locations = vec![Location::named(text.location)];
    event.participants = INVITEES.iter().map(participant).collect();
    event
}

/// One [`Invitee`] as the engine models a participant.
fn participant(invitee: &Invitee) -> Participant {
    let mut participant = Participant::attendee(invitee.email);
    participant.name = Some(invitee.name.to_owned());
    participant.participation_status = invitee.status.clone();
    if invitee.organizer {
        participant.roles.insert(ParticipantRole::Owner);
    }
    participant
}

/// The iTIP `REQUEST` the invitation mail carries, as an iCalendar document.
///
/// Built from the same [`starts_at`] the hold is, and emitted as a zoned `TZID` wall clock rather
/// than a UTC instant — that is what a real invitation carries, and it means this text does not
/// have to know whether the current week falls in CET or CEST.
///
/// The `ATTENDEE` lines carry `PARTSTAT` and `RSVP=TRUE`, so a client that read the *mail* rather
/// than the calendar would still see the same roster; the two agreeing is the property the
/// showcase is demonstrating.
pub(crate) fn ics(text: &InviteText, now: OffsetDateTime) -> String {
    let start = starts_at(now);
    let mut out = String::from(
        "BEGIN:VCALENDAR\r\n\
         VERSION:2.0\r\n\
         PRODID:-//Northwind//Showcase//EN\r\n\
         METHOD:REQUEST\r\n\
         BEGIN:VEVENT\r\n",
    );
    out.push_str(&format!("UID:{}\r\n", uid().as_str()));
    out.push_str("SEQUENCE:0\r\n");
    out.push_str(&format!("DTSTAMP:{}\r\n", stamp(now)));
    out.push_str(&format!("{}\r\n", date_time_line("DTSTART", &start)));
    out.push_str(&format!(
        "{}\r\n",
        date_time_line("DTEND", &plus_minutes(&start, MINUTES))
    ));
    out.push_str(&format!("SUMMARY:{}\r\n", escape(text.summary)));
    out.push_str(&format!("LOCATION:{}\r\n", escape(text.location)));
    out.push_str(&format!("DESCRIPTION:{}\r\n", escape(text.description)));
    for invitee in &INVITEES {
        out.push_str(&attendee_line(invitee));
    }
    out.push_str("END:VEVENT\r\nEND:VCALENDAR\r\n");
    out
}

/// An `ORGANIZER` or `ATTENDEE` property line for one invitee.
fn attendee_line(invitee: &Invitee) -> String {
    if invitee.organizer {
        return format!(
            "ORGANIZER;CN={}:mailto:{}\r\n",
            escape(invitee.name),
            invitee.email
        );
    }
    format!(
        "ATTENDEE;CN={};ROLE=REQ-PARTICIPANT;PARTSTAT={};RSVP=TRUE:mailto:{}\r\n",
        escape(invitee.name),
        partstat(&invitee.status),
        invitee.email
    )
}

/// The iCalendar `PARTSTAT` spelling of a participation status.
fn partstat(status: &ParticipationStatus) -> &'static str {
    match status {
        ParticipationStatus::Accepted => "ACCEPTED",
        ParticipationStatus::Declined => "DECLINED",
        ParticipationStatus::Tentative => "TENTATIVE",
        ParticipationStatus::Delegated => "DELEGATED",
        _ => "NEEDS-ACTION",
    }
}

/// A `DTSTART`/`DTEND` line for a zoned showcase instant: `NAME;TZID=<zone>:YYYYMMDDThhmmss`.
///
/// Only the zoned shape is handled, because [`starts_at`] only ever produces one — an all-day or
/// UTC form here would be dead code the seed cannot reach, and the panic says so rather than
/// silently emitting a line the parser would read as a different time.
fn date_time_line(name: &str, when: &CalendarDateTime) -> String {
    let CalendarDateTime::Zoned { local, zone } = when else {
        unreachable!("the showcase invitation is always a zoned wall clock")
    };
    format!(
        "{name};TZID={}:{:04}{:02}{:02}T{:02}{:02}{:02}",
        zone.as_str(),
        local.year(),
        local.month(),
        local.day(),
        local.hour(),
        local.minute(),
        local.second(),
    )
}

/// `when` moved on by `minutes`, staying a wall clock in the same zone.
///
/// The meeting is an hour inside a working afternoon, so this never crosses midnight or a DST
/// boundary; `time`'s own arithmetic does the carrying rather than hand-rolled minute maths.
fn plus_minutes(when: &CalendarDateTime, minutes: u64) -> CalendarDateTime {
    let CalendarDateTime::Zoned { local, zone } = when else {
        unreachable!("the showcase invitation is always a zoned wall clock")
    };
    let date = time::Date::from_calendar_date(
        local.year(),
        time::Month::try_from(local.month()).expect("a valid showcase month"),
        local.day(),
    )
    .expect("a valid showcase date");
    let at = date
        .with_hms(local.hour(), local.minute(), local.second())
        .expect("a valid showcase time")
        + time::Duration::minutes(i64::try_from(minutes).expect("a small showcase duration"));
    CalendarDateTime::Zoned {
        local: engine_core::time::LocalDateTime::new(
            at.year(),
            u8::from(at.month()),
            at.day(),
            at.hour(),
            at.minute(),
            at.second(),
        )
        .expect("a valid showcase local datetime"),
        zone: zone.clone(),
    }
}

/// `now` as an iCalendar UTC `DTSTAMP`.
fn stamp(now: OffsetDateTime) -> String {
    format!(
        "{:04}{:02}{:02}T{:02}{:02}{:02}Z",
        now.year(),
        u8::from(now.month()),
        now.day(),
        now.hour(),
        now.minute(),
        now.second(),
    )
}

/// Escapes a text value for an iCalendar property (RFC 5545 §3.3.11).
///
/// The seed's own strings are tame, but a `;` in a room name or a `,` in a sentence would
/// otherwise split the property and silently truncate what the card shows.
fn escape(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace(';', "\\;")
        .replace(',', "\\,")
        .replace('\n', "\\n")
}
