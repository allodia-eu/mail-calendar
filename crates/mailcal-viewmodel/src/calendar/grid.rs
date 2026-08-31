//! The time grid: day / 3-day / work-week / week.
//!
//! All four are the *same* view with a different number of day columns, so they are one
//! solver here rather than four. The caller passes the days it wants shown; this module
//! places each occurrence into a column, gives it wall-clock minutes within that column,
//! splits it where it crosses midnight, and resolves collisions into side-by-side lanes.
//!
//! # Wall clock, not elapsed time
//!
//! An occurrence's position comes from the **wall clock** it shows in the display zone
//! ([`engine_api::to_local`]), never from minutes elapsed since local midnight. On the
//! spring-forward day those differ by an hour: 09:00 local is only 480 real minutes after
//! midnight because 02:00 never happened, yet it belongs on the 09:00 row like any other
//! day. Working from the wall clock makes a 23- and a 25-hour day render identically,
//! which is what a user expects: the grid is 24 rows tall on every day of the year.

use engine_api::{CalendarDate, LocalDateTime, TimeZoneId, UtcDateTime, to_local};

use super::{
    days::{day_number, from_civil},
    packing::{self, Span},
};
use crate::invitation::ResponseStatus;

/// Minutes in a day column. DST does not change this: the grid is wall-clock.
const DAY_MINUTES: i64 = 24 * 60;

/// The shortest a timed segment may render, so a zero- or one-minute event is still
/// tappable and still collides with its neighbours instead of hiding inside them.
const MIN_SEGMENT_MINUTES: i64 = 15;

/// A timed event lasting at least this long is banded above the grid rather than drawn in
/// it: a booking that covers whole days is a banner, not a very tall block. Matches what
/// Google Calendar and Outlook do.
const BANNER_MINUTES: i64 = DAY_MINUTES;

/// One materialized occurrence, joined to the master event that carries its content.
///
/// The app builds these from `Engine::occurrences_in` (the instants) and `Engine::events`
/// (the title, calendar, and kind): occurrences carry *when*, the master carries *what*.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Occurrence {
    /// The owning account, so an action on the block routes to it (two accounts can mint
    /// the same event key).
    pub account: String,
    /// The master event's provider key.
    pub event: String,
    /// The calendar the event belongs to: the key for its colour and its visibility.
    pub calendar: String,
    /// The event's title (a placeholder if empty).
    pub title: String,
    /// The occurrence's absolute start.
    pub start: UtcDateTime,
    /// The occurrence's absolute end (exclusive).
    pub end: UtcDateTime,
    /// Whether the master is an all-day (date-only) event.
    pub all_day: bool,
    /// Whether this event's owning account supports calendar writes. The host uses this
    /// to hide edit affordances on read-only calendars.
    pub can_write: bool,
    /// Whether the user may **drag** this event to a new time: a writable calendar *and* an
    /// event that is theirs to reshape (their own appointment, or a meeting they organise).
    ///
    /// Strictly narrower than [`Self::can_write`]: a meeting somebody else called can sit on a
    /// writable calendar and still must not be silently re-timed. See
    /// `mailcal_app::invitations::owns_or_organizes`.
    pub can_move: bool,
    /// This occurrence's **original** start, as a wall clock in the event's own zone: the token
    /// that names one occurrence of a series to a write, and empty when the event does not
    /// recur.
    ///
    /// Opaque to the client: an identifier for "the Tuesday you dragged", never a time to
    /// compute with. Non-empty is also the signal that a drag must **ask** whether the user
    /// meant this occurrence or every one of them, because the core will not guess
    /// (`docs/calendar.md` §13).
    pub occurrence_start: String,
    /// How **this account** has answered, when the event is something it was invited to.
    ///
    /// An event with no attendees (the user's own appointment) is
    /// [`ResponseStatus::Accepted`]: they put it in their own diary, so it is a commitment, not
    /// an unanswered hold. A [`ResponseStatus::NeedsAction`] occurrence is a hold the organizer
    /// is still waiting on, which a client draws as a **dotted** block.
    ///
    /// [`ResponseStatus::Declined`] never reaches a grid: the core filters those out upstream
    /// (`docs/invitations.md`).
    pub participation: ResponseStatus,
}

/// One day column: the date it shows.
///
/// Deliberately *not* "is today", that goes stale every midnight, which would make the
/// snapshot wrong while the app sits open overnight. The client has a clock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GridDay {
    /// The local calendar date this column shows, `YYYY-MM-DD`.
    pub date: String,
}

/// A block drawn inside the grid: one day's worth of one occurrence.
///
/// An event crossing midnight becomes several of these (one per day it touches) so a
/// client only ever draws a rectangle inside a single column.
// Four bools, and clippy is right to ask. They are kept flat anyway, for two reasons: they are
// genuinely independent facts about one rectangle (where its edges are cut, and what the user
// may do to it), so no enum names their combinations without inventing states that do not
// exist, and this type is mirrored one-for-one by a **flat** UniFFI record, so grouping them
// here would put a shape in the view-model that the wire does not have.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimedSegment {
    /// The owning account.
    pub account: String,
    /// The master event's provider key.
    pub event: String,
    /// The calendar, for the block's colour.
    pub calendar: String,
    /// The event's title.
    pub title: String,
    /// Which day column (an index into [`TimeGrid::days`]).
    pub day: u32,
    /// Wall-clock minutes from midnight where the block starts, `0..1440`.
    pub start_minutes: u32,
    /// Wall-clock minutes from midnight where it ends, `0..=1440` and always greater than
    /// [`Self::start_minutes`].
    pub end_minutes: u32,
    /// Which lane of its collision cluster, `0..columns`.
    pub column: u32,
    /// How many lanes the cluster splits into: the divisor for the block's width.
    pub columns: u32,
    /// The event began before this column (draw the top edge open).
    pub continues_before: bool,
    /// The event runs past this column (draw the bottom edge open).
    pub continues_after: bool,
    /// Whether this event's owning account supports calendar writes. The host uses this
    /// to hide edit affordances on read-only calendars.
    pub can_write: bool,
    /// Whether this block may be **dragged**; see [`Occurrence::can_move`].
    pub can_move: bool,
    /// This occurrence's original start in the event's own zone, empty when it does not recur;
    /// see [`Occurrence::occurrence_start`].
    pub occurrence_start: String,
    /// How this account answered; [`ResponseStatus::NeedsAction`] is the unanswered hold a
    /// client draws with a dashed border and a hatched leading gutter.
    ///
    /// The visual is not enough on its own: a dashed border is invisible to a screen reader, so
    /// the accessibility label must **say** it ("Awaiting your response";
    /// `a11y_invitation_awaiting_response`). `docs/calendar.md` §4, the spoken-grid rule.
    pub participation: ResponseStatus,
}

/// A bar above the grid: an all-day or multi-day event, spanning whole day columns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllDayBand {
    /// The owning account.
    pub account: String,
    /// The master event's provider key.
    pub event: String,
    /// The calendar, for the bar's colour.
    pub calendar: String,
    /// The event's title.
    pub title: String,
    /// The first day column the bar covers.
    pub day: u32,
    /// How many columns it covers, at least 1.
    pub days: u32,
    /// Which stacked row of the banner it sits in.
    pub lane: u32,
    /// The event began before the first shown day (draw the left edge open).
    pub continues_before: bool,
    /// The event runs past the last shown day (draw the right edge open).
    pub continues_after: bool,
    /// Whether this event's owning account supports calendar writes. The host uses this
    /// to hide edit affordances on read-only calendars.
    pub can_write: bool,
    /// This occurrence's **original** start, on the same terms as
    /// [`TimedSegment::occurrence_start`]: the token that names one occurrence to a write, and
    /// empty when the event does not recur. A bar is one occurrence, so an edit or a delete
    /// reached from it has the same question to put as one reached from a block.
    pub occurrence_start: String,
    /// How this account answered; see [`TimedSegment::participation`], including the
    /// accessibility requirement.
    pub participation: ResponseStatus,
}

/// The laid-out grid a day/3-day/work-week/week view renders.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TimeGrid {
    /// The day columns, left to right.
    pub days: Vec<GridDay>,
    /// The blocks inside the grid.
    pub timed: Vec<TimedSegment>,
    /// The bars above it.
    pub all_day: Vec<AllDayBand>,
    /// How many stacked rows the banner needs: so the client can size it before laying
    /// the bars out, and reserve none at all when there are no all-day events.
    pub all_day_lanes: u32,
    /// The IANA display zone the layout was computed in.
    pub timezone: String,
}

/// Lays `occurrences` out over the `days` shown, in the `zone` display zone.
///
/// Occurrences outside the shown days are ignored, so a caller may pass everything it read
/// for a wider prefetch window without filtering first.
#[must_use]
pub fn build<'a>(
    days: &[CalendarDate],
    occurrences: impl IntoIterator<Item = &'a Occurrence>,
    zone: &TimeZoneId,
) -> TimeGrid {
    // The day axis is **civil day numbers**, not date strings. A date string can only be
    // compared against the days on screen, and the calendar arithmetic a grid needs; "the
    // day before this one" for an exclusive-midnight end; has to work when that day is
    // *off* screen. Getting a day number back to a label is one call, at the end.
    let shown: Vec<i64> = days.iter().copied().map(day_number).collect();
    let mut timed = Vec::new();
    let mut banded = Vec::new();

    for occurrence in occurrences {
        let Some(extent) = extent(occurrence, zone) else {
            // A zone the bundled tzdb cannot resolve has no position on a grid. The event
            // is not lost (the agenda still lists it) but guessing a column would draw it
            // confidently at the wrong time.
            continue;
        };
        if is_banner(occurrence) {
            banded.extend(band(occurrence, &extent, &shown));
        } else {
            timed.extend(segments(occurrence, &extent, &shown));
        }
    }

    let (all_day, all_day_lanes) = stack(banded);
    TimeGrid {
        days: days
            .iter()
            .map(|date| GridDay {
                date: date.to_string(),
            })
            .collect(),
        timed: columnize(timed),
        all_day,
        all_day_lanes,
        timezone: zone.as_str().to_owned(),
    }
}

/// Where an occurrence sits on the day axis: `(first day, minute in it, last day, minute
/// in it)`, all inclusive: the end already rolled back off an exclusive midnight.
pub(super) struct Extent {
    pub(super) first_day: i64,
    pub(super) start_minute: i64,
    pub(super) last_day: i64,
    end_minute: i64,
}

/// Resolves an occurrence onto the day axis, or `None` if its zone cannot be resolved.
///
/// Shared with the month grid, which needs the same day axis: an event covers exactly the days
/// `first_day..=last_day`, and the two layouts must not disagree about which day an event is on.
pub(super) fn extent(occurrence: &Occurrence, zone: &TimeZoneId) -> Option<Extent> {
    // An **all-day** event is a zoneless calendar date, expanded to UTC midnights and
    // invariant under the display zone (`calendar-semantics.md`), so its days come straight
    // off those instants. Localising them would be a real bug, not a rounding one: in any
    // zone east of UTC, midnight becomes 01:00/02:00 *the same day*, the exclusive end
    // stops looking like a midnight, and **every** one-day event renders two days wide.
    // A timed event is a genuine instant and does localise.
    let (first_day, start_minute, mut last_day, mut end_minute) = if occurrence.all_day {
        (
            utc_day(occurrence.start),
            utc_minute(occurrence.start),
            utc_day(occurrence.end),
            utc_minute(occurrence.end),
        )
    } else {
        let from = to_local(occurrence.start, zone).ok()?;
        let to = to_local(occurrence.end, zone).ok()?;
        (
            local_day(&from),
            minute_of(&from),
            local_day(&to),
            minute_of(&to),
        )
    };
    // The end is exclusive, so one landing exactly on midnight belongs to the day *before*.
    // Without this every all-day event renders a day too wide, and a timed one that ends at
    // midnight both claims the next day and reports itself as continuing into it.
    if end_minute == 0 && last_day > first_day {
        last_day -= 1;
        end_minute = DAY_MINUTES;
    }
    Some(Extent {
        first_day,
        start_minute,
        last_day,
        end_minute,
    })
}

/// Whether an occurrence belongs in the banner rather than the grid: an all-day event, or
/// a timed one long enough that drawing it as a block would just be a full-height bar.
fn is_banner(occurrence: &Occurrence) -> bool {
    occurrence.all_day || minutes_between(occurrence.start, occurrence.end) >= BANNER_MINUTES
}

/// The whole-minute span between two UTC instants, floored at zero.
///
/// Measured in **UTC**, where a day is always 24 hours, so "is this a day or more" has one
/// answer. Asking the same question of the local wall clocks would make a 24-hour booking a
/// banner on most days and a block on the fall-back day.
fn minutes_between(start: UtcDateTime, end: UtcDateTime) -> i64 {
    (epoch_minutes(end) - epoch_minutes(start)).max(0)
}

/// A UTC instant as whole minutes from the epoch.
fn epoch_minutes(at: UtcDateTime) -> i64 {
    utc_day(at) * DAY_MINUTES + utc_minute(at)
}

/// A timed occurrence's per-day blocks, clipped to the shown days and split at midnight.
fn segments(occurrence: &Occurrence, extent: &Extent, shown: &[i64]) -> Vec<TimedSegment> {
    let mut out = Vec::new();
    for (column, &day) in shown.iter().enumerate() {
        if day < extent.first_day || day > extent.last_day {
            continue;
        }
        let first = day == extent.first_day;
        let last = day == extent.last_day;
        let from_min = if first { extent.start_minute } else { 0 };
        let to_min = if last { extent.end_minute } else { DAY_MINUTES };
        // Give a zero- or one-minute event a floor, so it stays tappable and still collides
        // with its neighbours rather than hiding inside them.
        let to_min = to_min.max(from_min + MIN_SEGMENT_MINUTES).min(DAY_MINUTES);
        out.push(TimedSegment {
            account: occurrence.account.clone(),
            event: occurrence.event.clone(),
            calendar: occurrence.calendar.clone(),
            title: occurrence.title.clone(),
            day: u32::try_from(column).unwrap_or(u32::MAX),
            start_minutes: u32::try_from(from_min).unwrap_or(0),
            end_minutes: u32::try_from(to_min).unwrap_or(0),
            column: 0,
            columns: 1,
            continues_before: !first,
            continues_after: !last,
            can_write: occurrence.can_write,
            can_move: occurrence.can_move,
            occurrence_start: occurrence.occurrence_start.clone(),
            participation: occurrence.participation,
        });
    }
    out
}

/// An all-day or multi-day occurrence's banner bar, clipped to the shown days.
fn band(occurrence: &Occurrence, extent: &Extent, shown: &[i64]) -> Option<AllDayBand> {
    let first = shown.iter().position(|&day| day >= extent.first_day)?;
    let last = shown.iter().rposition(|&day| day <= extent.last_day)?;
    // Wholly outside the view: the two probes crossed, or the whole window sits past the
    // event (`first` found a day, but it is already after the event ended).
    if first > last || shown[first] > extent.last_day {
        return None;
    }
    Some(AllDayBand {
        account: occurrence.account.clone(),
        event: occurrence.event.clone(),
        calendar: occurrence.calendar.clone(),
        title: occurrence.title.clone(),
        day: u32::try_from(first).unwrap_or(0),
        days: u32::try_from(last - first + 1).unwrap_or(1),
        lane: 0,
        continues_before: shown[first] > extent.first_day,
        continues_after: shown[last] < extent.last_day,
        can_write: occurrence.can_write,
        occurrence_start: occurrence.occurrence_start.clone(),
        participation: occurrence.participation,
    })
}

/// Resolves the timed blocks' collisions, per day column.
///
/// Columns are solved **within** a day: two events on different days never collide, and
/// packing them together would let Tuesday's pile-up narrow Monday's lone meeting.
fn columnize(mut timed: Vec<TimedSegment>) -> Vec<TimedSegment> {
    let days: Vec<u32> = {
        let mut days: Vec<u32> = timed.iter().map(|s| s.day).collect();
        days.sort_unstable();
        days.dedup();
        days
    };
    for day in days {
        let indices: Vec<usize> = timed
            .iter()
            .enumerate()
            .filter(|(_, s)| s.day == day)
            .map(|(i, _)| i)
            .collect();
        let spans: Vec<Span> = indices
            .iter()
            .map(|&i| {
                Span::new(
                    i64::from(timed[i].start_minutes),
                    i64::from(timed[i].end_minutes),
                )
            })
            .collect();
        for (&i, placement) in indices.iter().zip(packing::pack(&spans)) {
            timed[i].column = placement.column;
            timed[i].columns = placement.columns;
        }
    }
    timed
}

/// Stacks the banner bars into lanes: each takes the topmost row free across every day it
/// covers, so bars never sit on top of each other and the banner stays as short as it can.
fn stack(mut bands: Vec<AllDayBand>) -> (Vec<AllDayBand>, u32) {
    // A total order over the input, so the lanes cannot depend on the order the caller
    // happened to collect the events in (the same reason the column packer sorts).
    let mut order: Vec<usize> = (0..bands.len()).collect();
    order.sort_by(|&a, &b| {
        (bands[a].day, u32::MAX - bands[a].days, &bands[a].event).cmp(&(
            bands[b].day,
            u32::MAX - bands[b].days,
            &bands[b].event,
        ))
    });

    // lane -> the first day column free in it.
    let mut lane_free: Vec<u32> = Vec::new();
    for &i in &order {
        let (day, days) = (bands[i].day, bands[i].days);
        let lane = lane_free
            .iter()
            .position(|&free| free <= day)
            .unwrap_or_else(|| {
                lane_free.push(0);
                lane_free.len() - 1
            });
        lane_free[lane] = day.saturating_add(days);
        bands[i].lane = u32::try_from(lane).unwrap_or(u32::MAX);
    }
    let lanes = u32::try_from(lane_free.len()).unwrap_or(u32::MAX);
    (bands, lanes)
}

/// The civil day number of a localised wall clock.
fn local_day(local: &LocalDateTime) -> i64 {
    from_civil(local.year(), local.month(), local.day())
}

/// The wall-clock minute-of-day of a localised time, `0..1440`.
fn minute_of(local: &LocalDateTime) -> i64 {
    i64::from(local.hour()) * 60 + i64::from(local.minute())
}

/// The civil day number of a UTC instant, unlocalized: for all-day values, which are
/// zoneless and must not be shifted by the display zone.
fn utc_day(at: UtcDateTime) -> i64 {
    from_civil(at.year(), at.month(), at.day())
}

/// The minute-of-day of a UTC instant, unlocalized.
fn utc_minute(at: UtcDateTime) -> i64 {
    i64::from(at.hour()) * 60 + i64::from(at.minute())
}

#[cfg(test)]
#[path = "grid_tests.rs"]
mod grid_tests;
