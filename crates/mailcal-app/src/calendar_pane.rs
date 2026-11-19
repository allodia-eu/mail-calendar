//! The calendar pane on a desktop: the mini month beside the grid and the calendars grouped by
//! account (`docs/calendar-pane.md`).
//!
//! A synchronous pull with arguments, like the grid's page: the client owns where the grid is and
//! what day it is, and the answer comes from the in-memory cache and the preferences, never the
//! store or the network.

use engine_api::{CalendarDate, Provider};
use mailcal_account::CalendarLayout;
use mailcal_viewmodel::calendar::pane::{self, CalendarGroup, MiniMonth, PaneView};

use crate::App;

/// What the pane is asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CalendarPaneQuery {
    /// Today in the display zone, by the client's clock.
    pub today: CalendarDate,
    /// What the grid is drawing.
    pub layout: CalendarLayout,
    /// The first day the grid shows; any day of the month for the month layout.
    pub anchor: CalendarDate,
    /// How many days the grid shows; read only for a time grid.
    pub columns: u32,
    /// The month the person paged the mini month to, or `None` to follow the grid.
    pub month: Option<CalendarDate>,
}

/// The pane, ready to draw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarPane {
    /// The mini month.
    pub month: MiniMonth,
    /// Every calendar, under its account.
    pub groups: Vec<CalendarGroup>,
}

/// Where the grid goes when a day of the mini month is pressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CalendarSeat {
    /// The layout to draw.
    pub layout: CalendarLayout,
    /// The grid's first day, or a day within the month for the month layout.
    pub anchor: CalendarDate,
}

impl<P: Provider> App<P> {
    /// The calendar pane for the grid `query` describes.
    #[must_use]
    pub fn calendar_pane(&self, query: CalendarPaneQuery) -> CalendarPane {
        let view = match query.layout {
            CalendarLayout::Day
            | CalendarLayout::ThreeDay
            | CalendarLayout::WorkWeek
            | CalendarLayout::Week => PaneView::Days {
                from: query.anchor,
                columns: query.columns,
            },
            CalendarLayout::Month => PaneView::Month {
                anchor: query.anchor,
            },
            CalendarLayout::Agenda => PaneView::Agenda,
        };
        CalendarPane {
            month: pane::mini_month(
                &view,
                query.today,
                query.month,
                self.display_settings().week_start.starts_monday(),
            ),
            groups: self.calendar_groups(),
        }
    }

    /// Where pressing `date` in the mini month puts the grid drawing `layout`: a week-shaped layout
    /// keeps its shape and aligns to the week holding the date, a day-shaped one and the month
    /// start at the date, and the agenda, which shows no particular day, opens that day.
    #[must_use]
    pub fn calendar_seat(&self, date: CalendarDate, layout: CalendarLayout) -> CalendarSeat {
        match layout {
            CalendarLayout::Week | CalendarLayout::WorkWeek => CalendarSeat {
                layout,
                anchor: self.week_start_date(date),
            },
            CalendarLayout::Day | CalendarLayout::ThreeDay | CalendarLayout::Month => {
                CalendarSeat {
                    layout,
                    anchor: date,
                }
            }
            CalendarLayout::Agenda => CalendarSeat {
                layout: CalendarLayout::Day,
                anchor: date,
            },
        }
    }

    /// Every calendar grouped under its account, in the account order; what the pane lists and
    /// what a phone's manager and a calendar picker group by.
    #[must_use]
    pub fn calendar_groups(&self) -> Vec<CalendarGroup> {
        let rows = self.calendars();
        let prefs = self
            .calendar_prefs
            .lock()
            .expect("calendar-prefs mutex poisoned");
        pane::groups(rows, |account| prefs.group_expanded(account))
    }

    /// Opens or shuts one account's group in the calendar pane, and persists it. Waits on nothing:
    /// the file is written behind.
    pub fn set_calendar_group_expanded(&self, account: &str, expanded: bool) {
        let changed = self
            .calendar_prefs
            .lock()
            .expect("calendar-prefs mutex poisoned")
            .set_group_expanded(account, expanded);
        if changed {
            self.calendar.resignal();
        }
    }
}

#[cfg(test)]
#[path = "calendar_pane_tests.rs"]
mod tests;
