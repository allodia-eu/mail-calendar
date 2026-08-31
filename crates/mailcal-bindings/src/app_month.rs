//! The month page, and the calendar manager's two writes.
//!
//! The month is a **different layout**, not the time grid with more columns: a cell has no hour
//! axis and no overlap solving, only a list of what happens that day. It reads the same cache
//! through the same discipline: a synchronous pull with an argument, never a pushed snapshot.
//!
//! The manager's writes (show/hide a calendar, override its colour) are applied at page-read time,
//! so an unticked calendar disappears from the grid immediately, with no sync and no network.

use mailcal_account::DefaultCalendar;
use mailcal_app::MonthPage as AppMonthPage;
use mailcal_viewmodel::calendar::{
    color::PALETTE,
    month::{MonthCell as AppMonthCell, MonthChip as AppMonthChip},
};

use crate::{CalendarRow, MailcalApp, MonthCell, MonthChip, MonthPage, app_calendar::today};

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

    /// Shows or hides one calendar's events, and persists the choice.
    ///
    /// Keyed on account **and** calendar: a calendar id is unique only within its account, so two
    /// accounts can each have a `work` calendar and hiding one must not hide the other.
    pub fn set_calendar_visible(&self, account: String, calendar: String, visible: bool) {
        self.runtime
            .block_on(self.app.set_calendar_visible(&account, &calendar, visible));
    }

    /// Overrides one calendar's colour, or clears the override (`None`) back to the server's.
    ///
    /// The hex is **not trusted**: the core snaps it to the nearest palette entry, so a client
    /// cannot introduce an off-palette colour; including Allodia Orange, which is reserved for
    /// actions.
    pub fn set_calendar_color(&self, account: String, calendar: String, hex: Option<String>) {
        self.runtime
            .block_on(self.app.set_calendar_color(&account, &calendar, hex));
    }

    /// Every calendar, with the user's decisions applied; what Settings lists.
    ///
    /// A synchronous, in-memory read of the same cache a page is drawn from; no store, no network.
    /// Exactly one row has `is_default` set whenever any calendar can take a write.
    pub fn calendars(&self) -> Vec<CalendarRow> {
        self.app.calendars().into_iter().map(Into::into).collect()
    }

    /// Chooses the calendar new events are filed on unless the user picks another in the editor.
    ///
    /// Passing `None` for either half clears the choice, which is **not** the same as naming a
    /// calendar: it returns to "whichever writable calendar comes first", so an account connected
    /// later can become the default on its own. The choice is not trusted to still be valid; the
    /// core resolves it against the calendars that exist every time it builds the list, and
    /// `CalendarRow::is_default` is the answer.
    pub fn set_default_calendar(&self, account: Option<String>, calendar: Option<String>) {
        let choice = account
            .zip(calendar)
            .map(|(account, calendar)| DefaultCalendar { account, calendar });
        self.runtime.block_on(self.app.set_default_calendar(choice));
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
        }
    }
}
