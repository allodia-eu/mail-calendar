//! The calendar pane's records: the mini month and the calendars grouped by account
//! (`docs/calendar-pane.md`).

use mailcal_app::calendar_pane::{
    CalendarPane as AppCalendarPane, CalendarSeat as AppCalendarSeat,
};
use mailcal_viewmodel::calendar::pane::{
    CalendarGroup as AppCalendarGroup, MiniDay as AppMiniDay, MiniMonth as AppMiniMonth,
    MiniWeek as AppMiniWeek, ShownSpan as AppShownSpan,
};

use crate::{CalendarLayout, CalendarRow, RecurrenceWeekday};

/// What the pane is asked for: where the grid is, and what day it is.
#[derive(uniffi::Record)]
pub struct CalendarPaneQuery {
    /// Today, `YYYY-MM-DD`, in the display zone, by the client's clock.
    pub today: String,
    /// What the grid is drawing.
    pub layout: CalendarLayout,
    /// The first day the grid shows, `YYYY-MM-DD`; any day of the month for the month layout.
    pub anchor: String,
    /// How many days the grid shows; ignored for the month and the agenda.
    pub columns: u32,
    /// The month the person paged the mini month to, as a `MiniMonth::previous` or `next` handed
    /// it; `None` follows the grid. A client clears it when the grid's shown days change.
    pub month: Option<String>,
}

/// Where a day sits in the band marking the days the grid shows, within its own row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum ShownSpan {
    /// Not shown.
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

/// One day of the mini month: a button that seats the grid with `calendar_seat`.
#[derive(uniffi::Record)]
pub struct MiniDay {
    /// The date, `YYYY-MM-DD`.
    pub date: String,
    /// The day of the month, to draw.
    pub day: u32,
    /// Whether the day belongs to the month shown; a client dims the others.
    pub in_month: bool,
    /// Whether it is today: a filled accent circle.
    pub is_today: bool,
    /// Its place in the band of shown days.
    pub shown: ShownSpan,
}

/// One row of the mini month.
#[derive(uniffi::Record)]
pub struct MiniWeek {
    /// The ISO 8601 week number, whatever the week start.
    pub number: u32,
    /// Seven days, in week-start order.
    pub days: Vec<MiniDay>,
}

/// The mini month: always six weeks.
#[derive(uniffi::Record)]
pub struct MiniMonth {
    /// The month's first day, `YYYY-MM-01`; the client words the title.
    pub first: String,
    /// What the previous arrow passes back as `CalendarPaneQuery::month`.
    pub previous: String,
    /// What the next arrow passes back.
    pub next: String,
    /// The weekdays heading the columns, in week-start order; the client words each initial.
    pub weekdays: Vec<RecurrenceWeekday>,
    /// Six rows.
    pub weeks: Vec<MiniWeek>,
}

/// One account's calendars.
#[derive(uniffi::Record)]
pub struct CalendarGroup {
    /// The account's id, for `PreferenceIntent::SetCalendarGroupExpanded`.
    pub account: String,
    /// The account's address: the heading.
    pub address: String,
    /// Whether the group shows its calendars.
    pub expanded: bool,
    /// Its calendars, in the account's order.
    pub calendars: Vec<CalendarRow>,
}

/// The calendar pane.
#[derive(uniffi::Record)]
pub struct CalendarPane {
    /// The mini month.
    pub month: MiniMonth,
    /// Every calendar under its account, in the account order.
    pub groups: Vec<CalendarGroup>,
}

/// Where a pressed mini-month day puts the grid; applied the way the layout menu's choice is.
#[derive(uniffi::Record)]
pub struct CalendarSeat {
    /// The layout to draw.
    pub layout: CalendarLayout,
    /// The grid's first day, `YYYY-MM-DD`, or a day within the month for the month layout.
    pub anchor: String,
}

impl From<AppShownSpan> for ShownSpan {
    fn from(span: AppShownSpan) -> Self {
        match span {
            AppShownSpan::Outside => Self::Outside,
            AppShownSpan::Only => Self::Only,
            AppShownSpan::Start => Self::Start,
            AppShownSpan::Middle => Self::Middle,
            AppShownSpan::End => Self::End,
        }
    }
}

/// The weekday at a Monday-based index, `0` = Monday.
fn weekday(index: u8) -> RecurrenceWeekday {
    match index % 7 {
        0 => RecurrenceWeekday::Monday,
        1 => RecurrenceWeekday::Tuesday,
        2 => RecurrenceWeekday::Wednesday,
        3 => RecurrenceWeekday::Thursday,
        4 => RecurrenceWeekday::Friday,
        5 => RecurrenceWeekday::Saturday,
        _ => RecurrenceWeekday::Sunday,
    }
}

impl From<AppMiniDay> for MiniDay {
    fn from(day: AppMiniDay) -> Self {
        Self {
            date: day.date,
            day: day.day,
            in_month: day.in_month,
            is_today: day.is_today,
            shown: day.shown.into(),
        }
    }
}

impl From<AppMiniWeek> for MiniWeek {
    fn from(week: AppMiniWeek) -> Self {
        Self {
            number: week.number,
            days: week.days.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<AppMiniMonth> for MiniMonth {
    fn from(month: AppMiniMonth) -> Self {
        Self {
            first: month.first,
            previous: month.previous,
            next: month.next,
            weekdays: month.weekdays.into_iter().map(weekday).collect(),
            weeks: month.weeks.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<AppCalendarGroup> for CalendarGroup {
    fn from(group: AppCalendarGroup) -> Self {
        Self {
            account: group.account,
            address: group.address,
            expanded: group.expanded,
            calendars: group.calendars.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<AppCalendarPane> for CalendarPane {
    fn from(pane: AppCalendarPane) -> Self {
        Self {
            month: pane.month.into(),
            groups: pane.groups.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<AppCalendarSeat> for CalendarSeat {
    fn from(seat: AppCalendarSeat) -> Self {
        Self {
            layout: seat.layout.into(),
            anchor: seat.anchor.to_string(),
        }
    }
}

#[cfg(test)]
#[path = "records_calendar_pane_tests.rs"]
mod tests;
