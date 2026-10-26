//! The [`CalendarFake`] variants: the same provider with one fact about its server changed.
//!
//! Split from `calendar.rs`, which holds the provider and its default shape, when the two
//! together crossed the 500-line limit.

use engine_api::CalendarUserAddresses;
use engine_core::calendar::Event;
use engine_provider::{Capabilities, OverrideSurvival, ProviderError, ReplyDelivery, WriteGuard};

use super::calendar::{CalendarFake, SERVER_SCHEDULED_RSVP};

impl CalendarFake {
    /// A calendar holding `events` whose RSVP write receipt carries `delivery`; what the server
    /// reported about getting the reply to the organiser.
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
            patch_failure: Some(|| ProviderError::invalid_state("the server rejected the patch")),
            ..Self::with_events(events)
        }
    }

    /// The same, but every patch's answer is lost after it was sent, so it may have landed.
    pub(crate) fn losing_patch_answers(events: Vec<Event>) -> Self {
        Self {
            patch_failure: Some(|| {
                ProviderError::needs_confirmation("the connection closed after the patch was sent")
            }),
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

    /// A writable calendar whose server **schedules**: storing a meeting is what sends its
    /// invitations (RFC 6638 auto-schedule, JMAP, Graph, Google). The only shape on which the
    /// core lets an organiser invite anyone.
    pub(crate) fn scheduling(events: Vec<Event>) -> Self {
        let fake = Self::with_events(events);
        Self {
            caps: fake.caps.with_calendar_scheduling(),
            ..fake
        }
    }

    /// The same scheduling server, naming the addresses it schedules as for this user.
    pub(crate) fn scheduling_as(events: Vec<Event>, users: CalendarUserAddresses) -> Self {
        Self {
            calendar_users: users,
            ..Self::scheduling(events)
        }
    }

    /// A CalDAV-shaped server: the calendars `(id, collection tag)` and the events on them,
    /// each calendar's events under a scope of its own.
    pub(crate) fn on_dav(calendars: &[(&str, &str)], events: Vec<Event>) -> Self {
        let calendars = calendars
            .iter()
            .map(|(id, tag)| {
                let mut calendar = engine_core::calendar::Calendar::new(
                    engine_core::ids::CalendarId::try_from(*id).unwrap(),
                    *id,
                );
                calendar.collection_tag = Some(engine_core::version::CollectionTag::new(*tag));
                calendar
            })
            .collect();
        Self {
            calendars,
            per_calendar: true,
            ..Self::with_events(events)
        }
    }

    /// The calendar each event fetch named, in order.
    pub(crate) fn event_fetches(&self) -> std::sync::Arc<std::sync::Mutex<Vec<String>>> {
        std::sync::Arc::clone(&self.event_fetches)
    }
}
