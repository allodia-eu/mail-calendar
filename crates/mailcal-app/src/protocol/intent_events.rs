//! The calendar-event half of the inbound protocol ([`EventIntent`]), which
//! [`Intent::Events`](super::Intent::Events) carries.
//!
//! A family of its own for the reason [`FolderIntent`](super::FolderIntent) is one: `intent.rs`
//! is at its 500-line limit. Every write here names its event by an [`EventRef`], the account
//! and the provider key together.

use engine_api::{Invitee, LocalDateTime};
use mailcal_account::{EventDrag, EventEdit};

use crate::reference::EventRef;

/// A write to a stored calendar event, or a new one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventIntent {
    /// Create a calendar event, then refresh the agenda.
    ///
    /// `account`/`calendar` are the client's calendar-picker choice: the owning account id and
    /// the `CalendarRow.id` key. Both `None` files the event in the default writable account's
    /// first calendar (the legacy behaviour). `all_day` selects the event form and how the times
    /// are read; `timezone` (when set) creates a timed event in that zone; `notes` is the
    /// description; `location` is the place. A create is the one write that sets a location
    /// from nothing: an edit reshapes it through [`EventIntent::Update`].
    Create {
        /// The event title.
        title: String,
        /// A timed event's start: a **wall clock** (`2026-07-01T10:00:00`) when `timezone` is set,
        /// else an RFC 3339 UTC instant (`2026-07-01T10:00:00Z`). For an all-day event, the start
        /// date (`2026-07-01`).
        start: String,
        /// The end, same terms as `start`. For an all-day event the end date is **exclusive**
        /// (a one-day event on the 1st ends on the 2nd).
        end: String,
        /// The owning account of the chosen calendar, or `None` for the default.
        account: Option<String>,
        /// The chosen calendar's row key (`CalendarRow.id`), or `None` for the account's first.
        calendar: Option<String>,
        /// Whether this is an all-day event (changes how `start`/`end` are parsed).
        all_day: bool,
        /// The IANA zone a timed event is created in: the device's zone, so it reads back the
        /// same clock on edit. `None`/empty falls back to UTC (and `start`/`end` are UTC
        /// instants).
        timezone: Option<String>,
        /// The description, if any.
        notes: Option<String>,
        /// The location, if any.
        location: Option<String>,
        /// How the event repeats, or `None` for a one-off. Changing the rule afterwards goes
        /// through [`EventIntent::Update`].
        recurrence: Option<mailcal_account::SimpleRecurrence>,
        /// The people to invite. `None` creates an appointment; the organiser is derived from the
        /// selected account. `Some` must contain at least one invitee.
        invitees: Option<Vec<Invitee>>,
    },
    /// Edit a stored calendar event; retitle, move, resize, change its notes, location or roster;
    /// then refresh the agenda.
    ///
    /// The write is a provider-neutral patch, so the adapter applies only the changed
    /// properties and the untouched recurrence rule, invitees, alarms and timezone survive.
    /// Rebuilding the document instead, which is all [`EventIntent::Create`] can do;
    /// would delete every one of them and report success.
    ///
    /// Rides the same inline-await write path as create/delete, its outcome surfaced through
    /// [`CalendarWriteStatus`](crate::CalendarWriteStatus) (`Saving` → `Saved`/`Failed`). Like
    /// them it is **not durable offline** yet: a failed edit stays failed rather than queueing
    /// (the shared outbox follow-up).
    Update {
        /// The event to edit; its account and provider key bound together.
        event: EventRef,
        /// What to change, in the event's **own wall clock**; see
        /// [`EventEdit`](mailcal_account::EventEdit). A move must not convert a zoned or
        /// all-day event, so the edit names wall-clock times, never UTC instants.
        edit: EventEdit,
    },
    /// Move or resize a stored calendar event by **dragging** it on the grid, then refresh the
    /// agenda.
    ///
    /// A sibling of [`EventIntent::Update`] rather than a special case of it, because a drag
    /// says something the editor cannot: *this far*, not *to here*. The client sends a signed
    /// offset in whole days and minutes and the core applies it to the event's own wall clock;
    /// so nothing about the zone the grid was drawn in reaches the write, a segment clipped to
    /// its day column needs no absolute anchor, and a move preserves its duration exactly. The
    /// reasoning in full: [`mailcal_account::apply_event_drag`].
    ///
    /// **Refused unless the event is the user's own**; their appointment, or a meeting they
    /// organise. A client gates the gesture on `TimedSegment::can_move`; this checks the same
    /// rule again, because a write must not trust a caller.
    ///
    /// Rides the same inline-await patch path as [`EventIntent::Update`], with the same
    /// [`CalendarWriteStatus`](crate::CalendarWriteStatus) reporting and the same lack of an
    /// offline outbox.
    Move {
        /// The event to move; its account and provider key bound together.
        event: EventRef,
        /// The drag: which edges moved, how far, and which occurrence of a series it was.
        drag: EventDrag,
    },
    /// Delete a calendar event, or one occurrence of it, then refresh the agenda.
    Delete {
        /// The event to delete; its account and provider key (resource href) bound
        /// together, so a key two accounts share affects only the owning account.
        event: EventRef,
        /// Which occurrence of a recurring event to remove, named by its **original**
        /// start. `None` deletes the whole series. There is no default on purpose, for the
        /// same reason [`EventEdit`](mailcal_account::EventEdit)`::occurrence` has none:
        /// cancelling one Tuesday and cancelling the standup are different requests, and
        /// only the user knows which they meant: so ask them.
        occurrence: Option<LocalDateTime>,
    },
}
