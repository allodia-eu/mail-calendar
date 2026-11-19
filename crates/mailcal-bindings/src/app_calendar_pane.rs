//! The calendar pane's pulls (`docs/calendar-pane.md`): synchronous and in memory, like the grid's
//! page, so a client re-pulls on `Surface::Calendar`, on `Surface::Settings`, and when the grid
//! comes to rest on another day.

use mailcal_app::calendar_pane::CalendarPaneQuery as AppCalendarPaneQuery;

use crate::{
    CalendarGroup, CalendarLayout, CalendarPane, CalendarPaneQuery, CalendarSeat, MailcalApp,
    app_calendar::today,
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

    /// Every calendar grouped under its account, in the account order: what the pane lists, and
    /// what a phone's calendar manager and a calendar picker group by.
    pub fn calendar_groups(&self) -> Vec<CalendarGroup> {
        self.app
            .calendar_groups()
            .into_iter()
            .map(Into::into)
            .collect()
    }
}
