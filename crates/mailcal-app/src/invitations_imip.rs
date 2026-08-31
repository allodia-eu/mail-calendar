//! Answering an invitation when **nothing else will tell the organiser**: the client-iMIP
//! route ([`Delivery::ClientImip`](crate::invitations::Delivery::ClientImip)).
//!
//! On the very common account shape of IMAP mail beside a plain RFC 4791 calendar, an
//! invitation arrives as an email and no part of the system connects the two: the calendar
//! server never sees it, so it deposits no hold and (crucially) schedules nothing when the
//! answer is stored. A `PARTSTAT` written there is a note to oneself.
//!
//! So this module does what Thunderbird does, in three steps, in this order:
//!
//! 1. **Put the meeting on the calendar** ([`App::store_invitation`]) if it is not already there,
//!    from the invitation's own bytes, under a guarded create.
//! 2. **Store the answer** through the ordinary RSVP verb, so the user's own diary records it.
//! 3. **Send the iTIP `REPLY`** as an iMIP message ([`App::send_imip_reply`]), which is the step
//!    that actually reaches the organiser.
//!
//! # Why that order, given either step can fail
//!
//! The reply goes **last**, and a failure to send it fails the whole answer. Reversed, a
//! successful send followed by a failed store would leave the organiser told and the user's
//! calendar silent, and the user, seeing an error, would press the button again and answer
//! twice. This way the worst case is a meeting sitting in the calendar unanswered, with an
//! error that says the answer was not sent, which is both true and fixed by pressing again.
//!
//! **Privacy.** Every log line here is a count or an outcome. A meeting's title, its organiser,
//! the attendee address and the user's note are exactly what `docs/logging.md` forbids.

use engine_api::{
    AccountId, ApiError, Draft, DraftCalendar, EmailAddress, Event, EventId, EventRsvp, EventWrite,
    InboundScheduling, Message, MessageIdHeader, Provider, RawIcal, ScheduleMethod, UtcDateTime,
    addresses_match, resolve_instant_in,
};

use crate::{
    App, CalendarWriteStatus,
    helpers::{generated_idempotency, generated_message_id, now_utc},
    invitations_rsvp::InvitationResponse,
    reference::MessageRef,
};

/// One answer, with everything the two writes and the one send need already resolved.
#[derive(Debug)]
pub(crate) struct ClientAnswer<'a> {
    /// The message the invitation arrived on.
    pub message: &'a MessageRef,
    /// That message as the store holds it; read for the subject the reply answers.
    pub original: &'a Message,
    /// The parsed iTIP payload, including the verbatim bytes a stored copy is made from.
    pub scheduling: &'a InboundScheduling,
    /// The address this account answers **as**: the one the invitation matched, which on an
    /// aliased account is not the account's primary identity.
    pub attendee: &'a str,
    /// The answer.
    pub response: InvitationResponse,
    /// The user's note to the organiser, if they wrote one. Becomes a `COMMENT` property.
    pub comment: Option<&'a str>,
    /// Whether to send the reply at all. On this route the choice is real; we are the sender.
    pub notify_organizer: bool,
    /// The localised subject line the client composed, if it supplied one.
    pub reply_subject: Option<&'a str>,
    /// The meeting's start as an instant.
    pub starts_at: UtcDateTime,
    /// The meeting's end as an instant, for the read-back window.
    pub ends_at: UtcDateTime,
    /// The calendar's copy, if the calendar already has one.
    pub stored: Option<Event>,
}

impl<P: Provider> App<P> {
    /// Answers `answer` over the client-iMIP route: store the meeting, store the answer, send
    /// the reply.
    ///
    /// # Errors
    ///
    /// Returns the reason the answer did not happen. A failure at any step is a failure of the
    /// whole answer; in particular a reply that could not be sent, which is the one failure
    /// this route exists to stop happening silently.
    pub(crate) async fn answer_by_imip(&self, answer: &ClientAnswer<'_>) -> Result<(), String> {
        let stored = match answer.stored.clone() {
            Some(event) => Some(event),
            None => {
                self.store_invitation(
                    &answer.message.account,
                    answer.scheduling,
                    answer.starts_at,
                    answer.ends_at,
                )
                .await?
            }
        };

        if let Some(stored) = &stored {
            self.store_answer(answer, stored).await?;
        }
        if answer.notify_organizer {
            self.send_imip_reply(answer, stored.as_ref()).await?;
        } else {
            log::info!("respond_to_invitation: the organizer was deliberately not told");
        }
        Ok(())
    }

    /// Writes this account's `PARTSTAT` into the stored meeting.
    ///
    /// Deliberately carries **neither** the note nor the "do not notify" flag. Both are refused
    /// by a CalDAV transport rather than dropped ([`RsvpControls`](engine_api::RsvpControls)),
    /// and on this route neither belongs here anyway: the note travels as a `COMMENT` in the
    /// reply we build, and whether the organiser hears about it is decided by whether we send
    /// that reply at all. Passing them here would turn a supported answer into an error.
    async fn store_answer(&self, answer: &ClientAnswer<'_>, stored: &Event) -> Result<(), String> {
        let acct = self
            .account_handle(&answer.message.account)
            .await
            .ok_or_else(|| "the account is not configured".to_owned())?;
        let provider = acct
            .calendar_providers
            .first()
            .ok_or_else(|| "the account has no calendar provider".to_owned())?;
        let rsvp = EventRsvp::to(stored, answer.attendee, answer.response.engine());
        let write = self
            .engine
            .rsvp_calendar_event(
                provider,
                &answer.message.account,
                &generated_idempotency(),
                stored,
                &rsvp,
            )
            .await
            .map_err(|err| err.to_string())?;
        let status = self
            .settle_calendar_write(provider, &answer.message.account, write.reconciled)
            .await;
        self.set_calendar_write_status(status);
        Ok(())
    }

    /// Puts the invitation on the account's calendar, and returns the stored event.
    ///
    /// `Ok(None)` means the account has **no calendar provider at all**: a bare IMAP account;
    /// so there is nothing to store and nothing lost by not storing it. The reply still goes.
    ///
    /// The document is the invitation's own bytes with its `METHOD` removed
    /// ([`crate::itip::for_storage`]), never a re-serialization of the parsed projection: the
    /// `ATTENDEE` line is the whole reason to store the invitation rather than a plain
    /// appointment, and a round-trip through a lossy model is exactly how such a line goes
    /// missing. That is also why this is [`EventWrite`] and not
    /// [`EventDraft`](engine_api::EventDraft), which carries neither an organiser nor attendees.
    ///
    /// # Both routes need this, and a live account is why
    ///
    /// It would be tidy if only the client-iMIP route had to store the meeting: a server that
    /// schedules puts it there itself. It is not true. `caldav.soverin.net` advertises
    /// `calendar-auto-schedule`, so the capability reads `true` and the route is `Server`; but
    /// nothing on that deployment moves an invitation from the **mailbox** into the calendar,
    /// and no RFC says anything should. Those are different jobs, and the token only claims the
    /// first. So the meeting is absent on a server that promises to schedule, and answering
    /// used to fail there with "the meeting is not in this account's calendar".
    ///
    /// Storing it is exactly the flow RFC 6638 §3.2.2 describes for an attendee: the client
    /// puts the scheduling object in its own calendar, and the server turns the changed
    /// `PARTSTAT` into the `REPLY`. It is what Apple Calendar and Thunderbird do, and it is
    /// harmless where the meeting is already there: the guarded create finds it and says so.
    pub(crate) async fn store_invitation(
        &self,
        account: &AccountId,
        scheduling: &InboundScheduling,
        starts_at: UtcDateTime,
        ends_at: UtcDateTime,
    ) -> Result<Option<Event>, String> {
        let acct = self
            .account_handle(account)
            .await
            .ok_or_else(|| "the account is not configured".to_owned())?;
        let Some(provider) = acct.calendar_providers.first() else {
            log::info!("respond_to_invitation: no calendar on this account; replying only");
            return Ok(None);
        };
        let event = &scheduling.message.event;
        let raw = event.raw_ical.as_ref().ok_or_else(|| {
            "the invitation did not survive with its original calendar data, so it cannot be \
             added to your calendar."
                .to_owned()
        })?;
        let document = crate::itip::for_storage(raw.as_str())
            .ok_or_else(|| "the invitation carries no calendar object to store".to_owned())?;

        // The account's first calendar, the same default a new event takes (`calendar_ops`).
        //
        // Read from the **store**, which is empty until a calendar sync has run, and answering
        // an invitation must not require the user to have visited the Calendar tab first. Found
        // against the live harness, where the mail list is where you land and the collection
        // list was therefore never fetched: the answer failed with "this account has no
        // calendar", on an account whose calendar was connected and discovered seconds earlier.
        // So an empty list means *not looked yet*, and the fix is to look.
        let mut calendars = self
            .engine
            .calendars(account)
            .await
            .map_err(|err| err.to_string())?;
        if calendars.is_empty() {
            log::info!("respond_to_invitation: no calendars in the store yet; syncing first");
            self.refresh_calendar_in_background().await;
            calendars = self
                .engine
                .calendars(account)
                .await
                .map_err(|err| err.to_string())?;
        }
        let collection = calendars
            .first()
            .ok_or_else(|| "this account has no calendar to add the meeting to".to_owned())?;
        let href = event_href(collection.id.key().as_str(), event.uid.as_str())
            .ok_or_else(|| "the meeting's id cannot be made into a calendar address".to_owned())?;
        let key = href.key().clone();

        let write = EventWrite::creating(href, event.uid.clone(), RawIcal::new(document));
        self.set_calendar_write_status(CalendarWriteStatus::Saving);
        match self
            .engine
            .put_calendar_document(provider, account, &generated_idempotency(), &write)
            .await
        {
            Ok(landed) => {
                log::info!("respond_to_invitation: stored the invitation on the calendar");
                let status = self
                    .settle_calendar_write(provider, account, landed.reconciled)
                    .await;
                self.set_calendar_write_status(status);
            }
            Err(err) => {
                // A refused precondition means something is **already** at that address; the
                // server deposited its own copy, or another device stored one first. That is
                // not a failure to answer: it is the create finding the meeting already there,
                // which is what the guard is for. Read it back and answer on it. Any other
                // error is a real failure.
                if !is_conflict(&err) {
                    return Err(format!(
                        "the meeting could not be added to your calendar: {err}"
                    ));
                }
                log::info!(
                    "respond_to_invitation: the meeting was already on the calendar; using it"
                );
                let _ = self
                    .engine
                    .reconcile_calendar_events(provider, account)
                    .await;
            }
        }

        // Read the event back **by the address we wrote to**, not by scanning the meeting's day
        // for its `UID`: the occurrence index only covers the window the store has expanded, and
        // an invitation for a date beyond it would come back empty from a write that landed
        // perfectly.
        let stored = self
            .engine
            .events_by_keys(account, core::slice::from_ref(&key))
            .await
            .map_err(|err| err.to_string())?
            .into_iter()
            .next();
        // Fall back to the day scan, which finds a copy the server put at an address of its own
        // choosing: the case the conflict above lands in on a server whose href convention is
        // not ours.
        match stored {
            Some(event) => Ok(Some(event)),
            None => Ok(self
                .stored_event_by_uid(account, event.uid.as_str(), starts_at, ends_at)
                .await),
        }
    }

    /// Builds the iTIP `REPLY` and sends it as an iMIP message: the step the organizer
    /// actually sees.
    ///
    /// The `SEQUENCE` and `UID` come from the **invitation**, not from the stored copy, because
    /// they are what the organiser's scheduler matches on and the mail is the thing being
    /// answered. Everything else that can differ (the attendee's own display name) is read
    /// from whichever copy has it.
    ///
    /// # Errors
    ///
    /// Returns the reason the reply did not go out: no organiser to answer, no mail provider,
    /// or a submission failure. Never reports a failed send as sent: a reply the organizer
    /// never got is the exact failure this route exists to prevent.
    pub(crate) async fn send_imip_reply(
        &self,
        answer: &ClientAnswer<'_>,
        stored: Option<&Event>,
    ) -> Result<(), String> {
        let event = &answer.scheduling.message.event;
        let organizer = answer.scheduling.message.organizer().ok_or_else(|| {
            "this invitation names no organizer, so there is nobody to reply to.".to_owned()
        })?;
        let zone = self.active_zone();
        // A reply that loses its `RECURRENCE-ID` answers the whole series. Refusing is the
        // right failure: accepting every future occurrence of a meeting because the user
        // accepted one is not a smaller mistake than not answering.
        let recurrence_id =
            match &event.recurrence_id {
                Some(instance) => Some(resolve_instant_in(instance, &zone).map_err(|err| {
                    format!("the occurrence being answered cannot be placed: {err}")
                })?),
                None => None,
            };
        let document = crate::itip::reply(&crate::itip::Reply {
            uid: event.uid.as_str(),
            sequence: event.sequence,
            organizer,
            attendee: answer.attendee,
            attendee_name: attendee_name(event, answer.attendee)
                .or_else(|| stored.and_then(|stored| attendee_name(stored, answer.attendee))),
            status: answer.response.partstat(),
            starts_at: answer.starts_at,
            recurrence_id,
            stamp: now_utc().map_err(|err| format!("the reply cannot be timestamped: {err}"))?,
            comment: answer.comment,
        });

        let subject = answer
            .reply_subject
            .map_or_else(|| reply_subject(answer.original), str::to_owned);
        let draft = reply_draft(answer, organizer, subject, document)
            .ok_or_else(|| "the reply could not be addressed".to_owned())?;

        if self
            .send_draft_result(&answer.message.account, &draft)
            .await
        {
            log::info!("respond_to_invitation: the iTIP reply was submitted");
            Ok(())
        } else {
            Err(
                "your answer could not be emailed to the organizer. Check the connection and \
                 try again."
                    .to_owned(),
            )
        }
    }
}

/// Assembles the message the reply travels in.
///
/// The `From` is the **matched attendee**, not the account identity: RFC 6047 §3 has the reply
/// come from the party answering, and an organiser's scheduler that finds no `ATTENDEE` for the
/// sender is entitled to ignore it. On an aliased account those are different addresses, which
/// is the whole reason the alias is resolved before we get here.
///
/// The text body repeats the subject rather than being empty. RFC 6047 §2.4 asks a scheduling
/// message to carry a human-readable alternative for the recipients whose client does not
/// understand `text/calendar`, and one line naming the answer and the meeting is that.
fn reply_draft(
    answer: &ClientAnswer<'_>,
    organizer: &str,
    subject: String,
    document: String,
) -> Option<Draft> {
    let message_id = MessageIdHeader::new(generated_message_id()).ok()?;
    let organizer = organizer
        .strip_prefix("mailto:")
        .or_else(|| organizer.strip_prefix("MAILTO:"))
        .unwrap_or(organizer);
    let body = match answer
        .comment
        .map(str::trim)
        .filter(|note| !note.is_empty())
    {
        Some(note) => format!("{subject}\r\n\r\n{note}"),
        None => subject.clone(),
    };
    let draft = Draft::new(
        message_id,
        EmailAddress::new(answer.attendee),
        vec![EmailAddress::new(organizer)],
        subject,
        body,
    )
    .with_calendar(DraftCalendar::new(ScheduleMethod::Reply, document));
    // Thread the reply onto the invitation, so an organiser reading their mailbox sees the
    // answer beside what they sent rather than as a stray message.
    Some(match answer.original.envelope.message_id.first() {
        Some(parent) => draft.in_reply_to(parent.clone(), vec![parent.clone()]),
        None => draft,
    })
}

/// The fallback subject: the invitation's own, prefixed the RFC 5322 way.
///
/// Used when the client supplied none. It is deliberately **not** an English "Accepted: …":
/// the core has no locale (`AGENTS.md` → "Localisation is client-side"), and a reply that
/// announced the answer in a language the user does not speak would be worse than one that
/// simply quotes the meeting's own subject back. The answer itself is in the iTIP part, which
/// is what the organiser's client reads.
fn reply_subject(original: &Message) -> String {
    let subject = original.envelope.subject.as_deref().unwrap_or_default();
    if subject.is_empty() {
        "Re:".to_owned()
    } else if subject.len() >= 3 && subject[..3].eq_ignore_ascii_case("re:") {
        subject.to_owned()
    } else {
        format!("Re: {subject}")
    }
}

/// The display name the invitation gave this attendee, if it gave one.
fn attendee_name<'a>(event: &'a Event, attendee: &str) -> Option<&'a str> {
    event
        .participants
        .iter()
        .find(|participant| {
            participant
                .email
                .as_deref()
                .is_some_and(|address| addresses_match(address, attendee))
        })
        .and_then(|participant| participant.name.as_deref())
}

/// Mints the resource address a new event is stored at: `<collection><uid>.ics`.
///
/// RFC 4791 §5.3.2 lets the **client** choose the resource name, and `<uid>.ics` is the
/// universal convention, which is what makes the create's `If-None-Match: *` worth having: a
/// server that deposited its own copy of this meeting put it at the same address, so the guard
/// finds it instead of overwriting it. A random name would always succeed and always duplicate.
///
/// Returns `None` if the result is not a usable key, which the non-empty collection and suffix
/// make unreachable in practice.
fn event_href(collection: &str, uid: &str) -> Option<EventId> {
    let mut href = collection.to_owned();
    if !href.ends_with('/') {
        href.push('/');
    }
    href.push_str(&encode_path_segment(uid));
    href.push_str(".ics");
    EventId::try_from(href.as_str()).ok()
}

/// Percent-encodes `segment` as a single URI path segment (RFC 3986 §2.3).
///
/// A `UID` is opaque; Exchange writes 200 hex characters, Google writes an `@`-bearing
/// address, and nothing forbids a slash: so it is encoded rather than trusted. The `unreserved`
/// set is left alone, because for those characters (and only those) the literal byte and its
/// `%XX` form are equivalent, so this matches an address the server may have minted for the
/// same `UID`.
fn encode_path_segment(segment: &str) -> String {
    let mut out = String::with_capacity(segment.len());
    for &byte in segment.as_bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(byte));
        } else {
            out.push('%');
            out.push(hex_upper(byte >> 4));
            out.push(hex_upper(byte & 0x0f));
        }
    }
    out
}

/// The upper-case hex digit for a 0–15 nibble.
fn hex_upper(nibble: u8) -> char {
    char::from_digit(u32::from(nibble), 16).map_or('0', |digit| digit.to_ascii_uppercase())
}

/// Whether a failed write was refused by its precondition rather than by anything else.
///
/// A guarded create's `412` means the resource is already there, which for this flow is a
/// *success* with a different next step: so it is separated from every other error by the
/// engine's own failure classification, never by matching on message text.
fn is_conflict(err: &ApiError) -> bool {
    err.is_conflict()
}

#[cfg(test)]
#[path = "invitations_imip_tests.rs"]
mod tests;
