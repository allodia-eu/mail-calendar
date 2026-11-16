//! The month page, and the calendar manager's two writes.
//!
//! The month is a **different layout**, not the time grid with more columns: a cell has no hour
//! axis and no overlap solving, only a list of what happens that day. It reads the same cache
//! through the same discipline: a synchronous pull with an argument, never a pushed snapshot.
//!
//! The manager's writes (show/hide a calendar, override its colour) are applied at page-read time,
//! so an unticked calendar disappears from the grid immediately, with no sync and no network.

use mailcal_app::MonthPage as AppMonthPage;
use mailcal_viewmodel::calendar::{
    color::{PALETTE, event_swatch},
    month::{MonthCell as AppMonthCell, MonthChip as AppMonthChip},
};

use crate::{
    CalendarRow, EventSwatch, MailcalApp, MonthCell, MonthChip, MonthPage, app_calendar::today,
};

#[uniffi::export]
impl MailcalApp {
    /// The month containing `anchor` (a `YYYY-MM-DD` local date; any day *within* the month).
    ///
    /// Always six weeks, so the grid does not change height as the user pages and a "+N more" chip
    /// that fits in February still fits in March.
    ///
    /// A malformed `anchor` falls back to today rather than failing the draw.
    pub fn month_page(&self, anchor: String) -> MonthPage {
        let anchor = anchor.parse().unwrap_or_else(|_| today());
        self.app.month_page(anchor).into()
    }

    /// Every calendar, with the user's decisions applied; what Settings lists.
    ///
    /// A synchronous, in-memory read of the same cache a page is drawn from; no store, no network.
    /// Exactly one row has `is_default` set whenever any calendar can take a write.
    pub fn calendars(&self) -> Vec<CalendarRow> {
        self.app.calendars().into_iter().map(Into::into).collect()
    }
}

/// The calendar colours a user may choose from, as `#rrggbb`.
///
/// The colour picker renders exactly these. Allodia Orange is deliberately **absent**: it means
/// "action" in this product, and a calendar that borrows it makes every event look like a button.
#[uniffi::export]
pub fn calendar_palette() -> Vec<String> {
    PALETTE.iter().map(|hex| (*hex).to_owned()).collect()
}

/// How an event block on the time grid wears the calendar colour `hex` (a `CalendarColor::hex`) in
/// a light or dark theme: a tint to fill it, a bar of the colour down its leading edge, and a title
/// that clears WCAG AA on the tint. A colour that does not parse draws in the palette's first.
#[uniffi::export]
#[must_use]
pub fn calendar_event_swatch(hex: String, dark: bool) -> EventSwatch {
    let swatch = event_swatch(&hex, dark);
    EventSwatch {
        fill: swatch.fill,
        accent: swatch.accent,
        text: swatch.text,
    }
}

impl From<AppMonthPage> for MonthPage {
    fn from(page: AppMonthPage) -> Self {
        Self {
            cells: page.grid.cells.into_iter().map(Into::into).collect(),
            timezone: page.grid.timezone,
            calendars: page.calendars.into_iter().map(Into::into).collect(),
            is_materialized: page.is_materialized,
        }
    }
}

impl From<AppMonthCell> for MonthCell {
    fn from(cell: AppMonthCell) -> Self {
        Self {
            date: cell.date,
            in_month: cell.in_month,
            chips: cell.chips.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<AppMonthChip> for MonthChip {
    fn from(chip: AppMonthChip) -> Self {
        Self {
            account: chip.account,
            event: chip.event,
            calendar: chip.calendar,
            title: chip.title,
            all_day: chip.all_day,
            start_minutes: chip.start_minutes,
            can_write: chip.can_write,
            occurrence_start: chip.occurrence_start,
            participation: chip.participation.into(),
            has_conference: chip.has_conference,
        }
    }
}
