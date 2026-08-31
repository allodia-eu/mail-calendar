//! Calendar test fixtures: the [`CalendarFake`] provider and its account/app builders.
//! Split out of `tests_fakes.rs` to keep each file under the size limit; a submodule of the
//! shared `fakes` module, reusing its `RecordingObserver`.

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use engine_api::{AccountId, EmailAddress, Engine, TimeZoneId};
use engine_core::{
    calendar::{Calendar, Event},
    ids::{CalendarId, EventId, Uid},
    membership::Memberships,
    raw::RawIcal,
    sync::{SyncState, SyncUpdate},
    time::{CalendarDateTime, LocalDateTime},
    version::{ETag, RevisionTokens},
};
use engine_provider::{
    Capabilities, ConnectionInfo, DeleteTarget, DraftRecurrence, EventDeletion, EventDraft,
    EventEdit, EventRsvp, EventWriteReceipt, OverrideSurvival, PatchTarget, Provider,
    ProviderError, ProviderResult, ReplyDelivery, RsvpControls, RsvpResponse, ScopeSync,
    WriteGuard,
};

use super::RecordingObserver;
use crate::{Account, App, Surface, Telemetry, TimeZoneInit};

/// One patch the provider was asked to apply, as a test reads it back.
///
/// By value, never as a `Debug` string: the questions asked of it; "did only the title
/// change?", "what wall clock did the drag actually send?", are answered wrong by a substring
/// match on a rendering nobody controls.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecordedPatch {
    /// Whether the patch targets the whole series or one occurrence.
    pub(crate) target: PatchTarget,
    /// The new title, when the patch carries one.
    pub(crate) summary: Option<String>,
    /// The new start, in the **event's own form** as the patcher rendered it: so a test can
    /// tell an Amsterdam wall clock from a UTC instant, which is the difference a drag on a
    /// zoned event lives or dies by.
    pub(crate) start: Option<CalendarDateTime>,
    /// The new end, same terms.
    pub(crate) end: Option<CalendarDateTime>,
}

/// Every patch the provider was asked to apply, in order.
type RecordedPatches = Arc<Mutex<Vec<RecordedPatch>>>;

/// The `(attendee, answer, comment, notify-organizer)` a test reads back for each RSVP.
type RecordedRsvps = Arc<Mutex<Vec<(String, RsvpResponse, Option<String>, bool)>>>;

/// What a server-scheduled transport (CalDAV auto-schedule, JMAP) lets a user control about
/// an RSVP: the answer, and nothing else. The fake's default, because it is the harness's
/// reality; both dev accounts are server-scheduled.
const SERVER_SCHEDULED_RSVP: RsvpControls = RsvpControls {
    comment: false,
    suppress_notification: false,
    guard: WriteGuard::Enforced,
};

/// What Graph and Google let a user control: a note for the organiser, and whether to send
/// one at all.
pub(crate) const FULL_RSVP_CONTROLS: RsvpControls = RsvpControls {
    comment: true,
    suppress_notification: true,
    guard: WriteGuard::Absent,
};

/// A minimal calendar provider: one calendar collection holding a configurable set of
/// events, snapshot-on-first-sync like [`FakeProvider`]. Records the keys it is asked to
/// delete so a test can assert the app routed [`Intent::DeleteEvent`](crate::Intent) to
/// the right account's provider.
pub(crate) struct CalendarFake {
    caps: Capabilities,
    calendars: Vec<Calendar>,
    events: Vec<Event>,
    /// The uid of each draft this provider was asked to create, so a test can prove which
    /// account's provider a create was routed to.
    creations: Arc<Mutex<Vec<String>>>,
    /// The calendar key each draft was created **in**, so a test can prove a create routed to
    /// the *chosen* calendar rather than simply the first.
    create_targets: Arc<Mutex<Vec<String>>>,
    /// The repeat rule each draft carried, by value: a create that repeats has to be
    /// distinguishable from one that does not, and by more than "a field was populated".
    create_rules: Arc<Mutex<Vec<Option<DraftRecurrence>>>>,
    deletions: Arc<Mutex<Vec<String>>>,
    /// What each delete removed: the whole event, or one named occurrence. By value, for the
    /// reason [`RecordedPatch`] is: cancelling one Tuesday and cancelling the standup are
    /// different requests, and a substring match cannot tell them apart.
    delete_targets: Arc<Mutex<Vec<DeleteTarget>>>,
    /// The revision guard each delete carried, so a test can prove a delete is conditioned on
    /// the revision the caller read (CalDAV `If-Match`) rather than issued unconditionally.
    delete_guards: Arc<Mutex<Vec<Option<RevisionTokens>>>>,
    /// Every edit this provider was asked to patch: so a test can assert what an edit actually
    /// sent *by value*, not by matching a `Debug` string.
    patches: RecordedPatches,
    /// Every RSVP this provider was asked to send, by value: the answering address above
    /// all, since an alias invitation answering as the account's primary is the bug D5 exists
    /// to prevent and a `Debug` string would not pin it.
    rsvps: RecordedRsvps,
    /// What this fake advertises it can control about an RSVP. Defaults to the
    /// server-scheduled shape (CalDAV/JMAP): no note, no choosing to stay quiet.
    rsvp_controls: Option<RsvpControls>,
    /// What this fake reports about getting an RSVP to the organiser (RFC 6638 §3.2.9), on the
    /// write receipt. Defaults to [`ReplyDelivery::NotReported`], which is what the great
    /// majority of servers say; including auto-scheduling ones that deliver perfectly.
    reply_delivery: ReplyDelivery,
    /// When set, every patch fails. The write path has no outbox behind it, so a failed edit
    /// is simply a lost edit: a test has to be able to prove the app says so.
    reject_patches: bool,
    /// When set, every read fails before it reaches anything: the account is there, the network
    /// is not. The store therefore stays as empty as it was, which is the shape a first launch
    /// with no connection has and the one case where claiming a materialized window is a lie.
    unreachable: bool,
    /// When set, an **event delta** (a `sync_events` with a cursor) fails. The initial
    /// snapshot still succeeds, so this fails *only* the post-write reconcile: the path that
    /// turns a landed write into `Reconciled::Failed`, and the calendar status into `Failed`.
    reject_event_delta: bool,
    /// How many times this provider has been asked to go to the "network". The grid must paint
    /// from the store, so "did opening the calendar cost a round-trip?" has to be answerable.
    syncs: Arc<AtomicUsize>,
}

impl CalendarFake {
    /// A calendar (collection `cal`) holding one event with provider key `event_key`.
    pub(crate) fn with_event(event_key: &str) -> Self {
        let calendar = Calendar::new(CalendarId::try_from("cal").unwrap(), "Calendar");
        let event = Event::new(
            EventId::try_from(event_key).unwrap(),
            Uid::new(format!("{event_key}@h")).unwrap(),
            Memberships::of_one(CalendarId::try_from("cal").unwrap()),
            CalendarDateTime::utc(LocalDateTime::new(2026, 6, 1, 9, 0, 0).unwrap()),
        );
        Self {
            caps: Capabilities::none()
                .with_calendars()
                .with_calendar_writes(WriteGuard::Enforced, OverrideSurvival::kept())
                .with_calendar_rsvp(SERVER_SCHEDULED_RSVP),
            calendars: vec![calendar],
            events: vec![event],
            creations: Arc::new(Mutex::new(Vec::new())),
            create_rules: Arc::new(Mutex::new(Vec::new())),
            delete_targets: Arc::new(Mutex::new(Vec::new())),
            create_targets: Arc::new(Mutex::new(Vec::new())),
            rsvps: Arc::new(Mutex::new(Vec::new())),
            rsvp_controls: Some(SERVER_SCHEDULED_RSVP),
            deletions: Arc::new(Mutex::new(Vec::new())),
            delete_guards: Arc::new(Mutex::new(Vec::new())),
            patches: Arc::new(Mutex::new(Vec::new())),
            reply_delivery: ReplyDelivery::NotReported,
            reject_patches: false,
            unreachable: false,
            reject_event_delta: false,
            syncs: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// A calendar holding exactly `events`: for tests that need events positioned
    /// relative to *today*, so they do not start failing when the year rolls over.
    pub(crate) fn with_events(events: Vec<Event>) -> Self {
        Self {
            events,
            ..Self::with_event("unused")
        }
    }

    /// A writable account advertising several named calendars (each id doubling as its name)
    /// and no events: for asserting a create routes to the **chosen** calendar, not the first.
    pub(crate) fn with_calendars(ids: &[&str]) -> Self {
        let calendars = ids
            .iter()
            .map(|id| Calendar::new(CalendarId::try_from(*id).unwrap(), *id))
            .collect();
        Self {
            calendars,
            events: Vec::new(),
            ..Self::with_event("unused")
        }
    }

    /// The same, but the RSVP write receipt carries `delivery`; what the server reported
    /// about getting the reply to the organiser.
    ///
    /// This is the one knob that turns a stored answer into a *question for the user*, so it
    /// takes the whole verdict rather than a bool: "the server said nothing" and "the server
    /// said 5.2" must be distinguishable in a test, because conflating them is exactly the bug
    /// (`docs/invitations.md`).
    pub(crate) fn reporting_delivery(events: Vec<Event>, delivery: ReplyDelivery) -> Self {
        Self {
            reply_delivery: delivery,
            ..Self::with_events(events)
        }
    }

    /// The same, but every patch is rejected: the server said no.
    pub(crate) fn rejecting_writes(events: Vec<Event>) -> Self {
        Self {
            reject_patches: true,
            ..Self::with_events(events)
        }
    }

    /// A calendar account that cannot be reached: no network. Its capabilities are unchanged, so
    /// the app still expects a calendar from it; it simply never gets one.
    pub(crate) fn unreachable() -> Self {
        Self {
            unreachable: true,
            ..Self::with_events(Vec::new())
        }
    }

    /// A writable account whose **post-write reconcile fails**: the create/edit/delete lands,
    /// but the event delta the engine runs to confirm it comes back an error, so the write
    /// settles as `Reconciled::Failed` and the calendar status as `Failed`.
    pub(crate) fn failing_reconcile(events: Vec<Event>) -> Self {
        Self {
            reject_event_delta: true,
            ..Self::with_events(events)
        }
    }

    /// A **read-only** calendar account: it syncs calendars and events but advertises no
    /// calendar writes, so `Capabilities::calendar_write_guard()` is `None` and every row it
    /// contributes must come back `can_write: false`. This is the subscribed-feed / read-only
    /// share case a client hides its edit affordances for.
    pub(crate) fn read_only(events: Vec<Event>) -> Self {
        Self {
            caps: Capabilities::none().with_calendars(),
            ..Self::with_events(events)
        }
    }

    /// A writable calendar account whose server **discards** what the user changed about a
    /// single occurrence when the series is edited: the Graph shape, and the case a client
    /// has to warn about before it lets the edit through.
    pub(crate) fn losing_overrides(events: Vec<Event>) -> Self {
        Self {
            caps: Capabilities::none()
                .with_calendars()
                .with_calendar_writes(
                    WriteGuard::Enforced,
                    OverrideSurvival {
                        survives_time_change: false,
                        survives_rule_change: false,
                        clobbers_own_fields: false,
                    },
                )
                .with_calendar_rsvp(SERVER_SCHEDULED_RSVP),
            ..Self::with_events(events)
        }
    }

    /// A shared handle to the uid of each draft this provider was asked to create.
    pub(crate) fn creations(&self) -> Arc<Mutex<Vec<String>>> {
        Arc::clone(&self.creations)
    }

    /// A shared handle to the calendar key each create was routed into.
    pub(crate) fn create_targets(&self) -> Arc<Mutex<Vec<String>>> {
        Arc::clone(&self.create_targets)
    }

    /// A shared handle to the repeat rule each create carried.
    pub(crate) fn create_rules(&self) -> Arc<Mutex<Vec<Option<DraftRecurrence>>>> {
        Arc::clone(&self.create_rules)
    }

    /// A shared handle to what each delete removed: the event, or one occurrence.
    pub(crate) fn delete_targets(&self) -> Arc<Mutex<Vec<DeleteTarget>>> {
        Arc::clone(&self.delete_targets)
    }

    /// A shared handle to the event keys this provider was asked to delete.
    pub(crate) fn deletions(&self) -> Arc<Mutex<Vec<String>>> {
        Arc::clone(&self.deletions)
    }

    /// A shared handle to the revision guard each delete carried.
    pub(crate) fn delete_guards(&self) -> Arc<Mutex<Vec<Option<RevisionTokens>>>> {
        Arc::clone(&self.delete_guards)
    }

    /// A shared handle to every edit this provider was asked to patch.
    pub(crate) fn patches(&self) -> RecordedPatches {
        Arc::clone(&self.patches)
    }

    /// A shared handle to the `(attendee, answer, comment, notify)` of each RSVP.
    pub(crate) fn rsvps(&self) -> RecordedRsvps {
        Arc::clone(&self.rsvps)
    }

    /// A shared handle to this provider's round-trip count.
    pub(crate) fn syncs(&self) -> Arc<AtomicUsize> {
        Arc::clone(&self.syncs)
    }
}

#[async_trait::async_trait]
impl Provider for CalendarFake {
    fn connection_info(&self) -> ConnectionInfo {
        ConnectionInfo::new(self.caps)
    }

    async fn sync_calendars(
        &self,
        _account: &AccountId,
        cursor: Option<&SyncState>,
    ) -> ProviderResult<ScopeSync<Calendar>> {
        self.syncs.fetch_add(1, Ordering::Relaxed);
        if self.unreachable {
            return Err(ProviderError::invalid_state(
                "the server could not be reached",
            ));
        }
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

    async fn sync_events(
        &self,
        _account: &AccountId,
        cursor: Option<&SyncState>,
    ) -> ProviderResult<ScopeSync<Event>> {
        self.syncs.fetch_add(1, Ordering::Relaxed);
        if self.unreachable {
            return Err(ProviderError::invalid_state(
                "the server could not be reached",
            ));
        }
        if cursor.is_some() {
            if self.reject_event_delta {
                return Err(ProviderError::invalid_state("the event delta failed"));
            }
            return Ok(ScopeSync::new(
                SyncUpdate::delta(Vec::new(), Vec::new()),
                SyncState::new("ev-2"),
            ));
        }
        let present = self.events.iter().map(|e| e.id.key().clone()).collect();
        Ok(ScopeSync::new(
            SyncUpdate::snapshot(self.events.clone(), present),
            SyncState::new("ev-1"),
        ))
    }

    async fn create_event(
        &self,
        _account: &AccountId,
        draft: &EventDraft,
    ) -> ProviderResult<EventWriteReceipt> {
        self.creations
            .lock()
            .unwrap()
            .push(draft.uid.as_str().to_owned());
        self.create_targets
            .lock()
            .unwrap()
            .push(draft.calendar.key().as_str().to_owned());
        self.create_rules
            .lock()
            .unwrap()
            .push(draft.recurrence.clone());
        let href = format!("/cal/{}.ics", draft.uid.as_str());
        let event = EventId::try_from(href.as_str())
            .unwrap_or_else(|_| EventId::try_from("/cal/event.ics").unwrap());
        Ok(EventWriteReceipt::new(
            event,
            draft.uid.clone(),
            RevisionTokens::none(),
        ))
    }

    async fn rsvp_event(
        &self,
        _account: &AccountId,
        base: &Event,
        rsvp: &EventRsvp,
    ) -> ProviderResult<EventWriteReceipt> {
        // The real adapters all refuse a control they cannot honour, through one shared
        // `RsvpControls::accept`. The fake does the same, so a core-side test can prove the
        // refusal reaches the user rather than the note being quietly dropped.
        self.rsvp_controls
            .ok_or_else(|| ProviderError::invalid_state("this provider cannot RSVP"))?
            .accept(rsvp)?;
        self.rsvps.lock().unwrap().push((
            rsvp.attendee.clone(),
            rsvp.response,
            rsvp.comment.clone(),
            rsvp.notify_organizer,
        ));
        // The verdict rides the receipt the write returns, exactly as `provider-caldav` does
        // after reading `SCHEDULE-STATUS` off the stored object.
        Ok(
            EventWriteReceipt::new(base.id.clone(), base.uid.clone(), RevisionTokens::none())
                .with_reply_delivery(self.reply_delivery.clone()),
        )
    }

    async fn patch_event(
        &self,
        _account: &AccountId,
        base: &Event,
        edit: &EventEdit,
    ) -> ProviderResult<EventWriteReceipt> {
        self.patches.lock().unwrap().push(RecordedPatch {
            target: edit.target.clone(),
            summary: edit.patch.summary_edit().map(str::to_owned),
            start: edit.patch.start_edit().cloned(),
            end: edit.patch.end_edit().cloned(),
        });
        if self.reject_patches {
            return Err(ProviderError::invalid_state(
                "the server rejected the patch",
            ));
        }
        Ok(EventWriteReceipt::new(
            base.id.clone(),
            base.uid.clone(),
            RevisionTokens::none(),
        ))
    }

    async fn delete_event(
        &self,
        _account: &AccountId,
        _base: Option<&Event>,
        deletion: &EventDeletion,
    ) -> ProviderResult<()> {
        self.deletions
            .lock()
            .unwrap()
            .push(deletion.event.key().as_str().to_owned());
        self.delete_guards
            .lock()
            .unwrap()
            .push(deletion.guard.clone());
        self.delete_targets
            .lock()
            .unwrap()
            .push(deletion.target.clone());
        Ok(())
    }
}
