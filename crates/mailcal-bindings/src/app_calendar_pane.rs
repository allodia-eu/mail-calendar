//! The calendar pane's pulls (`docs/calendar-pane.md`): synchronous and in memory, like the grid's
//! page, so a client re-pulls on `Surface::Calendar`, on `Surface::Settings`, and when the grid
//! comes to rest on another day.

use mailcal_app::calendar_pane::CalendarPaneQuery as AppCalendarPaneQuery;

use crate::{
    CalendarGroup, CalendarLayout, CalendarPane, CalendarPaneQuery, CalendarSeat, MailcalApp,
    NewEventCalendars, app_calendar::today,
};

#[uniffi::export]
impl MailcalApp {
    /// The pane beside the grid `query` describes. A malformed `today` or `anchor` reads as today;
    /// a malformed `month` follows the grid.
    pub fn calendar_pane(&self, query: CalendarPaneQuery) -> CalendarPane {
        let today_date = query.today.parse().unwrap_or_else(|_| today());
        self.app
            .calendar_pane(AppCalendarPaneQuery {
                today: today_date,
                layout: query.layout.into(),
                anchor: query.anchor.parse().unwrap_or(today_date),
                columns: query.columns,
                month: query.month.and_then(|month| month.parse().ok()),
            })
            .into()
    }

    /// Where pressing `date` (`YYYY-MM-DD`) in the mini month puts the grid drawing `layout`. A
    /// client applies it through the path its layout menu uses.
    pub fn calendar_seat(&self, date: String, layout: CalendarLayout) -> CalendarSeat {
        let date = date.parse().unwrap_or_else(|_| today());
        self.app.calendar_seat(date, layout.into()).into()
    }

    /// The calendar's focus date (`YYYY-MM-DD`): the day every view is drawn around, `today` (the
    /// client's, in the display zone) until one is set (`docs/calendar.md`, "The focus date").
    pub fn calendar_focus(&self, today: String) -> String {
        let today = today
            .parse()
            .unwrap_or_else(|_| crate::app_calendar::today());
        self.app.calendar_focus(today).to_string()
    }

    /// Puts the focus on `date` (`YYYY-MM-DD`): a day picked in the mini month or the month, Today,
    /// or a day's heading. A malformed date is ignored.
    pub fn set_calendar_focus(&self, date: String) {
        if let Ok(date) = date.parse() {
            self.app.set_calendar_focus(date);
        }
    }

    /// Moves the focus `steps` periods of `layout` (negative goes back), as previous and next do,
    /// and returns where it landed: a week or a month holding `today` lands on today. Draw the
    /// period `calendar_seat` gives for it.
    pub fn step_calendar_focus(&self, layout: CalendarLayout, steps: i32, today: String) -> String {
        let today = today
            .parse()
            .unwrap_or_else(|_| crate::app_calendar::today());
        self.app
            .step_calendar_focus(layout.into(), steps, today)
            .to_string()
    }

    /// Every calendar grouped under its account, in the account order: what the pane lists, and
    /// what a phone's calendar manager and a calendar picker group by.
    pub fn calendar_groups(&self) -> Vec<CalendarGroup> {
        self.app
            .calendar_groups()
            .into_iter()
            .map(Into::into)
            .collect()
    }

    /// The calendars a new event can be put on, under their accounts, and whether each account is
    /// named: what Settings → Calendar offers as the default and what an editor's calendar picker
    /// lists. Read from memory; a client reads it again on `Surface::Calendar`.
    pub fn new_event_calendars(&self) -> NewEventCalendars {
        self.app.new_event_calendars().into()
    }
}
