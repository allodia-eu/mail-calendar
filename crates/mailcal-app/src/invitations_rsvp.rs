//! Answering an invitation from the reading view.
//!
//! The card is anchored to a **message**, but the write lands on an **event**: so this
//! module's whole job is bridging the two: re-read the iTIP payload behind the message, work
//! out which of the account's addresses the invitation matched, find the event that `UID`
//! names in the calendar, and hand the engine a neutral [`EventRsvp`].
//!
//! # Why the address is re-derived rather than passed in
//!
//! An invitation to `info@…` on an account whose identity is `alice@…` must answer as
//! **`info@…`**, that is the `ATTENDEE` line the server looks for. A client cannot supply it:
//! it never sees the address set, and asking it to would put the alias rule in five places.
//! So the intent names the message, and the matching happens here, once, using the same
//! `matched_attendee` the card was built with. The card and the answer therefore cannot
//! disagree about who the user is.
//!
//! # Why the event has to be found by `UID` over a window
//!
//! The store indexes events by provider key, not by `UID`, and the invitation only knows the
//! latter. The meeting's own day is already read here for the conflict count, so the lookup
//! costs one occurrence read over that window, and if the event is not there, the answer is
//! refused rather than guessed at. That is the `ClientImip` case (a bare IMAP account with no
//! auto-scheduling server): the mail arrived, nothing put the meeting in a calendar, and
//! there is nothing to write to.
//!
//! **Privacy.** A meeting's title, organiser and attendees are message content, and an
//! address is worse. Nothing here logs any of it; counts and outcomes only
//! (`docs/logging.md`).

use engine_api::{
    AccountId, Event, EventRsvp, Horizon, ParticipationStatus, Provider, ProviderKey, RsvpResponse,
    UtcDateTime, resolve_instant_in,
};

use crate::{
    App, CalendarWriteStatus, helpers::generated_idempotency, invitations::Delivery,
    invitations_imip::ClientAnswer, reference::MessageRef,
};

/// What the user chose. The core's own three-value answer, mapped onto the engine's on the
/// way out: the client's enum, the core's enum and the engine's are deliberately separate
/// types so a new engine variant cannot silently become a new button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvitationResponse {
    /// Yes.
    Accept,
    /// Maybe.
    Tentative,
    /// No. The meeting then disappears from the grid (`docs/calendar.md`), reachable again
    /// through this very card.
    Decline,
}

impl InvitationResponse {
    /// The engine verb this answer becomes.
    pub(crate) const fn engine(self) -> RsvpResponse {
        match self {
            Self::Accept => RsvpResponse::Accepted,
            Self::Tentative => RsvpResponse::Tentative,
            Self::Decline => RsvpResponse::Declined,
        }
    }

    /// The `PARTSTAT` this answer writes into an iTIP `REPLY` we build ourselves.
    ///
    /// A separate mapping from [`engine`](Self::engine) because they answer different
    /// questions: that one names a verb a transport performs, this one names a **value in a
    /// document**. Collapsing them would tie the wire format of a reply to whatever the engine
    /// happens to call its RSVP verbs next.
    pub(crate) const fn partstat(self) -> ParticipationStatus {
        match self {
            Self::Accept => ParticipationStatus::Accepted,
            Self::Tentative => ParticipationStatus::Tentative,
            Self::Decline => ParticipationStatus::Declined,
        }
    }
}

impl<P: Provider> App<P> {
    /// Answers the invitation `message` carries, then rebuilds the calendar and the reading
    /// view from what the server now holds.
    ///
    /// Reports progress through [`CalendarWriteStatus`] like every other calendar write
    /// (`Saving` → `Saved`/`Failed`): the answer changes the calendar, so it belongs on the
    /// same indicator, and a decline that vanishes an event must not do so silently.
    ///
    /// **Nothing is applied optimistically.** The write is awaited inline behind that
    /// spinner, and both surfaces are then rebuilt from what the server actually holds. The
    /// alternative; hide the declined meeting immediately and put it back if the write
    /// fails; buys a few hundred milliseconds and costs a rollback path that would be
    /// exercised only when something has already gone wrong. Removing an event from the
    /// user's day is not the place for that trade.
    ///
    /// # Errors
    ///
    /// Returns the reason the answer did not happen: the message is not an invitation this
    /// account can answer, the meeting is not in the calendar, the account's transport cannot
    /// RSVP or cannot honour a control the caller asked for, or the provider call failed. The
    /// caller must not report a failure as sent: a reply the organiser never got is exactly
    /// the failure this whole feature exists to avoid.
    pub(crate) async fn respond_to_invitation(
        &self,
        message: &MessageRef,
        response: InvitationResponse,
        comment: Option<String>,
        notify_organizer: bool,
        reply_subject: Option<String>,
    ) -> Result<(), String> {
        // `Saving` goes up **before** the first check, and every failure below lands on
        // `Failed`. The checks are local and fast, so this is not about the spinner; it is
        // that a refusal must be *visible*. An invitation whose meeting is on no calendar used
        // to return early with the indicator still `Idle`: the user tapped Accept, nothing
        // happened, and nothing said why. That is the exact failure this feature exists to
        // remove, reintroduced at the last step.
        self.set_calendar_write_status(CalendarWriteStatus::Saving);
        let outcome = self
            .send_invitation_response(message, response, comment, notify_organizer, reply_subject)
            .await;
        if outcome.is_err() {
            self.set_calendar_write_status(CalendarWriteStatus::Failed);
        }
        outcome
    }

    /// The answer itself. Split from [`Self::respond_to_invitation`] only so that every `?`
    /// in here is reported, rather than each one having to remember to.
    async fn send_invitation_response(
        &self,
        message: &MessageRef,
        response: InvitationResponse,
        comment: Option<String>,
        notify_organizer: bool,
        reply_subject: Option<String>,
    ) -> Result<(), String> {
        let original = self
            .find_message_in(message)
            .await
            .ok_or_else(|| "the message is no longer in the store".to_owned())?;
        let scheduling = self
            .fetch_scheduling(message, &original)
            .await
            .ok_or_else(|| "the message carries no invitation to answer".to_owned())?;
        let event = &scheduling.message.event;

        // The same address set, and the same matcher, the card was built with: so the
        // buttons and the write cannot disagree about which identity is answering (§4/D5).
        let mut addresses = self.account_address_set(&message.account).await;
        addresses.extend(scheduling.delivery_recipients.iter().cloned());
        let attendee = crate::invitations::matched_attendee(event, &addresses)
            .ok_or_else(|| "this invitation is not addressed to this account".to_owned())?;

        let zone = self.active_zone();
        let starts_at = resolve_instant_in(&event.start, &zone)
            .map_err(|err| format!("the invitation's start cannot be placed: {err}"))?;
        let ends_at = crate::invitations_build::end_of(starts_at, event);

        let stored = self
            .stored_event_by_uid(&message.account, event.uid.as_str(), starts_at, ends_at)
            .await;

        // A superseded invitation is refused here as well as hidden on the card. The two are not
        // redundant: the write would land on whatever the calendar now holds, while the user was
        // reading the *old* mail's times: so answering "yes" would agree to a slot they were
        // never shown. The card is the disclosure; this is the guard (`docs/invitations.md`).
        if stored
            .as_ref()
            .is_some_and(|held| held.sequence > event.sequence)
        {
            return Err(
                "the organizer has sent a newer version of this invitation, so this copy \
                        can no longer be answered. Open the newer one to reply."
                    .to_owned(),
            );
        }

        // Which of the two routes this account has, and it is read *now*, from the same
        // capabilities the card was gated on, so the buttons and the write cannot disagree
        // about whether anyone will be told (`docs/invitations.md` → "Who delivers the answer").
        match self.account_delivery(&message.account).await {
            Delivery::ClientImip => {
                self.answer_by_imip(&ClientAnswer {
                    message,
                    original: &original,
                    scheduling: &scheduling,
                    attendee: &attendee,
                    response,
                    comment: comment.as_deref(),
                    notify_organizer,
                    reply_subject: reply_subject.as_deref(),
                    starts_at,
                    ends_at,
                    stored,
                })
                .await?;
                self.rebuild_calendar_view().await;
                self.republish_reading(message.clone()).await;
                return Ok(());
            }
            Delivery::None => {
                return Err(
                    "this account cannot answer invitations: its calendar server does \
                            not send replies for you, and its mail account cannot send one \
                            either."
                        .to_owned(),
                );
            }
            Delivery::Server => {}
        }

        // The meeting is answered on the calendar's copy: so if the calendar has none, put it
        // there first. A server that schedules is **not** thereby a server that files inbound
        // invitations: `caldav.soverin.net` advertises `calendar-auto-schedule` and still leaves
        // every emailed invitation sitting in the mailbox, because those are different jobs and
        // no RFC assigns the second to anyone. This used to be a dead end here, which is the
        // bug: Accept, and "the meeting is not in this account's calendar".
        let stored = match stored {
            Some(event) => event,
            None => self
                .store_invitation(&message.account, &scheduling, starts_at, ends_at)
                .await?
                .ok_or_else(|| {
                    "the meeting is not in this account's calendar, and this account has no \
                     calendar to add it to."
                        .to_owned()
                })?,
        };

        let mut rsvp = EventRsvp::to(&stored, attendee.clone(), response.engine());
        // Empty is not a comment. A client that always sends its (blank) note field would
        // otherwise be refused on every transport that carries no note at all.
        if let Some(text) = comment.clone().filter(|text| !text.trim().is_empty()) {
            rsvp = rsvp.comment(text);
        }
        if !notify_organizer {
            rsvp = rsvp.quietly();
        }

        let acct = self
            .account_handle(&message.account)
            .await
            .ok_or_else(|| "the account is not configured".to_owned())?;
        let provider = acct
            .calendar_providers
            .first()
            .ok_or_else(|| "the account has no calendar provider".to_owned())?;

        let write = self
            .engine
            .rsvp_calendar_event(
                provider,
                &message.account,
                &generated_idempotency(),
                &stored,
                &rsvp,
            )
            .await
            .map_err(|err| err.to_string())?;

        let status = self
            .settle_calendar_write(provider, &message.account, write.reconciled)
            .await;

        // The answer is stored. Whether anyone was *told* is a separate question, and this
        // route is the one that used to assume the answer was yes: `calendar-auto-schedule`
        // promises the server sends the reply, and a server that permanently fails to says so
        // in the event rather than in the response (RFC 6638 §3.2.9). The engine reads that
        // report while it performs the write and hands it back on the receipt, so the verdict
        // is known by the time this returns: no second round trip, and nothing to poll. Acting
        // on it is what stops "You accepted" from being the only thing a user ever sees when
        // the organiser was never told (`docs/invitations.md`).
        let outcome = self
            .resolve_reply_delivery(
                &ClientAnswer {
                    message,
                    original: &original,
                    scheduling: &scheduling,
                    attendee: &attendee,
                    response,
                    comment: comment.as_deref(),
                    notify_organizer,
                    reply_subject: reply_subject.as_deref(),
                    starts_at,
                    ends_at,
                    stored: Some(stored.clone()),
                },
                &write.write.reply_delivery,
            )
            .await;

        // Both surfaces move: the grid, because a decline removes the event and an accept
        // turns a hold into a commitment, and the reading view, because the card's answer now
        // comes from the calendar rather than from the frozen mail.
        self.rebuild_calendar_view().await;
        self.republish_reading(message.clone()).await;
        self.set_calendar_write_status(status);
        self.apply_reply_outcome(outcome)
    }

    /// Re-reads the open message and republishes its snapshot, so the card shows the answer
    /// that just landed.
    ///
    /// Deliberately **not** `open_message`: that marks the message read on the server, which
    /// answering an invitation is no reason to do: the user may well have answered from the
    /// list without opening it.
    async fn republish_reading(&self, message: MessageRef) {
        let snapshot = self.fetch_reading(message).await;
        self.reading.publish(snapshot);
    }

    /// The stored [`Event`] a `UID` names in `account`, looked up across the meeting's own
    /// window.
    ///
    /// The store has no `UID` index, so this is an occurrence read over the day plus a
    /// keyed fetch of the masters: the same two calls the conflict count makes. `None` means
    /// the calendar does not hold the meeting, which is a refusal, never a reason to invent
    /// one: writing to an event we cannot see would mean answering something else.
    pub(crate) async fn stored_event_by_uid(
        &self,
        account: &AccountId,
        uid: &str,
        starts_at: UtcDateTime,
        ends_at: UtcDateTime,
    ) -> Option<Event> {
        let window = Horizon::new(starts_at, ends_at).ok()?;
        let rows = self.engine.occurrences_in(account, window).await.ok()?;
        let keys: Vec<ProviderKey> = rows
            .iter()
            .map(|row| row.event.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        self.engine
            .events_by_keys(account, &keys)
            .await
            .ok()?
            .into_iter()
            .find(|event| event.uid.as_str() == uid)
    }
}
