//! The calendar pane on a desktop: the mini month beside the grid and the calendars grouped by
//! account (`docs/calendar-pane.md`).
//!
//! A synchronous pull with arguments, like the grid's page: the client owns where the grid is and
//! what day it is, and the answer comes from the in-memory cache and the preferences, never the
//! store or the network.

use engine_api::{CalendarDate, Provider};
use mailcal_account::CalendarLayout;
use mailcal_viewmodel::calendar::{
    focus::{self, Stride},
    pane::{self, CalendarGroup, MiniMonth, NewEventCalendars, PaneView},
};

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
        let mut month = pane::mini_month(
            &view,
            query.today,
            query.month,
            self.display_settings().week_start.starts_monday(),
        );
        if let Some(focus) = self.focus_date().map(|focus| focus.to_string()) {
            for day in month.weeks.iter_mut().flat_map(|week| week.days.iter_mut()) {
                day.is_focus = day.date == focus;
            }
        }
        CalendarPane {
            month,
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

    /// The calendars a new event can be put on, under their accounts: Settings' default calendar
    /// and every editor's calendar picker.
    #[must_use]
    pub fn new_event_calendars(&self) -> NewEventCalendars {
        pane::new_event_calendars(self.calendar_groups())
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

/// The calendar's **focus date**: the one day every view is drawn around, held here so four
/// clients cannot disagree about where the week opened from a paged month lands
/// (`docs/calendar.md`, "The focus date").
///
/// It is not persisted: a launch opens on today. A client sets it when the person picks a day (the
/// mini month, a month cell, Today, a day's heading) and moves it with [`App::step_calendar_focus`]
/// on previous and next, then draws the period [`App::calendar_seat`] gives for it.
impl<P: Provider> App<P> {
    /// The focus date, or `today` until a client has set one.
    #[must_use]
    pub fn calendar_focus(&self, today: CalendarDate) -> CalendarDate {
        self.focus_date().unwrap_or(today)
    }

    /// Puts the focus on `date`.
    pub fn set_calendar_focus(&self, date: CalendarDate) {
        self.calendar_cache
            .lock()
            .expect("calendar cache poisoned")
            .focus = Some(date);
    }

    /// Moves the focus `steps` periods of `layout` (negative steps go back) and returns where it
    /// landed: a week or a month that holds `today` lands on today itself ([`focus::step`]). The
    /// agenda shows no period, so it leaves the focus where it is.
    pub fn step_calendar_focus(
        &self,
        layout: CalendarLayout,
        steps: i32,
        today: CalendarDate,
    ) -> CalendarDate {
        let stride = match layout {
            CalendarLayout::Day => Stride::Day,
            CalendarLayout::ThreeDay => Stride::ThreeDays,
            CalendarLayout::WorkWeek | CalendarLayout::Week => Stride::Week,
            CalendarLayout::Month => Stride::Month,
            CalendarLayout::Agenda => return self.calendar_focus(today),
        };
        let starts_monday = self.display_settings().week_start.starts_monday();
        let mut cache = self.calendar_cache.lock().expect("calendar cache poisoned");
        let landed = focus::step(
            cache.focus.unwrap_or(today),
            stride,
            steps,
            today,
            starts_monday,
        );
        cache.focus = Some(landed);
        landed
    }

    pub(crate) fn focus_date(&self) -> Option<CalendarDate> {
        self.calendar_cache
            .lock()
            .expect("calendar cache poisoned")
            .focus
    }
}

#[cfg(test)]
#[path = "calendar_pane_tests.rs"]
mod tests;
