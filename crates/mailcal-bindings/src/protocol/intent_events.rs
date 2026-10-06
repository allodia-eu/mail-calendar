//! The calendar-event half of the FFI intent surface, nested behind one `Intent::Events` variant
//! for the reason `FolderIntent` is: [`Intent`](super::Intent) is at its 500-line limit. It
//! mirrors the core's own `EventIntent`.

use crate::{EventEdge, RecurrenceChange, SimpleRecurrence};

/// A write to a stored calendar event, or a new one.
#[derive(uniffi::Enum)]
pub enum EventIntent {
    /// Create a calendar event, then refresh the agenda.
    ///
    /// For a **timed** event (`all_day = false`) `start`/`end` are RFC 3339 UTC instants, for
    /// an **all-day** event they are `YYYY-MM-DD` dates and the end is **exclusive** (a one-day
    /// event on the 1st ends on the 2nd: the client converts its inclusive on-screen end).
    /// `account`/`calendar` are the picker's choice (the owning account id and `CalendarRow.id`);
    /// both `None` files it in the default writable account's first calendar.
    Create {
        /// The event title.
        title: String,
        /// The start: a **wall clock** (`2026-07-01T10:00:00`) when `timezone` is set, else a UTC
        /// instant; a date (`2026-07-01`) when all-day.
        start: String,
        /// The end; same terms as `start`; exclusive date for all-day.
        end: String,
        /// The chosen calendar's owning account id, or `None` for the default.
        account: Option<String>,
        /// The chosen calendar's row key (`CalendarRow.id`), or `None` for the first.
        calendar: Option<String>,
        /// Whether this is an all-day event.
        all_day: bool,
        /// The IANA zone a timed event is created in (the device's zone), so it reads back the
        /// same clock on edit. `None`/empty falls back to UTC (`start`/`end` then UTC instants).
        timezone: Option<String>,
        /// The description/notes, if any.
        notes: Option<String>,
        /// The location, if any. A create is the one write that sets it from nothing; an edit
        /// reshapes it through [`EventIntent::Update`]'s `location`.
        location: Option<String>,
        /// How the event repeats, or `None` for a one-off. Changing the rule afterwards goes
        /// through [`EventIntent::Update`]'s `recurrence`.
        #[uniffi(default = None)]
        recurrence: Option<SimpleRecurrence>,
    },
    /// Edit a stored calendar event, then refresh the agenda.
    ///
    /// A **provider-neutral patch**: only the fields present change; the recurrence rule,
    /// attendees, alarms and timezone survive. Every time field is a **wall clock in the
    /// event's own zone** (never a UTC instant), so a move cannot convert a zoned or all-day
    /// event: for an all-day event they are `YYYY-MM-DD` dates (end exclusive).
    ///
    /// The optional fields are three-state: absent leaves the property unchanged, an empty
    /// string clears it, a value sets it. `title`/`start`/`end` cannot be cleared (an event
    /// must keep them), so an empty value there is treated as "unchanged".
    Update {
        /// The account that owns the event (the row's `account`).
        account: String,
        /// The event's provider key (the row's `event`/`key`).
        key: String,
        /// The new title, or `None`/empty to leave it.
        title: Option<String>,
        /// The new start wall-clock (`2026-07-01T10:00:00`, or `2026-07-01` if all-day), or
        /// `None`/empty to leave it.
        start: Option<String>,
        /// The new end, same terms as `start` (exclusive date if all-day), or `None`/empty.
        end: Option<String>,
        /// The new notes/description: `None` leaves, empty clears, a value sets.
        notes: Option<String>,
        /// The new location: `None` leaves, empty clears, a value sets.
        location: Option<String>,
        /// For a recurring event, the **original** start wall-clock of the single occurrence
        /// to edit (splitting an override out of the series); `None`/empty edits the whole
        /// series. See `TimedSegment::occurrence_start`.
        occurrence: Option<String>,
        /// What happens to the repeat rule: `None` leaves the series as it is, `Set` replaces
        /// the rule, `Clear` makes the event a single one.
        ///
        /// Only a **series** edit may carry one, and only over a rule the core described as
        /// `EventRecurrence::Simple`. Pairing it with `occurrence`, or sending one for an
        /// event whose rule read `Complex`, is refused rather than written; see
        /// [`RecurrenceChange`].
        #[uniffi(default = None)]
        recurrence: Option<RecurrenceChange>,
        /// The occurrence `start` and `end` were **read from**, when this edit is meant for the
        /// series but the editor was opened on one occurrence: `EventDetail::occurrence_start`,
        /// handed straight back.
        ///
        /// An editor opened on one occurrence shows **that** occurrence's clocks, so sending
        /// them as the series' own moves the series to that occurrence and every earlier one
        /// stops existing. Naming where they came from makes the edit the *shift* the user
        /// made, applied to the series' own clock, which is what a drag on a series does.
        ///
        /// Send it whenever `occurrence` is `None` and the editor was opened on one; leave it
        /// `None` for an editor opened on the series, whose clocks are already the series'. It
        /// is ignored when `occurrence` is set. Setting it needs **both** `start` and `end`.
        #[uniffi(default = None)]
        times_from_occurrence: Option<String>,
    },
    /// Move or resize a stored calendar event by **dragging** it on the grid, then refresh the
    /// agenda.
    ///
    /// A drag is a **delta, not a destination**, and that is the whole design. The client sends
    /// how far the hand moved (signed whole days and minutes) and the core applies it to the
    /// event's own wall clock. Three things fall out that a dropped date-and-time cannot give:
    ///
    /// - **The display zone never reaches the write.** A meeting in `Europe/Amsterdam` read on a
    ///   device set to `America/New_York` is drawn six hours earlier; the clock it was dropped
    ///   under is not the clock it must be written with. An offset is the same number in both.
    /// - **A clipped segment still works.** An event crossing midnight is drawn as one segment per
    ///   day, each clipped to its column, so a segment's `start_minutes` is `0` on every day but
    ///   the first; there is no absolute start on screen to send.
    /// - **A move preserves its duration exactly**, because both edges take the same offset.
    ///
    /// A client that snaps its drop to the quarter hour simply snaps the offset.
    ///
    /// **Only the user's own events may be dragged**: an appointment nobody was invited to, or
    /// a meeting this account organises. Gate the gesture on `TimedSegment::can_move`; the core
    /// re-checks and refuses, because a write must not trust its caller. Moving a meeting
    /// *somebody else* called is *propose a new time*, which is a separate feature.
    Move {
        /// The account that owns the event (the segment's `account`).
        account: String,
        /// The event's provider key (the segment's `event`).
        key: String,
        /// Which edges the drag moved.
        edge: EventEdge,
        /// Whole days the dragged edge(s) move by, signed.
        days: i32,
        /// Minutes within the day the dragged edge(s) move by, signed. Ignored for an all-day
        /// event, which has no clock to move along.
        minutes: i32,
        /// The occurrence that was dragged, as `TimedSegment::occurrence_start` gave it;
        /// passed back **verbatim**, never parsed or recomputed.
        ///
        /// `None`/empty moves the **whole series**. There is no default and there must not be
        /// one: dragging one Tuesday standup is not the same as rewriting every Tuesday to
        /// eternity, so a client whose segment carries a non-empty `occurrence_start` **asks**
        /// before it sends.
        occurrence: Option<String>,
    },
    /// Delete a calendar event (or one occurrence of it) by its key, then refresh the agenda.
    Delete {
        /// The id of the account that owns the event (the row's `account`).
        account: String,
        /// The event's provider key.
        key: String,
        /// For a recurring event, the **original** start wall-clock of the single occurrence
        /// to remove; `None`/empty deletes the whole series. The same token
        /// `EventIntent::Update` takes, from `TimedSegment::occurrence_start`, and the same
        /// question to put to the user, because cancelling one Tuesday and cancelling the
        /// standup are different requests.
        #[uniffi(default = None)]
        occurrence: Option<String>,
    },
}
