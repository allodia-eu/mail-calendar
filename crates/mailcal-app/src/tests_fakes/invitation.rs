//! A provider that serves **both** an invitation email and the calendar it lands in.
//!
//! Answering an invitation is the one action that crosses the two domains: the card is
//! anchored to a message, the write lands on an event, and the address in between comes from
//! the account's identity set. `MailFake` does no calendars and `CalendarFake` does no mail,
//! so neither can hold that seam, and the seam is exactly what breaks. Hence a third fake,
//! deliberately narrow: one mailbox, one message, one calendar, one event, and the raw iMIP
//! source that ties them together by `UID`.
//!
//! The invitation is addressed to **`info@`** while the account's identity is `me@`: the D5
//! case: so a test that passes here has proven the alias path, not just "some address was
//! sent".

use std::sync::{Arc, Mutex};

use engine_api::{AccountId, EmailAddress, Engine, TimeZoneId};
use engine_core::{
    calendar::{Calendar, Event},
    ids::{CalendarId, MailboxId},
    mail::Mailbox,
    raw::RawMime,
    sync::{JmapDataType, SyncScope, SyncState, SyncUpdate, SyncWindow},
    version::{ETag, RevisionTokens},
};
use engine_provider::{
    Capabilities, ConnectionInfo, Draft, EmailChunk, EmailStream, EventRsvp, EventWrite,
    EventWriteReceipt, OverrideSurvival, Provider, ProviderError, ProviderResult, ReplyDelivery,
    RsvpControls, RsvpResponse, ScopeSync, SubmissionReceipt, WriteGuard, WritePrecondition,
};
use meeting::{imip_source, invitation_message, invited_event};

use super::RecordingObserver;
use crate::{Account, App, Surface, Telemetry, TimeZoneInit};

// The meeting these fixtures are all about, kept next door: see that file's header for why the
// bytes and the provider are separate concerns.
#[allow(clippy::duplicate_mod)]
#[path = "invitation_meeting.rs"]
mod meeting;

/// The `(attendee, answer, comment, notify-organizer)` each RSVP carried.
pub(crate) type RecordedRsvps = Arc<Mutex<Vec<(String, RsvpResponse, Option<String>, bool)>>>;

/// The `(href, document)` each whole-document write stored; how a test sees what was put on
/// the calendar, and under what address.
pub(crate) type RecordedPuts = Arc<Mutex<Vec<(String, String)>>>;

/// Every draft the mail side was asked to submit, whole. A test reads the `calendar` part off
/// one of these to prove an iTIP `REPLY` actually left the building.
pub(crate) type RecordedSends = Arc<Mutex<Vec<Draft>>>;

/// The meeting's `UID`, shared by the email and the calendar event, which is the only thing
/// linking them, and what the RSVP lookup resolves.
pub(crate) const MEETING_UID: &str = "meeting-9@test.local";

/// The address the invitation is sent to. **Not** the account's identity (`me@…`): this is an
/// alias, so a test proves the answer goes out as the address the invitation matched.
pub(crate) const ALIAS: &str = "info@test.local";

/// The message key the invitation arrives under.
pub(crate) const MESSAGE_KEY: &str = "invite-1";

/// The event key the meeting is stored under.
pub(crate) const EVENT_KEY: &str = "/cal/meeting-9.ics";

/// A provider serving one invitation email and the calendar the meeting is in.
pub(crate) struct InvitationFake {
    caps: Capabilities,
    rsvps: RecordedRsvps,
    controls: Option<RsvpControls>,
    /// When false, the calendar is empty: the `ClientImip` case, where the mail arrived but
    /// nothing put the meeting in a calendar and there is nothing to answer on.
    materialized: bool,
    /// The `SEQUENCE` the **calendar** holds. The mail's iCalendar carries none, so it reads as
    /// `0`; anything higher here is the organiser having re-sent the meeting since.
    stored_sequence: u32,
    /// Raw RFC 5322 bytes to serve instead of [`imip_source`], for the fixture suite: real
    /// captures whose MIME shapes are the point (`tests_invitation_fixtures.rs`).
    source: Option<Arc<Vec<u8>>>,
    /// Whole-document writes, in order.
    puts: RecordedPuts,
    /// Submitted drafts, in order.
    sends: RecordedSends,
    /// The event a `put_event` created, waiting for the next event-scope delta to report it;
    /// which is how the real thing behaves too: the write returns a receipt, and the store
    /// learns the object from the reconcile that follows.
    created: Arc<Mutex<Option<Event>>>,
    /// Whether the account has a calendar provider at all. Read only by [`invitation_app`].
    has_calendar: bool,
    /// What the server reports about getting the reply to the organiser (RFC 6638 §3.2.9),
    /// carried on the RSVP receipt. `NotReported` by default, which is what most servers say,
    /// *including* auto-scheduling ones that deliver perfectly, so it is the shape every
    /// pre-existing test here was written against.
    reply_delivery: ReplyDelivery,
}

impl InvitationFake {
    /// The default: a server-scheduled transport (CalDAV/JMAP) with the meeting already on
    /// the calendar as an unanswered hold, which is what every provider in use produces.
    pub(crate) fn new() -> Self {
        Self {
            caps: Capabilities::none()
                .with_mail()
                .with_message_source()
                .with_calendars()
                .with_calendar_writes(WriteGuard::Enforced, OverrideSurvival::kept())
                .with_calendar_rsvp(SERVER_SCHEDULED)
                // The server sends the reply itself, so the core sends nothing. This is the
                // half of the pair that decides which of the two answer routes runs, and it
                // being *on* here is what keeps every existing test on the path it was written
                // for.
                .with_calendar_scheduling(),
            rsvps: Arc::new(Mutex::new(Vec::new())),
            controls: Some(SERVER_SCHEDULED),
            materialized: true,
            stored_sequence: 0,
            source: None,
            puts: Arc::new(Mutex::new(Vec::new())),
            sends: Arc::new(Mutex::new(Vec::new())),
            created: Arc::new(Mutex::new(None)),
            has_calendar: true,
            reply_delivery: ReplyDelivery::NotReported,
        }
    }

    /// A server-scheduled transport that **reports** what became of the reply it promised to
    /// send: the RFC 6638 §3.2.9 status, on the write receipt.
    ///
    /// Takes the whole verdict rather than a "did it fail" flag: telling `NotReported` from
    /// `Delivered` from `Failed` is the entire point, and a boolean would collapse the two that
    /// must never be confused.
    pub(crate) fn reporting_delivery(mut self, delivery: ReplyDelivery) -> Self {
        self.reply_delivery = delivery;
        self
    }

    /// A plain RFC 4791 calendar beside a mail account that *can* send an iMIP message; the
    /// IMAP-plus-CalDAV shape, and the one the whole client-iMIP route exists for. The meeting
    /// is on no calendar, because nothing put it there.
    pub(crate) fn without_server_scheduling(mut self) -> Self {
        self.caps = Capabilities::none()
            .with_mail()
            .with_message_source()
            .with_submission()
            .with_scheduling_submission()
            .with_calendars()
            .with_calendar_writes(WriteGuard::Enforced, OverrideSurvival::kept())
            .with_calendar_rsvp(SERVER_SCHEDULED);
        self.materialized = false;
        self
    }

    /// The same, on an account with **no calendar at all**: a bare mailbox. Nothing can be
    /// stored, and the reply is the entire answer.
    ///
    /// Drops the calendar provider from the account rather than only clearing its capabilities,
    /// because the two are different states and the code distinguishes them: a provider that
    /// advertises no calendar is still a provider a write can be attempted against, while an
    /// account with an empty `calendar_providers` has nothing to attempt at all. Modelling only
    /// the first would let a store slip through that a real bare mailbox could never reach.
    pub(crate) fn with_mail_only(mut self) -> Self {
        self.caps = Capabilities::none()
            .with_mail()
            .with_message_source()
            .with_submission()
            .with_scheduling_submission();
        self.controls = None;
        self.materialized = false;
        self.has_calendar = false;
        self
    }

    /// A mail transport that cannot put `method=` on a body part (JMAP), beside a calendar
    /// server that does not schedule: no route at all.
    pub(crate) fn with_no_route(mut self) -> Self {
        self.caps = Capabilities::none()
            .with_mail()
            .with_message_source()
            .with_submission()
            .with_calendars()
            .with_calendar_writes(WriteGuard::Enforced, OverrideSurvival::kept())
            .with_calendar_rsvp(SERVER_SCHEDULED);
        self
    }

    /// A shared handle to every document this provider was asked to store.
    pub(crate) fn puts(&self) -> RecordedPuts {
        Arc::clone(&self.puts)
    }

    /// A shared handle to every draft this provider was asked to submit.
    pub(crate) fn sends(&self) -> RecordedSends {
        Arc::clone(&self.sends)
    }

    /// The organiser has since re-sent the meeting: the calendar holds `SEQUENCE:1` while this
    /// mail is still the `SEQUENCE:0` copy (RFC 5546 §2.1.5).
    pub(crate) fn superseded_by_a_newer_revision(mut self) -> Self {
        self.stored_sequence = 1;
        self
    }

    /// Serves `raw` as the message source instead of the built-in invitation.
    ///
    /// The calendar side is left alone deliberately: a fixture's meeting is not the one this
    /// fake holds, so the card falls back to the invitation itself, which is the state a real
    /// account is in for the seconds between the mail arriving and the calendar syncing.
    pub(crate) fn with_source(mut self, raw: &'static [u8]) -> Self {
        self.source = Some(Arc::new(raw.to_vec()));
        self
    }

    /// A transport that carries a note and can be told to stay quiet; Graph and Google.
    pub(crate) fn with_full_controls(mut self) -> Self {
        self.caps = self.caps.with_calendar_rsvp(FULL);
        self.controls = Some(FULL);
        self
    }

    /// The mail arrived but the meeting is on no calendar: a bare IMAP account with no
    /// auto-scheduling server behind it.
    pub(crate) fn without_the_meeting(mut self) -> Self {
        self.materialized = false;
        self
    }

    /// A shared handle to every RSVP this provider was asked to send.
    pub(crate) fn rsvps(&self) -> RecordedRsvps {
        Arc::clone(&self.rsvps)
    }
}

/// CalDAV auto-schedule and JMAP: the answer, and nothing around it.
const SERVER_SCHEDULED: RsvpControls = RsvpControls {
    comment: false,
    suppress_notification: false,
    guard: WriteGuard::Enforced,
};

/// Graph and Google: a note for the organiser, and whether to send one at all.
const FULL: RsvpControls = RsvpControls {
    comment: true,
    suppress_notification: true,
    guard: WriteGuard::Absent,
};

#[async_trait::async_trait]
impl Provider for InvitationFake {
    fn connection_info(&self) -> ConnectionInfo {
        ConnectionInfo::new(self.caps)
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
        let inbox = Mailbox::new(MailboxId::try_from("INBOX").unwrap(), "Inbox");
        Ok(ScopeSync::new(
            SyncUpdate::snapshot(vec![inbox.clone()], [inbox.id.key().clone()].into()),
            SyncState::new("mbox-1"),
        ))
    }

    /// The mail sync is **streamed**, that is the path the app actually drives, and a fake
    /// that only overrode `sync_email` would serve nothing at all.
    fn stream_email<'a>(
        &'a self,
        _account: &'a AccountId,
        cursor: Option<&'a SyncState>,
        _window: SyncWindow,
        _fetch_batch: usize,
        _chunk_size: usize,
    ) -> EmailStream<'a> {
        let chunk = if cursor.is_some() {
            EmailChunk::additive(Vec::new(), Vec::new(), Some(0), SyncState::new("email-2"))
        } else {
            let message = invitation_message();
            let present = [message.id.key().clone()].into();
            EmailChunk::reconcile_last(vec![message], present, Some(1), SyncState::new("email-1"))
        };
        Box::pin(futures::stream::iter(vec![Ok(chunk)]))
    }

    async fn fetch_message_source(
        &self,
        _account: &AccountId,
        _message: &engine_core::mail::Message,
    ) -> ProviderResult<RawMime> {
        Ok(RawMime::new(
            self.source
                .as_ref()
                .map_or_else(imip_source, |raw| raw.as_ref().clone()),
        ))
    }

    fn calendar_scope(&self, account: &AccountId) -> SyncScope {
        SyncScope::JmapType {
            account: account.clone(),
            data_type: JmapDataType::Calendar,
        }
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
        let calendar = Calendar::new(CalendarId::try_from("/cal/").unwrap(), "Work");
        Ok(ScopeSync::new(
            SyncUpdate::snapshot(vec![calendar.clone()], [calendar.id.key().clone()].into()),
            SyncState::new("cal-1"),
        ))
    }

    async fn sync_events(
        &self,
        _account: &AccountId,
        cursor: Option<&SyncState>,
    ) -> ProviderResult<ScopeSync<Event>> {
        if cursor.is_some() {
            // The delta a post-write reconcile makes. It reports the event a `put_event` just
            // created (once) which is exactly how a real transport behaves: the write hands
            // back a receipt and no object, and the store learns the object from this call.
            let created = self.created.lock().unwrap().take();
            return Ok(ScopeSync::new(
                SyncUpdate::delta(created.into_iter().collect(), Vec::new()),
                SyncState::new("ev-2"),
            ));
        }
        if !self.materialized {
            return Ok(ScopeSync::new(
                SyncUpdate::snapshot(Vec::new(), std::collections::BTreeSet::default()),
                SyncState::new("ev-1"),
            ));
        }
        let event = invited_event(self.stored_sequence);
        Ok(ScopeSync::new(
            SyncUpdate::snapshot(vec![event.clone()], [event.id.key().clone()].into()),
            SyncState::new("ev-1"),
        ))
    }

    /// The guarded create the client-iMIP route uses to put an invitation on the calendar.
    ///
    /// Records the address and the document, and refuses anything but a create: a caller that
    /// reached here with an unconditional write would be one that could silently overwrite a
    /// copy the server had deposited, which is the failure `WritePrecondition::IfAbsent` exists
    /// to make impossible.
    async fn put_event(
        &self,
        _account: &AccountId,
        write: &EventWrite,
    ) -> ProviderResult<EventWriteReceipt> {
        if write.guard != WritePrecondition::IfAbsent {
            return Err(ProviderError::invalid_state(
                "an invitation must be stored as a guarded create",
            ));
        }
        self.puts.lock().unwrap().push((
            write.event.as_str().to_owned(),
            write.ical.as_str().to_owned(),
        ));
        let mut event = invited_event(0);
        event.id = write.event.clone();
        *self.created.lock().unwrap() = Some(event);
        Ok(EventWriteReceipt::new(
            write.event.clone(),
            write.uid.clone(),
            RevisionTokens::from_etag(ETag::new("\"v2\"")),
        ))
    }

    async fn submit_email(
        &self,
        _account: &AccountId,
        draft: &Draft,
    ) -> ProviderResult<SubmissionReceipt> {
        self.sends.lock().unwrap().push(draft.clone());
        Ok(SubmissionReceipt::filed(
            engine_core::ids::ProviderKey::new("sent-1").unwrap(),
            draft.message_id.clone(),
        ))
    }

    async fn rsvp_event(
        &self,
        _account: &AccountId,
        base: &Event,
        rsvp: &EventRsvp,
    ) -> ProviderResult<EventWriteReceipt> {
        // Every real adapter refuses a control it cannot honour through this one shared
        // check; the fake does the same, so a core test proves the refusal reaches the user
        // rather than the note being quietly dropped on the way out.
        self.controls
            .ok_or_else(|| ProviderError::invalid_state("this provider cannot RSVP"))?
            .accept(rsvp)?;
        self.rsvps.lock().unwrap().push((
            rsvp.attendee.clone(),
            rsvp.response,
            rsvp.comment.clone(),
            rsvp.notify_organizer,
        ));
        // The verdict rides the receipt, exactly as `provider-caldav` returns it after reading
        // `SCHEDULE-STATUS` off the object it just stored.
        Ok(
            EventWriteReceipt::new(base.id.clone(), base.uid.clone(), RevisionTokens::none())
                .with_reply_delivery(self.reply_delivery.clone()),
        )
    }
}

/// An app over one account whose identity is `me@`; **not** the address the invitation is
/// sent to. The alias reaches the matcher through the message's own `To:` header, which is
/// what makes an aliased invitation work with no configuration at all.
pub(crate) fn invitation_app(
    provider: InvitationFake,
    surfaces: &Arc<Mutex<Vec<Surface>>>,
) -> App<InvitationFake> {
    invitation_app_with_prefs(provider, surfaces, None)
}

/// The same, with a preferences file: so a test can set the account's standing answer to the
/// reply-fallback question, and prove a remembered choice is honoured without a prompt.
///
/// Without a path the app has nowhere to read a choice from and every account answers `Ask`,
/// which is right for the default but makes `Always`/`Never` untestable.
pub(crate) fn invitation_app_with_prefs(
    provider: InvitationFake,
    surfaces: &Arc<Mutex<Vec<Surface>>>,
    prefs_path: Option<std::path::PathBuf>,
) -> App<InvitationFake> {
    let account = Account {
        id: AccountId::try_from("acct-a").unwrap(),
        // The mail half and the calendar half are two `Provider`s over **shared** recording
        // state, because the answer crosses them: the calendar stores it and the mail sends
        // it, and a test that could not see both would only ever prove half an answer.
        providers: vec![InvitationFake {
            caps: provider.caps,
            rsvps: Arc::clone(&provider.rsvps),
            controls: provider.controls,
            materialized: provider.materialized,
            stored_sequence: provider.stored_sequence,
            source: provider.source.clone(),
            puts: Arc::clone(&provider.puts),
            sends: Arc::clone(&provider.sends),
            created: Arc::clone(&provider.created),
            has_calendar: provider.has_calendar,
            reply_delivery: provider.reply_delivery.clone(),
        }],
        calendar_providers: if provider.has_calendar {
            vec![provider]
        } else {
            Vec::new()
        },
        contact_providers: Vec::new(),
        identity: EmailAddress::new("me@test.local"),
    };
    App::new(
        Engine::open_in_memory().unwrap(),
        vec![account],
        TimeZoneInit {
            device_zone: TimeZoneId::utc(),
            prefs_path,
        },
        None,
        Arc::new(RecordingObserver {
            surfaces: Arc::clone(surfaces),
        }),
        Telemetry::off(None),
    )
}
