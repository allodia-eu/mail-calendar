//! The calendar's own pane on a desktop: a mini month to move the grid with, and every calendar
//! grouped under its account (`docs/calendar-pane.md`).
//!
//! Pure: the grid's position and today arrive as arguments, from the client that owns them, so
//! the pane cannot drift from the grid beside it and goes stale at no midnight.

use engine_api::CalendarDate;

use super::{
    CalendarRow,
    days::{date_at, day_number, from_civil, iso_week, weekday},
};

/// How many weeks a mini month always has, so its height never changes as it pages.
pub const MINI_WEEKS: usize = 6;

/// What the grid beside the pane is drawing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneView {
    /// A time grid: `columns` consecutive days from `from`.
    Days {
        /// The first day the grid shows.
        from: CalendarDate,
        /// How many days it shows.
        columns: u32,
    },
    /// The month grid, on any day of its month.
    Month {
        /// A day within the month on screen.
        anchor: CalendarDate,
    },
    /// The agenda list, which shows no particular days.
    Agenda,
}

/// Where a day sits in the band marking the days the grid shows, within its own row of the mini
/// month: a band that crosses a week boundary is two capsules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShownSpan {
    /// Outside the band: not shown.
    Outside,
    /// Shown, with no shown neighbour in its row: a capsule of its own.
    Only,
    /// The band's first day in this row.
    Start,
    /// Inside the band.
    Middle,
    /// The band's last day in this row.
    End,
}

/// One day of the mini month.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MiniDay {
    /// The date, `YYYY-MM-DD`: what a press moves the grid to.
    pub date: String,
    /// The day of the month, to draw.
    pub day: u32,
    /// Whether the day belongs to the month shown, rather than to a neighbour's leading or
    /// trailing days, which a client dims.
    pub in_month: bool,
    /// Whether it is today, by the client's own clock.
    pub is_today: bool,
    /// Its place in the band of days the grid shows.
    pub shown: ShownSpan,
}

/// One row of the mini month.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MiniWeek {
    /// The ISO 8601 week number, whatever the week start.
    pub number: u32,
    /// Exactly seven days, in week-start order.
    pub days: Vec<MiniDay>,
}

/// The mini month.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MiniMonth {
    /// The month's first day, `YYYY-MM-01`; the client words the title.
    pub first: String,
    /// The previous month's first day, passed back as the paged month by the previous arrow.
    pub previous: String,
    /// The next month's first day, likewise.
    pub next: String,
    /// The seven weekdays heading the columns, `0` = Monday … `6` = Sunday, in week-start
    /// order; the client words each one.
    pub weekdays: Vec<u8>,
    /// Exactly [`MINI_WEEKS`] rows.
    pub weeks: Vec<MiniWeek>,
}

/// One account's calendars, as the pane lists them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarGroup {
    /// The account's id.
    pub account: String,
    /// The account's address: the heading a person reads.
    pub address: String,
    /// Whether the group shows its calendars; open until the person shuts it.
    pub expanded: bool,
    /// Its calendars, in the order the account lists them.
    pub calendars: Vec<CalendarRow>,
}

/// The first day of the month the mini month follows while nobody has paged it: the month layout's
/// own month; for a time grid, today's month while today is on screen and otherwise the month of
/// its first day; for the agenda, today's.
#[must_use]
pub fn followed_month(view: &PaneView, today: CalendarDate) -> CalendarDate {
    let within = match *view {
        PaneView::Days { from, columns } => {
            let first = day_number(from);
            let today_day = day_number(today);
            if (first..first + i64::from(columns)).contains(&today_day) {
                today
            } else {
                from
            }
        }
        PaneView::Month { anchor } => anchor,
        PaneView::Agenda => today,
    };
    first_of(within)
}

/// The mini month: `paged` when the person paged to a month of their own, otherwise the month
/// [`followed_month`] picks.
#[must_use]
pub fn mini_month(
    view: &PaneView,
    today: CalendarDate,
    paged: Option<CalendarDate>,
    week_starts_monday: bool,
) -> MiniMonth {
    let first = paged.map_or_else(|| followed_month(view, today), first_of);
    let first_day = day_number(first);
    let start_index = if week_starts_monday { 0 } else { 6 };
    let offset = (i64::from(weekday(first_day)) - start_index).rem_euclid(7);
    let grid_first = first_day - offset;
    let shown = match *view {
        PaneView::Days { from, columns } => {
            let from = day_number(from);
            from..from + i64::from(columns)
        }
        PaneView::Month { .. } | PaneView::Agenda => 0..0,
    };
    let today = day_number(today);

    let weeks = (0..MINI_WEEKS)
        .map(|row| {
            let row_first = grid_first + 7 * i64::try_from(row).unwrap_or(0);
            let days = (0..7)
                .map(|column| {
                    let day = row_first + column;
                    let date = date_at(day);
                    let span = if shown.contains(&day) {
                        let before = column > 0 && shown.contains(&(day - 1));
                        let after = column < 6 && shown.contains(&(day + 1));
                        match (before, after) {
                            (false, false) => ShownSpan::Only,
                            (false, true) => ShownSpan::Start,
                            (true, true) => ShownSpan::Middle,
                            (true, false) => ShownSpan::End,
                        }
                    } else {
                        ShownSpan::Outside
                    };
                    MiniDay {
                        date: date.to_string(),
                        day: u32::from(date.day()),
                        in_month: date.month() == first.month() && date.year() == first.year(),
                        is_today: day == today,
                        shown: span,
                    }
                })
                .collect();
            // Three days on from a row's first day is in the week of the row's Monday, whichever
            // day the row starts on.
            MiniWeek {
                number: iso_week(row_first + 3),
                days,
            }
        })
        .collect();

    MiniMonth {
        first: first.to_string(),
        previous: neighbour(first, -1).to_string(),
        next: neighbour(first, 1).to_string(),
        weekdays: (0..7)
            .map(|step| u8::try_from((start_index + step) % 7).expect("0..=6"))
            .collect(),
        weeks,
    }
}

/// The calendars grouped under their accounts, in the order the accounts first appear, which is
/// the account order the list arrives in; never sorted. An account with no calendars has no
/// group. `expanded` answers whether an account's group is open.
#[must_use]
pub fn groups(rows: Vec<CalendarRow>, expanded: impl Fn(&str) -> bool) -> Vec<CalendarGroup> {
    let mut groups: Vec<CalendarGroup> = Vec::new();
    for row in rows {
        if let Some(group) = groups.iter_mut().find(|group| group.account == row.account) {
            group.calendars.push(row);
        } else {
            groups.push(CalendarGroup {
                expanded: expanded(&row.account),
                account: row.account.clone(),
                address: row.account_address.clone(),
                calendars: vec![row],
            });
        }
    }
    groups
}

/// The calendars a new event can be put on, under their accounts: what Settings offers as the
/// default calendar and what an editor's calendar picker lists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewEventCalendars {
    /// Each account's writable calendars, in the account order; an account with none is left out.
    pub groups: Vec<CalendarGroup>,
    /// Whether each group is headed by its account: only while there are two or more. A single
    /// account states itself, and its calendars need nothing to tell them apart.
    pub named: bool,
}

/// `groups` narrowed to the calendars a new event can be put on.
#[must_use]
pub fn new_event_calendars(groups: Vec<CalendarGroup>) -> NewEventCalendars {
    let groups: Vec<CalendarGroup> = groups
        .into_iter()
        .filter_map(|mut group| {
            group.calendars.retain(|calendar| calendar.can_write);
            (!group.calendars.is_empty()).then_some(group)
        })
        .collect();
    NewEventCalendars {
        named: groups.len() > 1,
        groups,
    }
}

fn first_of(date: CalendarDate) -> CalendarDate {
    date_at(from_civil(date.year(), date.month(), 1))
}

/// The first day of the month `step` months from `first`.
fn neighbour(first: CalendarDate, step: i32) -> CalendarDate {
    let index = first.year() * 12 + i32::from(first.month()) - 1 + step;
    let month = u8::try_from(index.rem_euclid(12) + 1).expect("1..=12");
    date_at(from_civil(index.div_euclid(12), month, 1))
}

#[cfg(test)]
#[path = "pane_tests.rs"]
mod pane_tests;
