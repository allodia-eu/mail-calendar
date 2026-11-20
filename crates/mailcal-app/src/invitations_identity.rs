//! Which `ATTENDEE` this mailbox is on an invitation that arrived as mail.
//!
//! The answer is the address the reply is sent **from** and the line its `PARTSTAT` is written
//! on, so a wrong one answers as somebody else: on a server that refuses the sender it never
//! leaves the Outbox, and on one that allows it the organiser reads a colleague's answer. An
//! organiser addresses an invitation to every attendee, which is why `To:` and `Cc:` are the last
//! source asked and the only one that must name a single attendee (`docs/invitations.md` §4).

use engine_api::{Event, InboundScheduling, ParticipantRole, addresses_match};

use crate::invitations::matched_attendee;

/// The attendee `scheduling` invited on this account, verbatim as the invitation wrote it.
///
/// Asked in the order the sources are trusted, and the first that names an attendee decides:
///
/// 1. `own`: the account's identities and aliases, and those of an account it files invitations
///    with.
/// 2. The addresses the MTA recorded delivering this copy to: each is this mailbox.
/// 3. `To:` and `Cc:`, only when exactly one attendee is among them. Several means the sender wrote
///    to several invitees and nothing says which one this mailbox is, so there is no match rather
///    than a guess.
pub(crate) fn invited_attendee(
    event: &Event,
    own: &[String],
    scheduling: &InboundScheduling,
) -> Option<String> {
    matched_attendee(event, own)
        .or_else(|| matched_attendee(event, &scheduling.delivered_to))
        .or_else(|| sole_attendee_among(event, &scheduling.addressed_to))
}

/// `own` widened by the attendee this message invited, for the rules that read "my" answer off
/// an event: the card's own response and the diary it is checked against.
pub(crate) fn addresses_with(own: &[String], attendee: Option<&str>) -> Vec<String> {
    let mut addresses = own.to_vec();
    addresses.extend(attendee.map(str::to_owned));
    addresses
}

/// The one invitee `addresses` names, or `None` when it names none or several.
///
/// The organiser is not an invitee: a `REQUEST` is never answered as the person who sent it, and
/// an organiser who copied themselves in must not make the user's own address ambiguous.
fn sole_attendee_among(event: &Event, addresses: &[String]) -> Option<String> {
    let mut found: Option<&str> = None;
    for participant in &event.participants {
        if participant.roles.contains(&ParticipantRole::Owner) {
            continue;
        }
        let Some(email) = participant.email.as_deref() else {
            continue;
        };
        if !addresses.iter().any(|named| addresses_match(email, named)) {
            continue;
        }
        match found {
            Some(earlier) if addresses_match(earlier, email) => {}
            Some(_) => return None,
            None => found = Some(email),
        }
    }
    found.map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use engine_api::ParticipationStatus;

    use super::*;
    use crate::invitations_test_support::{attendee, event_with, instant, organizer};

    fn arrived(delivered_to: &[&str], addressed_to: &[&str], event: Event) -> InboundScheduling {
        InboundScheduling {
            message: engine_api::SchedulingMessage::new(
                engine_api::ScheduleMethod::Request,
                event,
                instant("2026-07-01T08:00:00Z"),
            ),
            media_type: "text/calendar".to_owned(),
            from_inline_body: true,
            delivered_to: delivered_to.iter().map(|&a| a.to_owned()).collect(),
            addressed_to: addressed_to.iter().map(|&a| a.to_owned()).collect(),
        }
    }

    /// An Exchange meeting: the board, a colleague and the user, all in `To:`, in that order.
    fn meeting() -> Event {
        event_with(vec![
            organizer("Organiser", "organiser@test.local"),
            attendee("board@test.local", ParticipationStatus::NeedsAction),
            attendee("colleague@test.local", ParticipationStatus::NeedsAction),
            attendee("me@test.local", ParticipationStatus::NeedsAction),
        ])
    }

    const EVERYONE: &[&str] = &["board@test.local", "colleague@test.local", "me@test.local"];

    #[test]
    fn the_accounts_own_address_wins_over_the_first_attendee_in_to() {
        // The reported case: every attendee is in `To:` and the first of them is the board, so
        // a match over the whole `To:` list answered as the board and the server refused it.
        let event = meeting();
        let scheduling = arrived(&[], EVERYONE, event.clone());
        assert_eq!(
            invited_attendee(&event, &["me@test.local".to_owned()], &scheduling).as_deref(),
            Some("me@test.local")
        );
    }

    #[test]
    fn the_address_it_was_delivered_to_wins_over_to() {
        // `To:` names the required attendee; the copy was delivered to the optional one's alias.
        let event = event_with(vec![
            attendee("alice+invite@test.local", ParticipationStatus::NeedsAction),
            attendee(
                "carol+optional@test.local",
                ParticipationStatus::NeedsAction,
            ),
        ]);
        let scheduling = arrived(
            &["carol+optional@test.local"],
            &["alice+invite@test.local", "carol+optional@test.local"],
            event.clone(),
        );
        assert_eq!(
            invited_attendee(&event, &["me@test.local".to_owned()], &scheduling).as_deref(),
            Some("carol+optional@test.local")
        );
    }

    #[test]
    fn to_names_the_attendee_when_it_names_only_one() {
        // A message that made no MTA hop, to an alias the account does not know.
        let event = event_with(vec![
            organizer("Organiser", "organiser@test.local"),
            attendee("alias@test.local", ParticipationStatus::NeedsAction),
        ]);
        let scheduling = arrived(&[], &["alias@test.local"], event.clone());
        assert_eq!(
            invited_attendee(&event, &[], &scheduling).as_deref(),
            Some("alias@test.local")
        );
    }

    #[test]
    fn to_naming_several_attendees_is_no_match_rather_than_the_first() {
        let event = meeting();
        let scheduling = arrived(&[], EVERYONE, event.clone());
        assert_eq!(
            invited_attendee(&event, &["alias@test.local".to_owned()], &scheduling),
            None
        );
    }

    #[test]
    fn an_organiser_in_to_does_not_make_the_one_invitee_ambiguous() {
        let event = event_with(vec![
            organizer("Organiser", "organiser@test.local"),
            attendee("alias@test.local", ParticipationStatus::NeedsAction),
        ]);
        let scheduling = arrived(
            &[],
            &["organiser@test.local", "alias@test.local"],
            event.clone(),
        );
        assert_eq!(
            invited_attendee(&event, &[], &scheduling).as_deref(),
            Some("alias@test.local")
        );
    }

    #[test]
    fn an_attendee_listed_twice_is_still_one_attendee() {
        let event = event_with(vec![
            attendee("alias@test.local", ParticipationStatus::NeedsAction),
            attendee("mailto:ALIAS@test.local", ParticipationStatus::NeedsAction),
        ]);
        let scheduling = arrived(&[], &["alias@test.local"], event.clone());
        assert_eq!(
            invited_attendee(&event, &[], &scheduling).as_deref(),
            Some("alias@test.local")
        );
    }
}
