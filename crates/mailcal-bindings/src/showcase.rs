//! The **showcase** providers: in-memory mail + calendar adapters that serve the richly-seeded
//! screenshot dataset ([`crate::showcase_data`]). They drive every client's screenshot mode
//! (`MAILCAL_SHOWCASE`), so store screenshots show a realistic app with no real account, and no
//! risk of personal mail leaking into a screenshot.
//!
//! Deliberately separate from [`crate::demo`]: that tiny four-row fixture is what the CI verify
//! gates (`MailcalVerify`, the FFI-loop test) assert on, so it must not change. These providers
//! mirror its shape (a JMAP-style account-wide snapshot) but carry the screenshot content.

use std::{
    collections::HashMap,
    sync::{Mutex, MutexGuard},
};

use engine_api::AccountId;
use engine_core::{
    calendar::{Calendar, Event},
    mail::{Mailbox, Message},
    raw::RawMime,
    scheduling::addresses_match,
    sync::{JmapDataType, SyncScope, SyncState, SyncUpdate, SyncWindow},
};
use engine_provider::{
    Capabilities, ConnectionInfo, EmailChunk, EmailStream, EventRsvp, EventWriteReceipt, Provider,
    ProviderError, ProviderResult, RsvpControls, ScopeSync, WriteGuard,
};

/// A mail provider serving one account's showcase mailbox: several folders of realistic
/// messages, each with a tailored body, in one account-wide snapshot. The screenshot twin of
/// [`crate::demo::DemoProvider`].
pub(crate) struct ShowcaseMailProvider {
    caps: Capabilities,
    mailboxes: Vec<Mailbox>,
    messages: Vec<Message>,
    /// Provider key → the message's full raw MIME source, served by `fetch_message_source`.
    bodies: HashMap<String, Vec<u8>>,
}

impl ShowcaseMailProvider {
    pub(crate) fn new(
        mailboxes: Vec<Mailbox>,
        messages: Vec<Message>,
        bodies: HashMap<String, Vec<u8>>,
    ) -> Self {
        Self {
            caps: Capabilities::none().with_mail().with_message_source(),
            mailboxes,
            messages,
            bodies,
        }
    }
}

#[async_trait::async_trait]
impl Provider for ShowcaseMailProvider {
    fn connection_info(&self) -> ConnectionInfo {
        ConnectionInfo::new(self.caps)
    }

    fn mailbox_scope(&self, account: &AccountId) -> SyncScope {
        SyncScope::JmapType {
            account: account.clone(),
            data_type: JmapDataType::Mailbox,
        }
    }

    fn email_scope(&self, account: &AccountId) -> SyncScope {
        SyncScope::JmapType {
            account: account.clone(),
            data_type: JmapDataType::Email,
        }
    }

    async fn sync_mailboxes(
        &self,
        _account: &AccountId,
        cursor: Option<&SyncState>,
    ) -> ProviderResult<ScopeSync<Mailbox>> {
        if cursor.is_some() {
            return Ok(ScopeSync::new(
                SyncUpdate::delta(Vec::new(), Vec::new()),
                SyncState::new("mbox-2"),
            ));
        }
        let present = self.mailboxes.iter().map(|m| m.id.key().clone()).collect();
        Ok(ScopeSync::new(
            SyncUpdate::snapshot(self.mailboxes.clone(), present),
            SyncState::new("mbox-1"),
        ))
    }

    fn stream_email<'a>(
        &'a self,
        _account: &AccountId,
        cursor: Option<&'a SyncState>,
        _window: SyncWindow,
        _fetch_batch: usize,
        _chunk_size: usize,
    ) -> EmailStream<'a> {
        let chunk = if cursor.is_some() {
            EmailChunk::additive(Vec::new(), Vec::new(), None, SyncState::new("email-2"))
        } else {
            let present = self.messages.iter().map(|m| m.id.key().clone()).collect();
            EmailChunk::reconcile_last(
                self.messages.clone(),
                present,
                Some(self.messages.len()),
                SyncState::new("email-1"),
            )
        };
        Box::pin(futures::stream::iter(vec![Ok(chunk)]))
    }

    async fn fetch_message_source(
        &self,
        _account: &AccountId,
        message: &Message,
    ) -> ProviderResult<RawMime> {
        let key = message.id.key().as_str();
        let bytes = self
            .bodies
            .get(key)
            .cloned()
            .unwrap_or_else(|| fallback_body(message));
        Ok(RawMime::new(bytes))
    }
}

/// A plain `text/html` body built from a message's preview, for the messages that carry no
/// tailored source of their own: so every message still opens to something readable.
fn fallback_body(message: &Message) -> Vec<u8> {
    let preview = message.preview.as_deref().unwrap_or("");
    format!(
        "Content-Type: text/html; charset=utf-8\r\n\r\n\
         <div style=\"font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.5;\">\
         <p>{preview}</p></div>"
    )
    .into_bytes()
}

/// A calendar provider serving one account's showcase calendars and events in one snapshot.
/// The calendar twin of [`ShowcaseMailProvider`]; mirrors the shape of the engine's own
/// calendar test fake. The account has several calendars (Work, Personal, Family), each in its
/// own colour, so the grid shows a real work-and-private-life mix.
///
/// # Why it answers invitations
///
/// The dataset seeds a meeting invitation, and the card's Accept / Maybe / Decline row appears
/// only where the account's calendar provider advertises
/// [`Capabilities::calendar_rsvp`]: a transport that cannot answer must show *no* buttons rather
/// than dead ones (`docs/invitations.md`). Advertising the capability without honouring it would
/// have been the same lie in the other direction: three buttons that look live and fail on tap.
/// So the events live behind a `Mutex` and an RSVP really does patch the attendee's `PARTSTAT`,
/// the way the harness's own server would.
pub(crate) struct ShowcaseCalendarProvider {
    caps: Capabilities,
    calendars: Vec<Calendar>,
    /// The events, plus the revision that changes whenever one of them does; see
    /// [`Self::sync_events`] for why a bare `Vec` is not enough here.
    events: Mutex<(Vec<Event>, u64)>,
}

impl ShowcaseCalendarProvider {
    pub(crate) fn new(calendars: Vec<Calendar>, events: Vec<Event>) -> Self {
        Self {
            caps: Capabilities::none().with_calendars().with_calendar_rsvp(
                // No note, and no way to keep the organiser out of it: this is a server-scheduled
                // transport in the shape of CalDAV auto-schedule and JMAP, which is what the
                // harness and the real accounts look like. The controls the seeded card offers
                // are therefore the ones a real account offers too.
                RsvpControls {
                    comment: false,
                    suppress_notification: false,
                    // Nothing else writes to this in-memory diary, so there is no lost update to
                    // guard against, and claiming `Enforced` would advertise a precondition that
                    // is never checked.
                    guard: WriteGuard::Absent,
                },
            )
            // The same shape again: a server-scheduled transport answers the organiser itself,
            // so the card offers its three buttons without the core needing a mail provider
            // that can send an iMIP reply. The showcase has none, and a screenshot of an
            // invitation with no buttons is not the product.
            .with_calendar_scheduling(),
            calendars,
            events: Mutex::new((events, 0)),
        }
    }

    /// The event list and its revision. Panicking on a poisoned lock is the same posture as the
    /// rest of the core: a poisoned mutex means another thread panicked mid-write, and carrying on
    /// over a half-applied RSVP would be worse than stopping.
    fn events(&self) -> MutexGuard<'_, (Vec<Event>, u64)> {
        self.events.lock().expect("showcase events poisoned")
    }
}

#[async_trait::async_trait]
impl Provider for ShowcaseCalendarProvider {
    fn connection_info(&self) -> ConnectionInfo {
        ConnectionInfo::new(self.caps)
    }

    async fn sync_calendars(
        &self,
        _account: &AccountId,
        cursor: Option<&SyncState>,
    ) -> ProviderResult<ScopeSync<Calendar>> {
        if cursor.is_some() {
            return Ok(ScopeSync::new(
                SyncUpdate::delta(Vec::new(), Vec::new()),
                SyncState::new("cal-2"),
            ));
        }
        let present = self.calendars.iter().map(|c| c.id.key().clone()).collect();
        Ok(ScopeSync::new(
            SyncUpdate::snapshot(self.calendars.clone(), present),
            SyncState::new("cal-1"),
        ))
    }

    /// Serves the events, keyed on a revision that an [`Self::rsvp_event`] bumps.
    ///
    /// The mailbox and calendar scopes above can short-circuit any second sync to an empty delta,
    /// because nothing ever changes them. This one **can** change: answering an invitation patches
    /// an attendee's status, and the engine's post-write reconcile is a *sync*, so a provider that
    /// always reported "nothing new" would leave the store holding the pre-answer copy. The card
    /// would then keep saying "You haven't answered" after the user had: the exact contradiction
    /// the card reads the calendar's copy to avoid. So a cursor naming the current revision is a
    /// no-op, and any other cursor re-serves the whole (tiny, in-memory) snapshot.
    async fn sync_events(
        &self,
        _account: &AccountId,
        cursor: Option<&SyncState>,
    ) -> ProviderResult<ScopeSync<Event>> {
        let (events, revision) = &*self.events();
        let state = SyncState::new(format!("ev-{revision}"));
        if cursor.is_some_and(|cursor| *cursor == state) {
            return Ok(ScopeSync::new(
                SyncUpdate::delta(Vec::new(), Vec::new()),
                state,
            ));
        }
        let present = events.iter().map(|e| e.id.key().clone()).collect();
        Ok(ScopeSync::new(
            SyncUpdate::snapshot(events.clone(), present),
            state,
        ))
    }

    /// Answers an invitation by patching the named attendee's participation status, the way an
    /// auto-scheduling server does; then bumps the revision so the reconcile that follows the
    /// write actually brings the new status back.
    ///
    /// `rsvp.attendee` is used **verbatim** and matched with the engine's `addresses_match`, never
    /// `==`: it is the address the invitation matched, which on an aliased account is not the
    /// account's identity, and iCalendar cases domains freely.
    async fn rsvp_event(
        &self,
        _account: &AccountId,
        base: &Event,
        rsvp: &EventRsvp,
    ) -> ProviderResult<EventWriteReceipt> {
        let (events, revision) = &mut *self.events();
        let event = events
            .iter_mut()
            .find(|event| event.uid == rsvp.uid)
            .ok_or_else(|| ProviderError::invalid_state("no such event in the showcase diary"))?;
        let attendee = event
            .participants
            .iter_mut()
            .find(|participant| {
                participant
                    .email
                    .as_deref()
                    .is_some_and(|email| addresses_match(email, &rsvp.attendee))
            })
            .ok_or_else(|| {
                ProviderError::invalid_state("that address is not an attendee of this meeting")
            })?;
        attendee.participation_status = rsvp.response.status();
        *revision += 1;
        Ok(EventWriteReceipt::new(
            base.id.clone(),
            base.uid.clone(),
            base.revisions.clone(),
        ))
    }
}
