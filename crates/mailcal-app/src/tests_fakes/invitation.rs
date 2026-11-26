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

use engine_api::{AccountId, CalendarSync, CalendarUserAddresses, CalendarWrites};
use engine_core::{
    calendar::{Calendar, Event},
    ids::{CalendarId, EventId, MailboxId, Uid},
    mail::Mailbox,
    raw::RawMime,
    sync::{JmapDataType, MailboxWindows, SyncScope, SyncState, SyncUpdate},
    version::{ETag, RevisionTokens},
};
use engine_provider::{
    CalendarListing, Capabilities, ConnectionInfo, Draft, EmailChunk, EmailStream, EventRsvp,
    EventWrite, EventWriteReceipt, OverrideSurvival, Provider, ProviderError, ProviderResult,
    ReplyDelivery, RsvpControls, RsvpResponse, ScopeSync, SubmissionReceipt, WriteGuard,
    WritePrecondition,
};
use meeting::{SOON, imip_source, invitation_message, invited_event};

use super::RecordingObserver;

#[path = "invitation_app.rs"]
mod app;
#[path = "invitation_linked.rs"]
mod linked;
// The meeting these fixtures are all about, kept next door: see that file's header for why the
// bytes and the provider are separate concerns.
#[path = "invitation_meeting.rs"]
mod meeting;
#[path = "invitation_routes.rs"]
mod routes;
pub(crate) use app::{invitation_app, invitation_app_with_prefs};
pub(crate) use linked::{
    CALENDAR_ACCOUNT, OTHER_MAILBOX, linked_invitation_app, shared_calendar_app,
};

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

/// The resource name the fake's adapter gives a new event: opaque, so nothing derived from the
/// `UID` can arrive at it.
pub(crate) const MINTED_NAME: &str = "minted-7f3a9c.ics";

/// A provider serving one invitation email and the calendar the meeting is in.
pub(crate) struct InvitationFake {
    caps: Capabilities,
    rsvps: RecordedRsvps,
    controls: Option<RsvpControls>,
    /// When false, the calendar is empty: the `ClientImip` case, where the mail arrived but
    /// nothing put the meeting in a calendar and there is nothing to answer on.
    pub(crate) materialized: bool,
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
    /// What the calendar server says it schedules as (RFC 6638 §2.4.1). `Unknown` by default,
    /// which leaves the route to the capabilities alone.
    calendar_users: CalendarUserAddresses,
    /// How the calendar takes a write.
    writes: Writes,
    /// How many days from today the meeting falls.
    days_ahead: i64,
}

/// How [`InvitationFake`]'s calendar takes a write.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Writes {
    /// It names a new event itself, as CalDAV does, and stores what it is sent.
    Named,
    /// Its server names new events, so the adapter has no address to give (the engine's
    /// default answer).
    ServerNamed,
    /// It refuses every write with a body that names the resource, as a CalDAV error body can.
    Refused,
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
            calendar_users: CalendarUserAddresses::Unknown,
            writes: Writes::Named,
            days_ahead: SOON,
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
        _window: MailboxWindows,
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
        Ok(RawMime::new(self.source.as_ref().map_or_else(
            || imip_source(self.days_ahead),
            |raw| raw.as_ref().clone(),
        )))
    }

    async fn submit_email(
        &self,
        _account: &AccountId,
        draft: &Draft,
        hand_over: &engine_provider::HandOver<'_>,
    ) -> ProviderResult<SubmissionReceipt> {
        self.sends.lock().unwrap().push(draft.clone());
        let handed = hand_over.commit().await?;
        Ok(SubmissionReceipt::filed(
            engine_core::ids::ProviderKey::new("sent-1").unwrap(),
            draft.message_id.clone(),
            &handed,
        ))
    }
}

#[async_trait::async_trait]
impl CalendarSync for InvitationFake {
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
    ) -> ProviderResult<CalendarListing> {
        if cursor.is_some() {
            return Ok(ScopeSync::new(
                SyncUpdate::delta(Vec::new(), Vec::new()),
                SyncState::new("cal-2"),
            )
            .into());
        }
        let calendar = Calendar::new(CalendarId::try_from("/cal/").unwrap(), "Work");
        Ok(ScopeSync::new(
            SyncUpdate::snapshot(vec![calendar.clone()], [calendar.id.key().clone()].into()),
            SyncState::new("cal-1"),
        )
        .into())
    }

    async fn sync_events(
        &self,
        _account: &AccountId,
        _calendar: &CalendarId,
        cursor: Option<&SyncState>,
        _window: engine_api::Horizon,
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
        let event = invited_event(self.stored_sequence, self.days_ahead);
        Ok(ScopeSync::new(
            SyncUpdate::snapshot(vec![event.clone()], [event.id.key().clone()].into()),
            SyncState::new("ev-1"),
        ))
    }
}

impl engine_api::MailboxWrites for InvitationFake {}

#[async_trait::async_trait]
impl CalendarWrites for InvitationFake {
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
        if self.writes == Writes::Refused {
            return Err(refusal_naming(write.event.as_str()));
        }
        if write.guard != WritePrecondition::IfAbsent {
            return Err(ProviderError::invalid_state(
                "an invitation must be stored as a guarded create",
            ));
        }
        self.puts.lock().unwrap().push((
            write.event.as_str().to_owned(),
            write.ical.as_str().to_owned(),
        ));
        // Listed in the server's form, whatever the written address spelled: Stalwart, SabreDAV
        // and Nextcloud all list a resource written at `…%40….ics` as `…@….ics`.
        let mut event = invited_event(0, self.days_ahead);
        event.id = EventId::try_from(write.event.as_str().replace("%40", "@").as_str()).unwrap();
        *self.created.lock().unwrap() = Some(event);
        Ok(EventWriteReceipt::new(
            write.event.clone(),
            write.uid.clone(),
            RevisionTokens::from_etag(ETag::new("\"v2\"")),
        ))
    }

    /// A stand-in for the adapter's naming rule, not the rule: a name no host could derive from
    /// the `UID`, so a core that spelled the address itself would write somewhere else. The
    /// engine proves the real rule, offline and against live servers.
    fn new_event_id(&self, calendar: &CalendarId, _uid: &Uid) -> ProviderResult<EventId> {
        if self.writes == Writes::ServerNamed {
            return Err(ProviderError::invalid_state(
                "provider does not support naming new events",
            ));
        }
        Ok(EventId::try_from(format!("{}{MINTED_NAME}", calendar.as_str()).as_str()).unwrap())
    }

    async fn calendar_user_addresses(
        &self,
        _account: &AccountId,
    ) -> ProviderResult<CalendarUserAddresses> {
        Ok(self.calendar_users.clone())
    }

    async fn rsvp_event(
        &self,
        _account: &AccountId,
        base: &Event,
        rsvp: &EventRsvp,
    ) -> ProviderResult<EventWriteReceipt> {
        if self.writes == Writes::Refused {
            return Err(refusal_naming(base.id.as_str()));
        }
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

/// A refusal shaped as `provider-caldav` renders one: the status and the server's body, which
/// here names the resource the write addressed.
fn refusal_naming(href: &str) -> ProviderError {
    ProviderError::permanent(format!("CalDAV HTTP 403: <d:href>{href}</d:href>"))
}
