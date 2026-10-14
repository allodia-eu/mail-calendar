//! Organising a meeting: on which calendars, and as whom.
//!
//! Storing a meeting and telling the people on it are two facts, and only the server decides the
//! second. A meeting is therefore offered and written only where the server sends the invitations
//! itself, and only as an address it schedules as: anywhere else the meeting would be stored and
//! nobody on it told, while the organiser's own calendar said they had been invited
//! (`docs/calendar.md`, "Organising a meeting").

use engine_api::{AccountId, CalendarUserAddresses, Event, EventDraft, Invitee, Provider};

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
    /// cannot invite as somebody else.
    pub(crate) async fn meeting_draft(
        &self,
        account: &AccountId,
        draft: EventDraft,
        invitees: Vec<Invitee>,
    ) -> Option<EventDraft> {
        let handle = self.account_handle(account).await?;
        if !Self::account_can_invite(&handle) {
            log::warn!("create_calendar_event: this calendar's server does not send invitations");
            return None;
        }
        let organiser = self.sender_identity(account).await?;
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
        let Some(handle) = self.account_handle(account).await else {
            return false;
        };
        if !Self::account_can_invite(&handle) {
            return false;
        }
        let addresses = self.account_address_set(account).await;
        crate::invitations::may_edit_roster(stored, &addresses)
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
