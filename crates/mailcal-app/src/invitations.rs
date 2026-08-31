//! Turning an inbound iTIP message into an [`InvitationCard`]: the **pure** half.
//!
//! Everything here is a function of its arguments: no engine, no store, no I/O. That is
//! deliberate. The RSVP gate is the contract this feature is judged on
//! (`docs/invitations.md`), and a contract enforced inside an async method that needs a live
//! provider to exercise is a contract with no test that can fail. The impure half; reading the
//! account's calendar to count conflicts, and building the preview grid; lives in
//! [`crate::invitations_build`].
//!
//! **Privacy.** Nothing here logs. A meeting's title, its organiser and its attendee addresses
//! are exactly the content `docs/logging.md` forbids in the diagnostic log.

use engine_api::{Event, ParticipationStatus, ScheduleMethod, UtcDateTime, addresses_match};
use mailcal_viewmodel::{
    AttendeeTally, InvitationKind, ResponseStatus, effective_response, plain_text,
};

/// The longest `SUMMARY` / `LOCATION` we pass to a client.
///
/// A title is a line of UI. These are attacker-controlled, and an unbounded one is a layout
/// attack as much as a rendering one; 50 KB of combining characters in a `SUMMARY` should cost
/// a truncated row, not the reading view.
const TEXT_LIMIT: usize = 200;

/// The longest `DESCRIPTION` we pass to a client.
///
/// Gmail writes a wall of `-::~:~::~:~:~:~:~:~:~:~:~::~:~::-` filler into every invitation
/// description; passing it through would push the message body off screen. The client says the
/// text was cut (`description_truncated`) rather than implying it ends here.
const DESCRIPTION_LIMIT: usize = 500;

/// Whether an inbound scheduling message gets a card, and which affordances it carries.
///
/// This is the **two-condition RSVP gate** of `docs/invitations.md`, as a table. Both
/// conditions are load-bearing and neither is sufficient:
///
/// - **A scheduling `METHOD`.** `PUBLISH` means "informational copy, no reply expected" (RFC 5546
///   §1.4): a published `.ics` newsletter, a room booking someone shared. It gets no card at all,
///   and keeps its attachment chip, because there is no organiser waiting on an answer. This is why
///   attendee-matching alone cannot be the rule: a `PUBLISH` that happens to list your address must
///   still not offer RSVP.
/// - **An `ATTENDEE` that is one of the account's own addresses**, alias included (§4). A `REQUEST`
///   for somebody else's meeting (a forwarded invitation) shows the details and offers no response,
///   because answering it would send a reply the organiser never asked this account for.
///
/// `REPLY` (an attendee answering *us*), `COUNTER`/`DECLINECOUNTER` (proposed new times, not in
/// v1), `ADD` and `REFRESH` produce no card: each needs its own UI to mean anything, and a card
/// that showed a meeting without saying what just happened to it would mislead.
// The arms are written out one per gate row even where two share a body: this function *is* the
// contract, and collapsing `(Request, false)` with `(Cancel, false)` would hide which of the two
// conditions each row turns on. Readability of the rule beats brevity here.
#[allow(clippy::match_same_arms)]
pub(crate) fn classify(method: &ScheduleMethod, i_am_an_attendee: bool) -> Option<InvitationKind> {
    match (method, i_am_an_attendee) {
        (ScheduleMethod::Request, true) => Some(InvitationKind::Rsvp),
        (ScheduleMethod::Request, false) => Some(InvitationKind::Informational),
        (ScheduleMethod::Cancel, true) => Some(InvitationKind::Cancelled),
        (ScheduleMethod::Cancel, false) => Some(InvitationKind::Informational),
        _ => None,
    }
}

/// Downgrades an answerable invitation to [`InvitationKind::Superseded`] when the calendar
/// already holds a **newer revision of the same meeting**.
///
/// An organiser who moves a meeting re-sends the whole invitation, and both copies stay in the
/// mailbox: so the older mail keeps offering Accept / Tentative / Decline over times that are no
/// longer the meeting's. RFC 5546 §2.1.5 orders revisions of one `UID` by `SEQUENCE`, which every
/// organiser bumps on a significant change (RFC 5545 §3.8.7.4).
///
/// Only [`InvitationKind::Rsvp`] is downgraded. `Informational` and `Cancelled` offer no buttons
/// anyway, and a cancellation's own wording (*this meeting is off*) matters more to the reader
/// than the age of the mail carrying it.
///
/// # Why `SEQUENCE` alone, and not RFC 5546's `DTSTAMP` tie-break
///
/// The tie-break is defined for ordering two *scheduling messages*, and both carry a `DTSTAMP`.
/// Here the comparison is against a **stored event**, which has none: only `updated`
/// (`LAST-MODIFIED`), a different property that changes whenever *anything* touches the object.
/// Our own RSVP write touches it. So a `DTSTAMP`-versus-`LAST-MODIFIED` tie-break would mark an
/// invitation superseded moments after the user answered it, hiding the card that reports their
/// own answer.
///
/// Sequence-only is therefore deliberately conservative, and the two ways it can be wrong are not
/// symmetric: **missing** a supersession leaves a stale card exactly as it is today, while
/// **inventing** one hides a reply the organiser is still waiting for. So it claims supersession
/// only where the organiser explicitly said so by bumping the counter.
pub(crate) fn supersede(
    kind: InvitationKind,
    mail_sequence: u32,
    stored: Option<&Event>,
) -> InvitationKind {
    let outranked = stored.is_some_and(|event| event.sequence > mail_sequence);
    match kind {
        InvitationKind::Rsvp if outranked => InvitationKind::Superseded,
        other => other,
    }
}

/// Whether the day preview draws the **meeting itself**, as a hold no calendar holds yet.
///
/// The picture answers *"where would this land in my day"*, and where nothing files an invitation
/// into the calendar: a bare mailbox, or an IMAP+CalDAV account with no bridge from the mail
/// store: the one block the card is about is the one block missing. It is drawn like any other
/// unanswered hold, dotted, and **only** in the preview: nothing is written to the calendar.
///
/// **The calendar is the gate, not a capability.** `Capabilities::calendar_scheduling` answers
/// whether the server schedules what *we* write; a server can advertise RFC 6638 and still never
/// move an invitation out of a mailbox, because no RFC assigns that job to anyone
/// (`docs/invitations.md`). So the test is the one `App::store_invitation` already applies before
/// answering: does the calendar hold this meeting?
///
/// Two kinds get nothing. A **cancelled** meeting is off, and a hold for it would invent a
/// commitment the reader then has to disprove; a **superseded** one is only ever superseded
/// because the calendar holds a newer revision, which is drawn instead. Neither does a meeting
/// the user has **declined**, which every other surface hides.
pub(crate) fn proposed_hold(
    kind: InvitationKind,
    stored: Option<&Event>,
    mine: ResponseStatus,
) -> bool {
    stored.is_none()
        && matches!(kind, InvitationKind::Rsvp | InvitationKind::Informational)
        && mine != ResponseStatus::Declined
}

/// Finds the `ATTENDEE` on `event` that is this account; matching against the whole address
/// **set**, not one identity, and returns that address verbatim.
///
/// Returning the *matched* address, rather than a bool, is what makes an alias RSVP work: the
/// CalDAV write primitive patches the `PARTSTAT` of a named `ATTENDEE` line, so it has to be
/// handed the address the invitation actually used (`info@…`), not the account's primary
/// (`alice@…`). Passing the primary finds no attendee line and the RSVP fails (D4).
///
/// Comparison is the engine's `addresses_match`; case-insensitive and `mailto:`-insensitive.
/// Never `==`: iCalendar cases domains freely and writes the scheme inconsistently, and a
/// missed match here silently means "you are not invited to this".
pub(crate) fn matched_attendee(event: &Event, my_addresses: &[String]) -> Option<String> {
    event.participants.iter().find_map(|participant| {
        let attendee = participant.email.as_deref()?;
        my_addresses
            .iter()
            .any(|mine| addresses_match(attendee, mine))
            .then(|| attendee.to_owned())
    })
}

/// How an answer would actually reach the organiser on this account.
///
/// The question the card used to ask was *"can this transport express an answer?"*;
/// [`RsvpControls`](engine_api::RsvpControls) being present. That is a different question from
/// *"will anyone be told?"*, and on the very common shape of IMAP mail beside a plain CalDAV
/// calendar the two come apart completely: the `PARTSTAT` is stored perfectly and nobody
/// schedules, so the organiser waits forever while every local copy says the answer was sent.
/// That is the bug this enum exists to make unrepresentable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Delivery {
    /// The calendar server schedules (RFC 6638, or a cloud API that always does): storing the
    /// answer *is* sending it, and sending an iMIP message of our own would reach the organizer
    /// twice.
    Server,
    /// Nobody schedules, but the mail transport can carry an iTIP object with its `method=`
    /// parameter: so this core builds the `REPLY` and sends it, the way Thunderbird does.
    ClientImip,
    /// Neither route exists. The card offers no buttons; `docs/invitations.md` requires the
    /// explanation, never three controls that quietly go nowhere.
    None,
}

/// Which route an account has, from the two capabilities that decide it.
///
/// `can_store` is whether the calendar transport can express an answer at all
/// ([`RsvpControls`](engine_api::RsvpControls) being present); `server_schedules` and
/// `can_send_imip` are the engine's `calendar_scheduling` and `scheduling_submission`.
///
/// Two arms are worth reading twice, and they look alike while meaning opposite things:
///
/// - `(false, true, _)`: a server that schedules, over a calendar we cannot write an answer into,
///   delivers nothing, because the thing it schedules *on* is the write we cannot make. Sending our
///   own iMIP would be defensible, but the answer would then live only as an email while the user's
///   own diary silently disagreed. The honest report is that this account cannot answer.
/// - `(false, false, true)`: an account with **no calendar at all** (bare IMAP). Nothing disagrees
///   with anything, because there is no diary to contradict; the organiser learns the answer, which
///   is what the button is for. So this one does answer, and it is why `can_store` is not simply
///   required.
pub(crate) const fn delivery(
    can_store: bool,
    server_schedules: bool,
    can_send_imip: bool,
) -> Delivery {
    match (can_store, server_schedules, can_send_imip) {
        (true, true, _) => Delivery::Server,
        (_, false, true) => Delivery::ClientImip,
        _ => Delivery::None,
    }
}

/// This account's own answer so far, read from its matched `ATTENDEE` line.
///
/// Defaults to [`ResponseStatus::NeedsAction`]; both when the attendee carries no `PARTSTAT`
/// (RFC 5545's own default) and when we are not an attendee at all, in which case no card offers
/// a response anyway.
pub(crate) fn my_response(event: &Event, my_addresses: &[String]) -> ResponseStatus {
    event
        .participants
        .iter()
        .find(|participant| {
            participant.email.as_deref().is_some_and(|attendee| {
                my_addresses
                    .iter()
                    .any(|mine| addresses_match(attendee, mine))
            })
        })
        .map_or(ResponseStatus::NeedsAction, |participant| {
            response_status(&participant.participation_status)
        })
}

/// Maps the engine's open `ParticipationStatus` onto the card's closed [`ResponseStatus`].
///
/// An unknown or vendor status falls back to `NeedsAction`: "we do not know that you answered"
/// is the honest reading, and it is also the safe one; it offers the buttons rather than
/// claiming an answer the user never gave.
pub(crate) fn response_status(status: &ParticipationStatus) -> ResponseStatus {
    match status {
        ParticipationStatus::Accepted => ResponseStatus::Accepted,
        ParticipationStatus::Declined => ResponseStatus::Declined,
        ParticipationStatus::Tentative => ResponseStatus::Tentative,
        ParticipationStatus::Delegated => ResponseStatus::Delegated,
        _ => ResponseStatus::NeedsAction,
    }
}

/// Counts how the invitation's attendees have answered.
///
/// Participants with no address are skipped: an `ATTENDEE` line with no cal-address cannot be
/// anybody, and counting it would inflate "of N" with entries the user can never see.
///
/// **The organiser is never counted among those yet to answer.** Whether they appear as an
/// `ATTENDEE` of their own meeting is a per-sender accident; Stalwart lists itself
/// `PARTSTAT=ACCEPTED`, Google emits only an `ORGANIZER` line, and iCalendar's default for a
/// missing `PARTSTAT` is `NEEDS-ACTION`. Taken literally that reports the person who *called*
/// the meeting as not having replied to it, so a two-person Google invitation reads "0
/// accepted · 2 awaiting". RFC 5546 §3.2.1 has the organiser attending by definition, so an
/// organiser with no explicit answer is counted as one, which also makes the same meeting
/// tally identically whichever server sent it. An organiser who explicitly declined keeps
/// their answer, only the absent one is inferred.
///
/// That inference is [`mailcal_viewmodel::effective_response`], shared with the event detail's
/// attendee list rather than written twice: a meeting that tallied "2 accepted" over a roster
/// showing one of the two as unanswered would be one screen contradicting another.
pub(crate) fn tally(event: &Event) -> AttendeeTally {
    let mut tally = AttendeeTally::default();
    for participant in event.participants.iter().filter(|p| p.email.is_some()) {
        tally.total += 1;
        match effective_response(participant) {
            ResponseStatus::Accepted => tally.accepted += 1,
            ResponseStatus::Declined => tally.declined += 1,
            ResponseStatus::Tentative => tally.tentative += 1,
            // A delegated attendee has handed the decision on, so they are not waiting on
            // anybody, but they have not accepted either. Grouping them with unanswered
            // keeps `total` the sum of the four buckets.
            ResponseStatus::NeedsAction | ResponseStatus::Delegated => tally.needs_action += 1,
        }
    }
    tally
}

/// Formats the organiser for display as `Name <email>`, or bare `email` when unnamed.
///
/// Both halves are attacker-controlled, so both go through [`plain_text`]. Empty when the
/// invitation named no organiser, which a client renders as its own "unknown organiser"
/// string rather than a fabricated one here (localisation is client-side).
pub(crate) fn organizer_line(event: &Event) -> String {
    // Strictly by role, in order: the `ORGANIZER` (Owner) wins over a `ROLE=CHAIR` attendee, which
    // wins over the first participant. One `find` over "Owner **or** Chair" would return whichever
    // came first in the iCalendar's property order: so an invitation that lists a chairing
    // attendee above its organiser would name the wrong person as the organiser.
    let Some(organizer) = event
        .participants
        .iter()
        .find(|p| p.has_role(&engine_api::ParticipantRole::Owner))
        .or_else(|| {
            event
                .participants
                .iter()
                .find(|p| p.has_role(&engine_api::ParticipantRole::Chair))
        })
        .or_else(|| event.participants.first())
    else {
        return String::new();
    };
    // The address is attacker-controlled too: `normalize_address` only lowercases and strips
    // `mailto:`, so an `ORGANIZER` carrying a bidi override (or 50 KB of it) would reach a native
    // label unbounded and able to reverse the reading order of the line it sits on.
    let email = organizer
        .email
        .as_deref()
        .map(engine_api::normalize_address)
        .map(|address| plain_text(&address, TEXT_LIMIT).0)
        .unwrap_or_default();
    match organizer.name.as_deref().map(str::trim) {
        Some(name) if !name.is_empty() && !email.is_empty() => {
            format!("{} <{}>", plain_text(name, TEXT_LIMIT).0, email)
        }
        _ => email,
    }
}

/// The sanitised, truncated `SUMMARY`.
pub(crate) fn summary(event: &Event) -> String {
    plain_text(&event.title, TEXT_LIMIT).0
}

/// The sanitised, truncated first `LOCATION`, or empty when the invitation carried none.
///
/// Sabre/CalDAV invitations frequently carry an **empty** `LOCATION`, so a blank one must read
/// as "no location" rather than as a location that happens to be blank.
pub(crate) fn location(event: &Event) -> String {
    event
        .locations
        .iter()
        .find_map(|location| {
            let (text, _) = plain_text(location.name.as_deref().unwrap_or_default(), TEXT_LIMIT);
            (!text.is_empty()).then_some(text)
        })
        .unwrap_or_default()
}

/// The sanitised, truncated `DESCRIPTION` and whether it was cut short.
pub(crate) fn description(event: &Event) -> (String, bool) {
    plain_text(
        event.description.as_deref().unwrap_or_default(),
        DESCRIPTION_LIMIT,
    )
}

/// One thing already in the user's diary, reduced to what the conflict rule needs.
///
/// Deliberately not an `Event`: the rule is about *this* account's commitments over a window,
/// and stating exactly the four facts it uses keeps it a pure function with a test that can fail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DiaryEntry {
    /// The event's cross-system `UID`, for recognising the invitation's own copy.
    pub(crate) uid: String,
    /// The occurrence's absolute start.
    pub(crate) start: UtcDateTime,
    /// The occurrence's absolute end (exclusive).
    pub(crate) end: UtcDateTime,
    /// How this account has answered it.
    pub(crate) my_response: ResponseStatus,
}

/// Counts the things in `diary` that genuinely clash with a meeting over
/// `[window_start, window_end)`.
///
/// Three exclusions, each of which produces a wrong number if dropped:
///
/// 1. **The invitation's own event.** Every server in use here auto-schedules: the tentative hold
///    is already on the calendar by the time the mail is read. Counting it would report *every*
///    invitation as clashing with itself, which trains the user to ignore the number.
/// 2. **Other unanswered holds.** Two invitations nobody has answered are not yet a conflict. This
///    is not a guess about intent: an Outlook organiser says it explicitly, with
///    `X-MICROSOFT-CDO-BUSYSTATUS:TENTATIVE` alongside `INTENDEDSTATUS:BUSY`: hold it tentatively
///    until answered, busy once accepted.
/// 3. **Declined events.** The user said no; it is not a commitment, and `docs/calendar.md` already
///    hides it from the grid, so counting it would contradict what is on screen.
///
/// Overlap is half-open on both sides (`a.start < b.end && b.start < a.end`), so a meeting
/// starting exactly when another ends does **not** clash; back-to-back is the normal way a
/// diary is packed, and flagging it would make the count useless.
pub(crate) fn count_conflicts(
    invitation_uid: &str,
    window_start: UtcDateTime,
    window_end: UtcDateTime,
    diary: &[DiaryEntry],
) -> u32 {
    let overlapping = diary
        .iter()
        .filter(|entry| entry.uid != invitation_uid)
        .filter(|entry| {
            !matches!(
                entry.my_response,
                ResponseStatus::NeedsAction | ResponseStatus::Declined
            )
        })
        .filter(|entry| entry.start < window_end && window_start < entry.end)
        .count();
    u32::try_from(overlapping).unwrap_or(u32::MAX)
}

/// How this account has answered a **stored** calendar event, for the grid, the month and the
/// agenda.
///
/// Not the same question as [`my_response`], which reads an inbound invitation. Here the event is
/// already on the calendar, and four cases have to be told apart:
///
/// - **No participants at all**: the user's own appointment, something they put in their own diary.
///   That is a commitment, so it must not land in the "unanswered" bucket the conflict rule skips
///   and the grid draws dotted.
/// - **Participants, but none of them us**: a meeting on our calendar that we are not an attendee
///   of (a room booking, a colleague's event on a shared calendar). Ours to keep, so again a
///   commitment.
/// - **We called the meeting**; see [`organized_by_us`]. A commitment by definition.
/// - **We are an attendee**; read our own `PARTSTAT`.
pub(crate) fn diary_participation(event: &Event, my_addresses: &[String]) -> ResponseStatus {
    if event.participants.is_empty() || matched_attendee(event, my_addresses).is_none() {
        return ResponseStatus::Accepted;
    }
    let answer = my_response(event, my_addresses);
    // Exactly [`tally`]'s rule, applied to the diary instead of to the card: an organiser with no
    // explicit answer is counted as attending, and one who explicitly answered keeps that answer.
    if answer == ResponseStatus::NeedsAction && organized_by_us(event, my_addresses) {
        return ResponseStatus::Accepted;
    }
    answer
}

/// Whether this event is **ours to reshape**: the gate on dragging one about on the grid.
///
/// Two cases, and they are the two halves of "your own diary":
///
/// - **Nobody was invited.** An event with no participants at all is an appointment the user put in
///   their own calendar. There is no one to tell, so moving it is a private act.
/// - **We called the meeting.** An organiser moving their own meeting is the normal way a meeting
///   gets moved, and the server's scheduling layer (CalDAV auto-schedule, Graph, Google, JMAP)
///   sends the updates the attendees need.
///
/// Everything else is **someone else's**: a meeting we were invited to, a room booking, a
/// colleague's event on a shared calendar. Those we may have write access to and still must not
/// silently re-time: the right affordance there is *propose a new time*, which is iTIP
/// `COUNTER` and a feature of its own. Until it exists the block simply does not lift
/// (`docs/calendar.md` §13).
///
/// Note what this is **not**: it is not [`diary_participation`]'s question. That one collapses
/// "nobody was invited" and "invited, but not us" into the same `Accepted`, because for *drawing*
/// they are the same thing; both are commitments, neither is an unanswered hold. For *writing*
/// they could not be more different.
pub(crate) fn owns_or_organizes(event: &Event, my_addresses: &[String]) -> bool {
    event.participants.is_empty() || organized_by_us(event, my_addresses)
}

/// Whether **we** called this meeting; any `ORGANIZER` (Owner) participant at one of our own
/// addresses.
///
/// Written over every Owner participant rather than over the one [`matched_attendee`] found,
/// because the two ways a server can represent "the organiser is also attending" are both in the
/// wild: JSCalendar merges `ORGANIZER` and the matching `ATTENDEE` into **one** participant
/// carrying both roles, while a plain iCalendar server can leave them as two lines that decode to
/// two participants. Reading only the matched attendee would miss the split shape, and the
/// address is the same in both, so asking about the address answers both.
///
/// Why this case exists at all: RFC 5545's default `PARTSTAT` is `NEEDS-ACTION`, so a server that
/// writes the organiser in as an attendee without one (CalDAV routinely does) reports the person
/// who *called* the meeting as not having replied to it. Taken literally the grid draws the
/// user's own meeting as an unanswered hold, and the conflict rule, which skips unanswered;
/// then tells the next invitation the slot is free. RFC 5546 §3.2.1 has the organiser attending
/// by definition, so the answer is not "we do not know", it is "yes".
fn organized_by_us(event: &Event, my_addresses: &[String]) -> bool {
    event
        .participants
        .iter()
        .filter(|participant| participant.has_role(&engine_api::ParticipantRole::Owner))
        .filter_map(|participant| participant.email.as_deref())
        .any(|organizer| {
            my_addresses
                .iter()
                .any(|mine| addresses_match(organizer, mine))
        })
}
