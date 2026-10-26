//! Answering an invitation from its event in the calendar, rather than from the email it came in.
//!
//! The answer goes the way the invitation card's does (`docs/invitations.md`, "Who delivers the
//! answer"): where the calendar server schedules, storing the answer is sending it, and a server
//! that reports it could not deliver gets the same question; where it does not, this core writes
//! the answer and emails the `REPLY` itself. What the card reads from the iTIP message is read
//! here from the stored event, which is the copy the calendar shows.
//!
//! The engine's RSVP writes an event as a whole, so an answer given here is for the whole
//! meeting, a series included (`docs/calendar.md`, "Answering from the calendar").
//!
//! **Privacy.** A meeting's title, organiser and attendees are content; nothing here logs them.

use engine_api::{AccountId, Event, EventRsvp, ParticipantRole, Provider, resolve_instant_in};
use mailcal_viewmodel::ResponseStatus;

use crate::{
    App, CalendarWriteStatus,
    helpers::generated_idempotency,
    invitations::{Delivery, matched_attendee, may_edit_roster, my_response},
    invitations_imip::{ReplyLetter, ReplySource, attendee_name},
    invitations_rsvp::InvitationResponse,
    reference::EventRef,
};

impl<P: Provider> App<P> {
    /// Whether `account`'s calendar is connected now: a calendar adapter is bound to it.
    async fn calendar_connected(&self, account: &AccountId) -> bool {
        self.account_handle(account)
            .await
            .is_some_and(|handle| !handle.calendar_providers.is_empty())
    }

    /// This account's answer to `stored`, when it may give one from the calendar: it is invited,
    /// it does not organise the meeting, its calendar is connected, and the account has a way to
    /// tell the organiser. `None` when any of those fails, which is when a client offers no
    /// answer.
    pub(crate) async fn answer_from_calendar(
        &self,
        account: &AccountId,
        stored: &Event,
    ) -> Option<ResponseStatus> {
        // An appointment invites nobody, and needs neither the alias list nor the account.
        if stored.participants.is_empty() || !self.calendar_connected(account).await {
            return None;
        }
        let addresses = self.account_address_set(account).await;
        if may_edit_roster(stored, &addresses) {
            return None;
        }
        let attendee = matched_attendee(stored, &addresses)?;
        let mail = self.answering_mail(account, &attendee).await;
        if self.delivery_between(&mail, account, Some(&attendee)).await == Delivery::None {
            return None;
        }
        Some(my_response(stored, &addresses))
    }

    /// Answers the meeting `event` names, then rebuilds the calendar from what the server holds.
    ///
    /// Reports progress through [`CalendarWriteStatus`] like the invitation card's answer, and
    /// like it applies nothing optimistically.
    ///
    /// # Errors
    ///
    /// Returns why the answer did not happen: the event is gone, this account organises it or
    /// is not on it, the account has no way to tell the organiser, or a write or a send failed.
    pub(crate) async fn respond_to_event(
        &self,
        event: &EventRef,
        response: InvitationResponse,
        reply_subject: Option<String>,
    ) -> Result<(), String> {
        self.set_calendar_write_status(CalendarWriteStatus::Saving);
        let outcome = self
            .send_event_response(event, response, reply_subject)
            .await;
        if outcome.is_err() {
            self.answer_failed();
        }
        outcome
    }

    async fn send_event_response(
        &self,
        event: &EventRef,
        response: InvitationResponse,
        reply_subject: Option<String>,
    ) -> Result<(), String> {
        let stored = self
            .stored_event(event)
            .await
            .ok_or_else(|| "the event is no longer in the calendar".to_owned())?;
        let addresses = self.account_address_set(&event.account).await;
        if may_edit_roster(&stored, &addresses) {
            return Err("this account organises the meeting, so there is nothing to answer".into());
        }
        let letter = self
            .event_letter(event, &stored, &addresses, response, None, reply_subject)
            .await?;
        let acct = self
            .account_handle(&event.account)
            .await
            .ok_or_else(|| "the account is not configured".to_owned())?;
        // What the grid shows of a calendar that is not connected is what was last synced; an
        // answer given from it would tell the organiser one thing and leave the calendar saying
        // another.
        let provider = acct
            .calendar_providers
            .first()
            .ok_or_else(|| "the calendar is not connected".to_owned())?;

        match self
            .delivery_between(&letter.account, &event.account, Some(&letter.attendee))
            .await
        {
            Delivery::None => Err(
                "this account cannot answer invitations: its calendar server does not send \
                 replies for you, and its mail account cannot send one either."
                    .to_owned(),
            ),
            Delivery::ClientImip => {
                // The calendar's copy changes where it can be written, so the grid agrees with
                // what the organiser is told; the reply is what tells them.
                let answer_store = Some(provider)
                    .filter(|p| p.connection_info().capabilities.calendar_rsvp().is_some());
                if let Some(provider) = answer_store {
                    let rsvp = EventRsvp::to(&stored, letter.attendee.clone(), response.engine());
                    let write = self
                        .engine
                        .rsvp_calendar_event(
                            provider,
                            &event.account,
                            &generated_idempotency(),
                            &stored,
                            &rsvp,
                        )
                        .await
                        .map_err(|err| err.to_string())?;
                    let status = self
                        .settle_calendar_write(provider, &event.account, write.reconciled)
                        .await;
                    self.set_calendar_write_status(status);
                }
                self.send_reply_letter(&letter).await?;
                if answer_store.is_none() {
                    // Nothing was written, so there is no write to settle; a client holds the
                    // answer back while `Saving`, and would hold it for good.
                    self.set_calendar_write_status(CalendarWriteStatus::Idle);
                }
                self.rebuild_calendar_view().await;
                Ok(())
            }
            Delivery::Server => {
                let rsvp = EventRsvp::to(&stored, letter.attendee.clone(), response.engine());
                let write = self
                    .engine
                    .rsvp_calendar_event(
                        provider,
                        &event.account,
                        &generated_idempotency(),
                        &stored,
                        &rsvp,
                    )
                    .await
                    .map_err(|err| self.answer_write_failed(&err))?;
                let status = self
                    .settle_calendar_write(provider, &event.account, write.reconciled)
                    .await;
                let mail = letter.account.clone();
                let outcome = self
                    .resolve_reply_delivery(&mail, Ok(letter), &write.write.reply_delivery)
                    .await;
                self.rebuild_calendar_view().await;
                self.set_calendar_write_status(status);
                self.apply_reply_outcome(outcome)
            }
        }
    }

    /// The reply this account gives to the meeting `event` names, read from the calendar's
    /// copy: for the delivery question's "email it" after an answer given here.
    ///
    /// # Errors
    ///
    /// Returns why no reply can be written: the event is gone, or see [`Self::event_letter`].
    pub(crate) async fn event_reply_letter(
        &self,
        event: &EventRef,
        response: InvitationResponse,
        comment: Option<String>,
        reply_subject: Option<String>,
    ) -> Result<ReplyLetter, String> {
        let stored = self
            .stored_event(event)
            .await
            .ok_or_else(|| "the event is no longer in the calendar".to_owned())?;
        let addresses = self.account_address_set(&event.account).await;
        self.event_letter(event, &stored, &addresses, response, comment, reply_subject)
            .await
    }

    /// The reply read from `stored`, the calendar's copy of the meeting.
    ///
    /// The organiser is the participant the stored meeting names `Owner`, or `Chair` where none
    /// is `Owner` (the JMAP shape). The answer is for the whole meeting, so no `RECURRENCE-ID`.
    ///
    /// # Errors
    ///
    /// Returns why no reply can be written: the meeting does not invite this account, names no
    /// organiser, or its start cannot be placed.
    async fn event_letter(
        &self,
        event: &EventRef,
        stored: &Event,
        addresses: &[String],
        response: InvitationResponse,
        comment: Option<String>,
        reply_subject: Option<String>,
    ) -> Result<ReplyLetter, String> {
        let attendee = matched_attendee(stored, addresses)
            .ok_or_else(|| "this meeting does not invite this account".to_owned())?;
        let has_owner = stored
            .participants
            .iter()
            .any(|participant| participant.has_role(&ParticipantRole::Owner));
        let role = if has_owner {
            ParticipantRole::Owner
        } else {
            ParticipantRole::Chair
        };
        let organizer = stored
            .participants
            .iter()
            .find(|participant| participant.has_role(&role))
            .and_then(|participant| participant.email.clone())
            .ok_or_else(|| {
                "this meeting names no organizer, so there is nobody to reply to.".to_owned()
            })?;
        let starts_at = resolve_instant_in(&stored.start, &self.active_zone())
            .map_err(|err| format!("the meeting's start cannot be placed: {err}"))?;
        Ok(ReplyLetter {
            account: self.answering_mail(&event.account, &attendee).await,
            source: ReplySource::Event(event.clone()),
            summary: stored.title.clone(),
            uid: stored.uid.as_str().to_owned(),
            sequence: stored.sequence,
            organizer,
            attendee_name: attendee_name(stored, &attendee).map(str::to_owned),
            attendee,
            response,
            comment,
            starts_at,
            recurrence_id: None,
            subject: reply_subject.unwrap_or_else(|| format!("Re: {}", stored.title)),
            thread: None,
        })
    }
}
