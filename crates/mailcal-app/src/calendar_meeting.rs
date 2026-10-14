//! Organising a meeting: on which calendars, and as whom.
//!
//! Storing a meeting and telling the people on it are two facts, and only the server decides the
//! second. A meeting is therefore offered and written only where the server sends the invitations
//! itself, and only as an address it schedules as: anywhere else the meeting would be stored and
//! nobody on it told, while the organiser's own calendar said they had been invited
//! (`docs/calendar.md`, "Organising a meeting").

use engine_api::{
    AccountId, CalendarUserAddresses, Event, EventDraft, Invitee, InviteePatch, Provider,
    addresses_match,
};

use crate::{Account, App};

impl<P: Provider> App<P> {
    /// Whether `account` may organise a meeting: its calendar can be written, and its server
    /// sends the invitations a meeting write implies (`Capabilities::calendar_scheduling`: an
    /// RFC 6638 auto-scheduling CalDAV server, JMAP, Graph, Google).
    pub(crate) fn account_can_invite(account: &Account<P>) -> bool {
        Self::account_can_write(account)
            && account.calendar_providers.first().is_some_and(|provider| {
                provider
                    .connection_info()
                    .capabilities
                    .calendar_scheduling()
            })
    }

    /// `draft` as a meeting `account` organises, inviting `invitees`; `None`, logged, when the
    /// account may not organise one or the roster is invalid.
    ///
    /// The organiser is the account's own sender identity, never a client's choice, so a client
    /// cannot invite as somebody else. `chosen` is the account the client picked: an appointment
    /// may fall back to the default account when that one cannot take it, but a meeting may not,
    /// because the account it lands in is the one that invites everybody.
    pub(crate) async fn meeting_draft(
        &self,
        chosen: Option<&str>,
        account: &AccountId,
        draft: EventDraft,
        invitees: Vec<Invitee>,
    ) -> Option<EventDraft> {
        if chosen.is_some_and(|chosen| chosen != account.as_str()) {
            log::warn!("create_calendar_event: the chosen calendar's account cannot be written");
            return None;
        }
        let handle = self.account_handle(account).await?;
        if !Self::account_can_invite(&handle) {
            log::warn!("create_calendar_event: this calendar's server does not send invitations");
            return None;
        }
        let Some(organiser) = self.sender_identity(account).await else {
            log::warn!("create_calendar_event: the account has no address to organise as");
            return None;
        };
        if !self.schedules_as(account, &organiser.email).await {
            log::warn!("create_calendar_event: the server does not schedule as this organiser");
            return None;
        }
        mailcal_account::attach_meeting(draft, &organiser, invitees)
            .inspect_err(|_| log::warn!("create_calendar_event: the meeting fields were invalid"))
            .ok()
    }

    /// Whether `account` may change the roster of `stored`: it organises the meeting, and its
    /// server sends the updates a roster change implies.
    pub(crate) async fn may_change_roster(&self, account: &AccountId, stored: &Event) -> bool {
        // An appointment has no organiser to be, so this needs neither the account nor its
        // aliases, which are read from disk.
        if stored.participants.is_empty() {
            return false;
        }
        let Some(handle) = self.account_handle(account).await else {
            return false;
        };
        if !Self::account_can_invite(&handle) {
            return false;
        }
        let addresses = self.account_address_set(account).await;
        crate::invitations::may_edit_roster(stored, &addresses)
    }

    /// Whether `account` may apply `changes` to the roster of `stored`: it may change the roster
    /// at all, and the roster the changes leave is one a new meeting would be allowed, so an
    /// update cannot do what [`mailcal_account::attach_meeting`] refuses a create.
    pub(crate) async fn may_apply_roster(
        &self,
        account: &AccountId,
        stored: &Event,
        changes: &InviteePatch,
    ) -> bool {
        if !leaves_a_valid_roster(stored, changes) {
            log::warn!("update_calendar_event: the invitee changes would leave an invalid meeting");
            return false;
        }
        if !self.may_change_roster(account, stored).await {
            return false;
        }
        // Asked here rather than for the detail, which stays a local read: a roster change on a
        // meeting organised as an address the server does not schedule as would reach nobody.
        let addresses = self.account_address_set(account).await;
        let Some(organiser) = stored.participants.iter().find_map(|participant| {
            participant
                .email
                .as_deref()
                .filter(|email| addresses.iter().any(|mine| addresses_match(email, mine)))
                .filter(|_| {
                    participant.has_role(&engine_api::ParticipantRole::Owner)
                        || participant.has_role(&engine_api::ParticipantRole::Chair)
                })
        }) else {
            return false;
        };
        if !self.schedules_as(account, organiser).await {
            log::warn!("update_calendar_event: the server does not schedule as this organiser");
            return false;
        }
        true
    }

    /// Whether `account`'s calendar server schedules as `address`.
    ///
    /// RFC 6638 §3.1 has a server act only for an organiser in the principal's calendar user
    /// address set, and say nothing about one outside it. A server that named its set, or said it
    /// schedules for nobody, is taken at its word; one that could not be asked leaves the
    /// decision to `calendar_scheduling`, already checked.
    async fn schedules_as(&self, account: &AccountId, address: &str) -> bool {
        match self.calendar_user_addresses(account).await {
            None | Some(CalendarUserAddresses::Unknown) => true,
            Some(users) => users.recognises(address),
        }
    }
}

/// Whether `changes` leave `stored` a meeting [`engine_api::MeetingDraft::validate`] would accept:
/// the organiser is not invited to their own meeting, and somebody still is.
///
/// The organiser is read as [`crate::invitations::may_edit_roster`] reads them: `Owner`, or
/// `Chair` where no `Owner` exists.
fn leaves_a_valid_roster(stored: &Event, changes: &InviteePatch) -> bool {
    let has_owner = stored
        .participants
        .iter()
        .any(|participant| participant.has_role(&engine_api::ParticipantRole::Owner));
    let organiser_role = if has_owner {
        engine_api::ParticipantRole::Owner
    } else {
        engine_api::ParticipantRole::Chair
    };
    let organisers: Vec<&str> = stored
        .participants
        .iter()
        .filter(|participant| participant.has_role(&organiser_role))
        .filter_map(|participant| participant.email.as_deref())
        .collect();
    let is_organiser = |address: &str| organisers.iter().any(|o| addresses_match(o, address));
    let upserts: Vec<&str> = changes
        .upserts()
        .iter()
        .map(|invitee| invitee.identity().address().as_str())
        .collect();
    if upserts.iter().any(|address| is_organiser(address)) {
        return false;
    }
    let removed = |address: &str| {
        changes
            .removals()
            .iter()
            .any(|gone| addresses_match(gone.as_str(), address))
    };
    !upserts.is_empty()
        || stored
            .participants
            .iter()
            .filter_map(|participant| participant.email.as_deref())
            .any(|address| !is_organiser(address) && !removed(address))
}
