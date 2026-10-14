//! The meeting editor's two questions, answered once for every client: whether what was typed
//! is an address that can be invited, and what an edited invitee list changes.

use engine_api::{CalendarAddress, addresses_match};

use crate::{EventAttendee, MeetingInvitee, MeetingInviteePatch};

/// What an invitee field's value would do if added to the list.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum InviteeCheck {
    /// Invite this address, written the way the meeting will store it.
    Valid {
        /// The canonical address.
        address: String,
    },
    /// Not an email address.
    NotAnAddress,
    /// Already on the list.
    AlreadyInvited,
    /// The organiser's own address. The organiser attends by definition, and a meeting that
    /// invited them as well would be refused.
    Organiser,
}

/// Whether `typed` can be added to a meeting whose list already holds `invited` and whose
/// organiser is `organiser` (the chosen calendar's account address on a create, the organiser's
/// row on an edit). An editor adds the address only on [`InviteeCheck::Valid`], and says why not
/// otherwise.
#[must_use]
#[uniffi::export]
pub fn check_meeting_invitee(
    typed: String,
    invited: Vec<String>,
    organiser: String,
) -> InviteeCheck {
    let Ok(address) = CalendarAddress::parse(typed.trim()) else {
        return InviteeCheck::NotAnAddress;
    };
    let same = |other: &str| addresses_match(other, address.as_str());
    if same(&organiser) {
        InviteeCheck::Organiser
    } else if invited.iter().any(|other| same(other)) {
        InviteeCheck::AlreadyInvited
    } else {
        InviteeCheck::Valid {
            address: address.as_str().to_owned(),
        }
    }
}

/// What turning the meeting's `attendees` into `invitees` changes, as the patch an
/// `EventIntent::Update` carries; `None` when it changes nothing.
///
/// Only the rows an organiser may edit take part: an attendee with no `role` (the organiser, a
/// chair) is neither removed for being absent from `invitees` nor changed by appearing in it. A
/// row is changed when its role moved, and the change carries no name, so the name the server
/// holds stays: the one on the row is the bounded display text, not the stored value.
#[must_use]
#[uniffi::export]
pub fn meeting_invitee_patch(
    attendees: Vec<EventAttendee>,
    invitees: Vec<MeetingInvitee>,
) -> Option<MeetingInviteePatch> {
    let editable: Vec<&EventAttendee> = attendees
        .iter()
        .filter(|attendee| !attendee.is_organizer && attendee.role.is_some())
        .collect();
    let fixed = |email: &str| {
        attendees.iter().any(|attendee| {
            (attendee.is_organizer || attendee.role.is_none())
                && addresses_match(&attendee.email, email)
        })
    };
    let upsert: Vec<MeetingInvitee> = invitees
        .iter()
        .filter(|invitee| !fixed(&invitee.email))
        .filter_map(|invitee| {
            match editable
                .iter()
                .find(|attendee| addresses_match(&attendee.email, &invitee.email))
            {
                None => Some(invitee.clone()),
                Some(attendee) if attendee.role != Some(invitee.role) => Some(MeetingInvitee {
                    name: None,
                    ..invitee.clone()
                }),
                Some(_) => None,
            }
        })
        .collect();
    let remove: Vec<String> = editable
        .iter()
        .filter(|attendee| {
            !invitees
                .iter()
                .any(|invitee| addresses_match(&attendee.email, &invitee.email))
        })
        .map(|attendee| attendee.email.clone())
        .collect();
    (!upsert.is_empty() || !remove.is_empty()).then_some(MeetingInviteePatch { upsert, remove })
}

#[cfg(test)]
#[path = "meeting_editor_tests.rs"]
mod tests;
