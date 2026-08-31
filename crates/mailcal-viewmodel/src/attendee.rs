//! Who is on an event, and how they answered: the participant view-models two surfaces share.
//!
//! The invitation card summarises participants as counts ([`crate::AttendeeTally`]); the calendar's
//! event detail lists them ([`EventAttendee`]). Both read the same `PARTSTAT` off the same stored
//! event, so both go through [`effective_response`] here rather than each mapping it themselves.
//! That is not tidiness: the rule has a non-obvious case in it (an organiser with no answer attends
//! by definition), and a meeting that tallied "2 accepted" while its roster showed one of those two
//! as *not having answered* would be one screen contradicting another.
//!
//! # Every string here is attacker-controlled
//!
//! An attendee's display name and address came from whoever sent the invitation, so both go through
//! [`plain_text`]; control characters and bidi overrides out, whitespace collapsed, length
//! bounded, and a client renders the result as **text, never markup** (`use_markup(false)` on
//! GTK).

use std::collections::HashMap;

use engine_api::{Event, Participant, ParticipantRole, ParticipationStatus, normalize_address};

use crate::{ResponseStatus, text::plain_text};

/// The longest name or address we hand a client. Matches the invitation card's own limit; the
/// same values, off the same wire.
const TEXT_LIMIT: usize = 200;

/// One person on an event, for the detail view's attendee list.
///
/// A roster, where [`crate::AttendeeTally`] is a count: the tally answers "how is this meeting
/// going" above a message body, this answers "who is coming" on an event you opened. Built by
/// [`event_attendees`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventAttendee {
    /// The display name, or empty when the event carried none: a client falls back to
    /// [`Self::email`] rather than inventing a name here. **Attacker-controlled plain text.**
    pub name: String,
    /// The address, normalised (lowercased, `mailto:` stripped). **Attacker-controlled plain
    /// text**; normalising is not sanitising, so this has been through [`plain_text`] too.
    pub email: String,
    /// Whether this participant called the meeting (the `ORGANIZER`), so a client can say so.
    pub is_organizer: bool,
    /// How they answered, with [`effective_response`]'s organiser rule already applied.
    pub response: ResponseStatus,
}

/// A participant's answer as a surface should show it.
///
/// Two rules, both of which exist because a literal reading of `PARTSTAT` is wrong:
///
/// 1. **An unknown or vendor status reads as `NeedsAction`**; "we do not know that you answered" is
///    the honest reading, and the safe one.
/// 2. **An organiser with no answer has accepted.** Whether the organiser appears as an `ATTENDEE`
///    of their own meeting is a per-sender accident; Stalwart lists itself `PARTSTAT=ACCEPTED`,
///    Google emits only an `ORGANIZER` line, and RFC 5545 defaults a missing `PARTSTAT` to
///    `NEEDS-ACTION`. Taken literally that reports the person who *called* the meeting as not
///    having replied to it. RFC 5546 §3.2.1 has the organiser attending by definition, so the
///    absent answer is inferred, and the same meeting then reads identically whichever server sent
///    it. An organiser who explicitly **declined** keeps that answer, only the absent one is
///    inferred.
#[must_use]
pub fn effective_response(participant: &Participant) -> ResponseStatus {
    let status = match &participant.participation_status {
        ParticipationStatus::Accepted => ResponseStatus::Accepted,
        ParticipationStatus::Declined => ResponseStatus::Declined,
        ParticipationStatus::Tentative => ResponseStatus::Tentative,
        ParticipationStatus::Delegated => ResponseStatus::Delegated,
        _ => ResponseStatus::NeedsAction,
    };
    if status == ResponseStatus::NeedsAction && participant.has_role(&ParticipantRole::Owner) {
        return ResponseStatus::Accepted;
    }
    status
}

/// Everyone on `event`, organiser first, ready to render.
///
/// Three decisions, each of which a real server forces:
///
/// - **One row per address.** JSCalendar merges the `ORGANIZER` and the matching `ATTENDEE` into a
///   single participant carrying both roles, while a plain iCalendar server leaves them as two
///   lines that decode to two participants: the same split shape `organized_by_us` was written for.
///   Listing them verbatim would print the organiser twice on exactly the servers that split them,
///   and only there. Merging keeps the **explicit** answer over an absent one, so the `ATTENDEE`
///   line's `PARTSTAT=ACCEPTED` is not lost to an `ORGANIZER` line that carries none.
/// - **A participant with no address is skipped**, matching the tally: an `ATTENDEE` line with no
///   cal-address cannot be anybody, and a roster longer than the "of N" beside it is a discrepancy
///   with nothing on screen to explain it.
/// - **The organiser sorts first**, and everyone else keeps the event's own order. Who called the
///   meeting is the one piece of structure in the list, so it does not depend on where a provider
///   happened to write the line. Which participant that *is* is `organized_this`, and it is not
///   simply the `owner` role, because JMAP servers do not emit one.
#[must_use]
pub fn event_attendees(event: &Event) -> Vec<EventAttendee> {
    let mut attendees: Vec<EventAttendee> = Vec::new();
    let mut seen: HashMap<String, usize> = HashMap::new();
    let has_owner = event
        .participants
        .iter()
        .any(|participant| participant.has_role(&ParticipantRole::Owner));
    for participant in &event.participants {
        let Some(address) = participant.email.as_deref() else {
            continue;
        };
        let normalized = normalize_address(address);
        let is_organizer = organized_this(participant, has_owner);
        let response = effective_response(participant);
        if let Some(&index) = seen.get(&normalized) {
            merge_into(&mut attendees[index], participant, is_organizer, response);
            continue;
        }
        seen.insert(normalized.clone(), attendees.len());
        attendees.push(EventAttendee {
            name: participant
                .name
                .as_deref()
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map_or_else(String::new, |name| plain_text(name, TEXT_LIMIT).0),
            email: plain_text(&normalized, TEXT_LIMIT).0,
            is_organizer,
            response,
        });
    }
    // A stable sort, so "organiser first" is the *only* re-ordering: everyone else stays in the
    // order the event listed them.
    attendees.sort_by_key(|attendee| !attendee.is_organizer);
    attendees
}

/// Whether this participant is the one who **called** the meeting, for the row's "Organiser" mark.
///
/// The `owner` role is the answer wherever a server writes one: a CalDAV/SabreDAV account does.
/// But **JMAP does not**: Stalwart decodes an `ORGANIZER` line into a participant carrying
/// `chair`, alongside `required`/`optional` attendees, with no `owner` anywhere on the event.
/// Asking only for `owner` would therefore leave every JMAP meeting with nobody marked; the
/// feature silently doing nothing on a whole class of account, which is worse than not shipping it.
///
/// So: `owner` if the event has one, otherwise `chair`. That is the **same precedence**
/// `organizer_line` already uses for the invitation card, deliberately; "who called this meeting"
/// is one question and must not have two answers. It stops one step short of that function's final
/// fallback (the first participant): a card names somebody or says nothing, but a *list* would be
/// printing a guess as a fact next to four other rows.
///
/// Note this is **not** the same test as [`effective_response`]'s inference, which stays strictly
/// on `owner`: RFC 5546 §3.2.1 gives the *organiser* attendance by definition, and a `CHAIR`
/// attendee line is an ordinary answer slot that happens to be chairing. So a chair with no
/// `PARTSTAT` is marked as the organiser and still reads as unanswered, which is what the data
/// actually says.
fn organized_this(participant: &Participant, event_has_owner: bool) -> bool {
    if participant.has_role(&ParticipantRole::Owner) {
        return true;
    }
    !event_has_owner && participant.has_role(&ParticipantRole::Chair)
}

/// Folds a second line for an address we already have into the row we built for the first.
///
/// The organiser flag is sticky (either line saying so makes it so), a name fills a gap but never
/// overwrites one, and an **explicit** answer wins over an absent one: the split shape usually
/// writes the `PARTSTAT` on the `ATTENDEE` line and nothing on the `ORGANIZER` line, so taking
/// "first wins" here would throw the real answer away.
///
/// "Explicit" is asked of the **raw** `PARTSTAT`, never of the mapped [`ResponseStatus`], and that
/// is the whole subtlety: [`effective_response`] turns an organiser's *absent* answer into
/// `Accepted`, so a mapped test would let a bare `ORGANIZER` line overwrite the `DECLINED` its own
/// `ATTENDEE` line carries; silently re-accepting a meeting the user declined.
fn merge_into(
    existing: &mut EventAttendee,
    participant: &Participant,
    is_organizer: bool,
    response: ResponseStatus,
) {
    existing.is_organizer |= is_organizer;
    if let Some(name) = participant
        .name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .filter(|_| existing.name.is_empty())
    {
        existing.name = plain_text(name, TEXT_LIMIT).0;
    }
    if participant.participation_status != ParticipationStatus::NeedsAction {
        existing.response = response;
    }
}

#[cfg(test)]
mod tests;
