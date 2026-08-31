//! Turning a drag on the grid into an edit of a stored event.
//!
//! # A drag is a delta, not a destination
//!
//! The obvious design is for a client to send where it dropped the block: a date and a
//! wall clock. It is wrong three times over, and each way is invisible until it bites:
//!
//! 1. **The client draws in the *display* zone; the event lives in its own.** A meeting in
//!    `Europe/Amsterdam` read on a device set to `America/New_York` is drawn six hours earlier, so
//!    the clock the user dropped it under is not the clock the event must be written with. A client
//!    that sent the drop position would move a colleague's meeting to the wrong hour for everybody
//!    else: the exact failure [`EventEdit`](crate::EventEdit)'s wall-clock rule exists to prevent.
//! 2. **A dragged block is not always the whole event.** The grid splits an event crossing midnight
//!    into one segment per day, and clips each to its column, so a segment's `start_minutes` is `0`
//!    on every day but the first. Drag the second day of a two-day booking and there is no absolute
//!    start on screen to send.
//! 3. **A destination cannot preserve a duration.** Rounding a drop to the grid and then
//!    re-deriving the end silently re-times the event; a delta moves both edges by the same amount
//!    and the duration comes out bit-identical.
//!
//! A delta has none of those problems, because it is the same number in either zone and on
//! any segment: the hand moved the block *this far*. So the client sends whole days and
//! minutes, and this module applies them to the event's **own** wall clock. Nothing about
//! the display zone reaches the write at all.
//!
//! The arithmetic is deliberately **wall-clock**, not elapsed time: dragging an event across
//! a spring-forward boundary keeps it at 10:00 rather than landing it at 11:00, which is
//! what the grid shows and what the user meant (`docs/calendar.md` §1: the grid is a wall
//! clock, so a 23- and a 25-hour day render identically).

use engine_api::OccurrenceRow;
use engine_core::{
    calendar::Event,
    time::{CalendarDateTime, LocalDateTime, TimeZoneId, UtcDateTime},
};
use mailcal_viewmodel::calendar::days::{date_at, from_civil};

use crate::{
    AccountError, EventEdit,
    event_detail::{datetime_str, end_wall_clock},
};

/// The shortest an event may be left by a resize, in minutes.
///
/// The same floor the grid gives a block so it stays tappable
/// (`mailcal_viewmodel::calendar::grid`'s `MIN_SEGMENT_MINUTES`). Dragging an edge past its
/// opposite **clamps** here rather than failing: a calendar that refuses the gesture leaves the
/// user holding a block that will not shrink, with nothing on screen to say why.
const MIN_EVENT_MINUTES: i64 = 15;

/// Minutes in a day. DST cannot change this here: the arithmetic is wall-clock.
const DAY_MINUTES: i64 = 24 * 60;

/// The furthest a single drag may carry an event, in days.
///
/// A gesture cannot produce anything near this; the bound exists because the value crosses the
/// FFI, and the civil-date conversion below panics outside representable calendar time. A limit
/// that rejects is better than a conversion that aborts the process.
const MAX_DRAG_DAYS: i64 = 366;

/// Which edges of an event a drag moved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventEdge {
    /// The block itself was dragged: both edges move together, so the duration is preserved
    /// exactly.
    Whole,
    /// The top edge was dragged: the event starts earlier or later and ends where it did.
    Start,
    /// The bottom edge was dragged: the event ends earlier or later and starts where it did.
    End,
}

/// One drag of an event on the grid, in the currency the grid speaks.
///
/// [`days`](Self::days) and [`minutes`](Self::minutes) are **wall-clock** offsets, both signed
/// and both applied together: a block dragged from Monday 09:00 to Tuesday 08:30 is
/// `days: 1, minutes: -30`. A client that snaps its drop to the quarter hour simply snaps the
/// delta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventDrag {
    /// Which edges moved.
    pub edge: EventEdge,
    /// Whole days the dragged edge(s) move by.
    pub days: i32,
    /// Minutes within the day the dragged edge(s) move by. Ignored for an all-day event,
    /// which has no clock to move along.
    pub minutes: i32,
    /// Which occurrence of a recurring event was dragged, named by its **original** start as a
    /// wall clock in the event's own zone (`TimedSegment::occurrence_start`).
    ///
    /// `None` moves the **whole series**. There is no default and there must not be one:
    /// dragging one Tuesday standup is not the same as rewriting every Tuesday to eternity, and
    /// only the user knows which they meant: so a client asks before it sends
    /// (`docs/calendar.md` §13).
    pub occurrence: Option<LocalDateTime>,
}

/// The [`EventEdit`] a drag of `event` produces, in the event's own wall clock.
///
/// Applies the delta to whichever edges [`EventDrag::edge`] names, clamps a resize so the event
/// keeps at least a quarter of an hour (a whole day, when all-day), and leaves every other
/// property alone: the caller hands the result to
/// [`build_event_patch`](crate::build_event_patch), which patches rather than rebuilds, so the
/// recurrence rule, attendees, alarms and zone survive.
///
/// The event's **form never changes**: an all-day event stays all-day and a zoned one stays in
/// its zone, because the edit names a wall clock and `build_event_patch` renders it in whatever
/// form the stored event already has.
///
/// # Errors
///
/// Returns [`AccountError::CalendarWrite`] if the drag is further than a year, or if
/// the event's own bounds or the shifted result are not representable.
pub fn apply_event_drag(event: &Event, drag: &EventDrag) -> Result<EventEdit, AccountError> {
    let (days, minutes) = bounded(drag, event.start.is_all_day())?;
    let (start, end) = own_bounds(event)?;

    let (start, end) = match drag.edge {
        // Both edges by the same delta: the duration comes out bit-identical, so there is
        // nothing to clamp.
        EventEdge::Whole => (shift(start, days, minutes)?, shift(end, days, minutes)?),
        // A dragged start may not pass its end, and a dragged end may not pass its start, so
        // each is clamped *towards* the edge it did not touch, and only that edge moves.
        EventEdge::Start => (
            earlier(shift(start, days, minutes)?, latest_start(end, event)?),
            end,
        ),
        EventEdge::End => (
            start,
            later(shift(end, days, minutes)?, earliest_end(start, event)?),
        ),
    };

    Ok(EventEdit {
        start: Some(start),
        end: Some(end),
        occurrence: drag.occurrence,
        ..EventEdit::default()
    })
}

/// The occurrence at `instant` as a wall clock in the event's **own** terms: the token a client
/// hands back as [`EventDrag::occurrence`] to move one occurrence of a series.
///
/// It is deliberately opaque to the client: an identifier for "the Tuesday you dragged", not a
/// time to compute with. The core mints it and the core reads it, so no client has to know that
/// a `RECURRENCE-ID` is written in the series' own zone rather than in the one it drew.
///
/// `display_zone` is the zone the horizon was expanded in, and it is used for exactly one case: a
/// **floating** recurring event, whose occurrences the engine resolves through the host zone
/// (`engine-recurrence`'s `host_zone`). A zoned event uses its own zone and an all-day one is
/// zoneless, so neither consults it.
///
/// Always a full `YYYY-MM-DDTHH:MM:SS`, even for an all-day event whose `RECURRENCE-ID` is a bare
/// date: the token has to survive a round trip through the FFI, where one shape parses and two
/// do not, and `build_event_patch` reads only the date part back out for an all-day event
/// anyway. One format, no branch at the boundary.
///
/// Returns `None` when the instant cannot be rendered in the event's zone.
#[must_use]
pub fn occurrence_wall_clock(
    event: &Event,
    instant: UtcDateTime,
    display_zone: &TimeZoneId,
) -> Option<String> {
    occurrence_local(event, instant, display_zone).map(datetime_str)
}

/// `instant` as a wall clock in the event's **own** terms, before it is written as a token.
///
/// The rule the token is minted by, on its own, because reading one occurrence's times needs it
/// applied to *both* edges rather than to the recurrence id alone. Keeping it in one function is
/// what stops a detail and a token disagreeing about which clock an occurrence keeps.
#[must_use]
pub fn occurrence_local(
    event: &Event,
    instant: UtcDateTime,
    display_zone: &TimeZoneId,
) -> Option<LocalDateTime> {
    match &event.start {
        // All-day occurrences are zoneless: the engine expands them to UTC midnights, and
        // localising one would drag it onto the day before or after: the bug that renders every
        // one-day event two days wide (`docs/calendar.md` §1).
        CalendarDateTime::Date(_) => {
            LocalDateTime::new(instant.year(), instant.month(), instant.day(), 0, 0, 0).ok()
        }
        CalendarDateTime::Floating(_) => engine_api::to_local(instant, display_zone).ok(),
        CalendarDateTime::Zoned { zone, .. } => engine_api::to_local(instant, zone).ok(),
    }
}

/// Whether `occurrence` names an occurrence of `event` that this core actually put on the grid.
///
/// The other half of [`occurrence_wall_clock`], and the answer to the one question a write can
/// ask about a scope: the token is opaque and the core is the only thing that mints one, so
/// "does this name an occurrence?" *is* "is this one we would mint?".
///
/// Implemented by **re-minting and comparing**, never by parsing the token back. A second
/// reader would be a second opinion about what a `RECURRENCE-ID` is, and the two would
/// eventually disagree; silently, on somebody's series. Re-minting cannot drift from the
/// emitter, because it *is* the emitter.
///
/// `rows` are the store's materialized occurrences the caller read for the day the token names;
/// rows belonging to another event are ignored. A one-off event is named by no token at all, so
/// it answers `false` for every one, which is also the refusal for "split an override out of an
/// event that has no series".
#[must_use]
pub fn names_an_occurrence(
    event: &Event,
    rows: &[OccurrenceRow],
    occurrence: LocalDateTime,
    display_zone: &TimeZoneId,
) -> bool {
    stored_occurrence(event, rows, occurrence, display_zone).is_some()
}

/// The stored row `occurrence` names, or `None` when none of `rows` is it.
///
/// The search [`names_an_occurrence`] is the yes/no of. A reader wants the row itself: it holds
/// the instants the expander actually produced, which is where an occurrence the user moved
/// keeps its own times: the master's are the *first* occurrence's, not this one's.
#[must_use]
pub fn stored_occurrence<'a>(
    event: &Event,
    rows: &'a [OccurrenceRow],
    occurrence: LocalDateTime,
    display_zone: &TimeZoneId,
) -> Option<&'a OccurrenceRow> {
    event.recurrence.as_ref()?;
    let named = datetime_str(occurrence);
    let key = event.id.key().as_str();
    rows.iter()
        .filter(|row| row.event.as_str() == key)
        .find(|row| {
            // The identity of an occurrence is the slot it came from, not where it now sits;
            // the same `recurrence_id.unwrap_or(start)` the grid mints its token from.
            occurrence_wall_clock(event, row.recurrence_id.unwrap_or(row.start), display_zone)
                .is_some_and(|token| token == named)
        })
}

/// The drag's `(days, minutes)` as `i64`, bounded, with an all-day event's minutes dropped.
///
/// An all-day event has no clock to move along (its start is a bare date) so a minute
/// component is not merely useless, it would round a date onto its neighbour. Dropped rather
/// than rejected: a client that sends one is asking for a whole-day move.
fn bounded(drag: &EventDrag, all_day: bool) -> Result<(i64, i64), AccountError> {
    let days = i64::from(drag.days);
    let minutes = if all_day { 0 } else { i64::from(drag.minutes) };
    let span = days + minutes.div_euclid(DAY_MINUTES);
    if span.abs() > MAX_DRAG_DAYS {
        return Err(AccountError::CalendarWrite(format!(
            "a drag of {span} days is beyond the {MAX_DRAG_DAYS}-day limit"
        )));
    }
    Ok((days, minutes))
}

/// The event's own `(start, end)` as wall clocks in its own zone.
///
/// An all-day event's end is the **exclusive** midnight after its last day, matching the form
/// [`EventDetail`](crate::EventDetail) reports and `EventDraft` requires: so a one-day event on
/// the 1st comes back `(1st, 2nd)` and a whole-day resize is plain arithmetic on that.
pub(crate) fn own_bounds(event: &Event) -> Result<(LocalDateTime, LocalDateTime), AccountError> {
    let unrepresentable =
        || AccountError::CalendarWrite("the event's own times are not representable".to_owned());
    match &event.start {
        CalendarDateTime::Date(date) => {
            let start = LocalDateTime::new(date.year(), date.month(), date.day(), 0, 0, 0)
                .map_err(|_| unrepresentable())?;
            let days = i64::try_from(event.duration.days()).unwrap_or(1).max(1);
            Ok((start, shift(start, days, 0)?))
        }
        CalendarDateTime::Floating(local) | CalendarDateTime::Zoned { local, .. } => {
            Ok((*local, end_wall_clock(event).ok_or_else(unrepresentable)?))
        }
    }
}

/// The latest a dragged **start** may land: the event's end, less the minimum it must keep.
fn latest_start(end: LocalDateTime, event: &Event) -> Result<LocalDateTime, AccountError> {
    if event.start.is_all_day() {
        shift(end, -1, 0)
    } else {
        shift(end, 0, -MIN_EVENT_MINUTES)
    }
}

/// The earliest a dragged **end** may land: the event's start, plus the minimum it must keep.
fn earliest_end(start: LocalDateTime, event: &Event) -> Result<LocalDateTime, AccountError> {
    if event.start.is_all_day() {
        shift(start, 1, 0)
    } else {
        shift(start, 0, MIN_EVENT_MINUTES)
    }
}

/// The earlier of two wall clocks.
fn earlier(a: LocalDateTime, b: LocalDateTime) -> LocalDateTime {
    if order(a) <= order(b) { a } else { b }
}

/// The later of two wall clocks.
fn later(a: LocalDateTime, b: LocalDateTime) -> LocalDateTime {
    if order(a) >= order(b) { a } else { b }
}

/// A wall clock as a comparable `(day, minute, second)`: the civil day number, so the ordering
/// is right across a month or a year boundary that a field-by-field compare would get wrong.
fn order(at: LocalDateTime) -> (i64, i64, i64) {
    (
        from_civil(at.year(), at.month(), at.day()),
        i64::from(at.hour()) * 60 + i64::from(at.minute()),
        i64::from(at.second()),
    )
}

/// `local` moved by `days` whole days and `minutes` minutes, on the **wall clock**.
///
/// Civil-day arithmetic, so a move across a DST boundary keeps the clock reading: +1 day from
/// 10:00 is 10:00 the next day, whether that day is 23, 24 or 25 hours long. Seconds ride
/// along untouched, so an event at `:30` past does not quietly lose them.
fn shift(local: LocalDateTime, days: i64, minutes: i64) -> Result<LocalDateTime, AccountError> {
    let total = i64::from(local.hour()) * 60 + i64::from(local.minute()) + minutes;
    let day =
        from_civil(local.year(), local.month(), local.day()) + days + total.div_euclid(DAY_MINUTES);
    let minute_of_day = total.rem_euclid(DAY_MINUTES);
    let date = date_at(day);
    LocalDateTime::new(
        date.year(),
        date.month(),
        date.day(),
        u8::try_from(minute_of_day / 60).unwrap_or(0),
        u8::try_from(minute_of_day % 60).unwrap_or(0),
        local.second(),
    )
    .map_err(|err| AccountError::CalendarWrite(format!("the dragged time is out of range: {err}")))
}

#[cfg(test)]
#[path = "calendar_drag_tests.rs"]
mod tests;
