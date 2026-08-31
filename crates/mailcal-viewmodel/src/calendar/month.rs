//! The month grid: six weeks of day cells, each listing the events that fall on it.
//!
//! Not the time grid with more columns: a genuinely different layout. A month cell has no hour
//! axis and no overlap solving; it is a *list*, ordered so the day reads top to bottom the way it
//! happens: all-day events first (they bound the whole day), then timed events by start.
//!
//! **Always six weeks (42 cells)**, even when five would do. A month grid that changes height as
//! you page makes the whole screen jump, and a "+2 more" chip that fits in February and not in
//! March is worse than one that always fits.
//!
//! Every cell carries *all* its events, not a truncated list plus a count. How many chips fit is a
//! question of how tall a cell is on this screen (a client concern, like the hour height) so the
//! client caps and counts the remainder. The core does not guess at a phone's row height.

use engine_api::{CalendarDate, TimeZoneId};

use super::{
    days::{date_at, day_number, from_civil, weekday},
    grid::{Occurrence, extent},
};

/// How many day cells a month page always has: six weeks of seven.
pub const MONTH_CELLS: usize = 42;

/// One event on one day of the month grid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonthChip {
    /// The owning account.
    pub account: String,
    /// The event's provider key.
    pub event: String,
    /// The calendar it belongs to: the client looks its colour up.
    pub calendar: String,
    /// The event's title.
    pub title: String,
    /// Whether it covers the whole day (drawn as a filled bar rather than a dotted entry).
    pub all_day: bool,
    /// Wall-clock minutes from midnight it starts at; `0` for an all-day event.
    pub start_minutes: u32,
    /// Whether this event's owning account supports calendar writes.
    pub can_write: bool,
    /// This occurrence's **original** start, on the same terms as
    /// [`TimedSegment::occurrence_start`](crate::calendar::grid::TimedSegment::occurrence_start):
    /// the token that names one occurrence to a write, and empty when the event does not recur.
    /// A chip is one occurrence, so an edit or a delete reached from the month grid has the same
    /// question to put as one reached from a block.
    pub occurrence_start: String,
    /// How this account answered;
    /// [`ResponseStatus::NeedsAction`](crate::invitation::ResponseStatus::NeedsAction) is an
    /// unanswered hold, drawn dotted, and its accessibility label must say so
    /// (`docs/calendar.md` §4). Declined events never reach a month cell; the core filters
    /// them upstream.
    pub participation: crate::invitation::ResponseStatus,
}

/// One day cell of the month grid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonthCell {
    /// The date this cell shows, `YYYY-MM-DD`.
    pub date: String,
    /// Whether the date belongs to the **anchored month** rather than to the leading or trailing
    /// days of its neighbours. A client dims the others; without this it cannot tell them apart,
    /// and the 1st of the next month looks like part of this one.
    pub in_month: bool,
    /// Everything on this day, all-day first and then by start time. **Not** truncated: the client
    /// caps it to what fits and counts the rest.
    pub chips: Vec<MonthChip>,
}

/// A month, laid out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonthGrid {
    /// Exactly [`MONTH_CELLS`] cells, row-major: six weeks of seven days.
    pub cells: Vec<MonthCell>,
    /// The IANA display zone the layout was computed in.
    pub timezone: String,
}

/// Lays out the month containing `anchor`.
///
/// The grid starts on the week containing the 1st: so it opens with a few days of the previous
/// month, and always runs [`MONTH_CELLS`] days.
#[must_use]
pub fn build<'a>(
    anchor: CalendarDate,
    week_starts_monday: bool,
    occurrences: impl IntoIterator<Item = &'a Occurrence>,
    zone: &TimeZoneId,
) -> MonthGrid {
    // Collected once: the cells below each scan it, and a one-shot iterator cannot be read 42
    // times. Still borrows, nothing here needs to own an occurrence.
    let occurrences: Vec<&Occurrence> = occurrences.into_iter().collect();
    let first_of_month = from_civil(anchor.year(), anchor.month(), 1);
    let index = i64::from(weekday(first_of_month));
    // Monday-based indices, so a Sunday-start week is the same run rotated one day back.
    let offset = if week_starts_monday {
        index
    } else {
        (index + 1) % 7
    };
    let first_cell = first_of_month - offset;

    let cells = (0..MONTH_CELLS)
        .map(|step| {
            let day = first_cell + i64::try_from(step).unwrap_or(0);
            let date = date_at(day);
            MonthCell {
                date: date.to_string(),
                in_month: date.month() == anchor.month() && date.year() == anchor.year(),
                chips: chips_on(day, &occurrences, zone),
            }
        })
        .collect();

    MonthGrid {
        cells,
        timezone: zone.as_str().to_owned(),
    }
}

/// Everything happening on the civil day `day`, in reading order.
///
/// The day axis comes from the time grid's own [`extent`], so the two layouts cannot disagree about
/// which day an event lands on; including the all-day rules (zoneless dates, exclusive midnight
/// ends) that took two bugs to get right.
fn chips_on(day: i64, occurrences: &[&Occurrence], zone: &TimeZoneId) -> Vec<MonthChip> {
    let mut chips: Vec<MonthChip> = occurrences
        .iter()
        .filter_map(|occurrence| {
            let extent = extent(occurrence, zone)?;
            if day < extent.first_day || day > extent.last_day {
                return None;
            }
            Some(MonthChip {
                account: occurrence.account.clone(),
                event: occurrence.event.clone(),
                calendar: occurrence.calendar.clone(),
                title: occurrence.title.clone(),
                all_day: occurrence.all_day,
                // An all-day event has no start time to show, and a multi-day event that began
                // yesterday runs through this day from midnight: not from its own start, which was
                // on a different day and would read as a lie here.
                start_minutes: if occurrence.all_day || day > extent.first_day {
                    0
                } else {
                    u32::try_from(extent.start_minute).unwrap_or(0)
                },
                can_write: occurrence.can_write,
                occurrence_start: occurrence.occurrence_start.clone(),
                participation: occurrence.participation,
            })
        })
        .collect();

    // All-day first (they bound the whole day, so they read as its heading) then by start, then
    // by title so two events at the same minute cannot swap places between two clients.
    chips.sort_by(|a, b| {
        b.all_day
            .cmp(&a.all_day)
            .then(a.start_minutes.cmp(&b.start_minutes))
            .then(a.title.cmp(&b.title))
            .then(a.event.cmp(&b.event))
    });
    chips
}

/// The day number of `date`; re-exported for the app layer's horizon checks.
#[must_use]
pub fn cell_day(date: CalendarDate) -> i64 {
    day_number(date)
}

#[cfg(test)]
#[path = "month_tests.rs"]
mod month_tests;
