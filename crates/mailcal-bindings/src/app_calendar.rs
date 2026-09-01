//! The calendar-grid method on [`MailcalApp`], and the view-model → FFI conversions.
//!
//! `calendar_page` is a **direct, synchronous query**, unlike every other read here, which
//! pulls a snapshot slot after an observer signal. That is deliberate: a calendar is paged
//! through continuously, and a client renders the page either side of the one in view so
//! the next swipe is instant. One snapshot slot cannot hold three pages, and `dispatch`
//! is fire-and-forget on a multi-threaded runtime, so two quick swipes would race and the
//! grid could settle on *last* week after the user had already swiped to next.
//!
//! So the client owns the anchor and asks for what it wants. A pull cannot arrive out of
//! order. The call never touches the store or the network: it reads an in-memory cache,
//! which `Surface::Calendar` (now a cache-invalidation signal) tells the client to re-pull.

use mailcal_app::{CalendarPage as AppCalendarPage, EventDetail as AppEventDetail, EventRef};
use mailcal_viewmodel::calendar::{
    CalendarRow as AppCalendarRow,
    color::{CalendarColor as AppCalendarColor, Swatch as AppSwatch},
    grid::{AllDayBand as AppAllDayBand, GridDay as AppGridDay, TimedSegment as AppTimedSegment},
};

use crate::{
    AllDayBand, CalendarColor, CalendarPage, CalendarRow, EventAttendee, EventDetail, GridDay,
    MailcalApp, ProposedEdit, SeriesEditWarning, Swatch, TimedSegment,
};

#[uniffi::export]
impl MailcalApp {
    /// The calendar page a grid draws: `columns` consecutive days starting at `from`, laid out.
    ///
    /// The days are **consecutive from the anchor** and snapped to nothing. That is what lets a
    /// client zoom the day axis without the grid relocating: widening three columns to seven keeps
    /// the same first day, so the days the user was reading stay put. Snapping to a Monday-aligned
    /// week instead would have to jump; it cannot contain an arbitrary three-day window.
    ///
    /// Week alignment is a separate, deliberate act: call [`MailcalApp::week_start_date`] when the
    /// user *picks* a week, not every time the column count changes.
    ///
    /// Synchronous and cheap; read it directly while paging, and call it again for the
    /// neighbouring pages to prefetch them. It never blocks on the store or the network.
    ///
    /// A malformed `from` falls back to today rather than failing the draw: a blank screen is a
    /// worse answer to a host bug than the wrong week.
    pub fn calendar_range(&self, from: String, columns: u32) -> CalendarPage {
        let from = from.parse().unwrap_or_else(|_| today());
        self.app.calendar_range(from, columns.clamp(1, 14)).into()
    }

    /// The first day of the week containing `date` (`YYYY-MM-DD`), per the user's week-start
    /// setting.
    ///
    /// The core owns which day a week begins on so three clients cannot disagree; get it wrong and
    /// every column shifts, so the user reads Tuesday's meetings under Monday's heading.
    pub fn week_start_date(&self, date: String) -> String {
        let date = date.parse().unwrap_or_else(|_| today());
        self.app.week_start_date(date).to_string()
    }

    /// The full detail of one stored event (by its row's `account` + `key`), or `None` if it is
    /// not in the store: the detail view a tap opens, and what the editor prefills from.
    ///
    /// `occurrence` is the token the tapped surface carried
    /// (`TimedSegment::occurrence_start` and its siblings), passed back **verbatim**. Send it
    /// and the times are that occurrence's; send `None`/empty, which is all an agenda row and
    /// a one-off event have, and they are the series'. A series' own start is its **first**
    /// occurrence's, so a client that drops the token here shows September's standup as
    /// August's, and prefills an editor that would write that date back.
    ///
    /// A local read: no network, no expansion. A malformed `account`/`key` pair is treated as
    /// "not found" rather than an error, so a stale reference costs the user a closed sheet, not
    /// a crash.
    pub fn event_detail(
        &self,
        account: String,
        key: String,
        occurrence: Option<String>,
    ) -> Option<EventDetail> {
        let event = EventRef::from_parts(&account, key)?;
        self.runtime
            .block_on(self.app.event_detail(&event, occurrence.as_deref()))
            .map(Into::into)
    }

    /// What saving this edit over the **whole series** would cost the occurrences the user
    /// changed individually, or `None` when there is nothing to say.
    ///
    /// Ask it with the payload about to be dispatched: the same values as
    /// `Intent::UpdateEvent`, in the same three-state form, and show what comes back between
    /// Save and the write. `None` means save straight away; a series with no per-occurrence
    /// work, a server that keeps it, and an edit that does not touch what would be lost all
    /// answer `None`, which is what keeps the dialog rare enough to be worth reading.
    ///
    /// **Never ask it for a single-occurrence edit.** That writes an override of its own and
    /// costs no other occurrence anything; the answer is `None` by construction.
    ///
    /// A local read, and not on any path the user waits on: it happens when Save is pressed.
    pub fn series_edit_warning(
        &self,
        account: String,
        key: String,
        edit: ProposedEdit,
    ) -> Option<SeriesEditWarning> {
        let event = EventRef::from_parts(&account, key)?;
        let edit = edit.into_account_edit()?;
        self.runtime
            .block_on(self.app.series_edit_warning(&event, &edit))
            .map(Into::into)
    }
}

/// Today's date in UTC: only the fallback for an unparseable anchor. Shared with the month page.
pub(crate) fn today() -> engine_api::CalendarDate {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    mailcal_viewmodel::calendar::days::date_at(i64::try_from(seconds / 86_400).unwrap_or_default())
}

impl From<AppEventDetail> for EventDetail {
    fn from(detail: AppEventDetail) -> Self {
        Self {
            account: detail.account,
            key: detail.key,
            calendar: detail.calendar,
            title: detail.title,
            all_day: detail.all_day,
            timezone: detail.timezone,
            start: detail.start,
            end: detail.end,
            location: detail.location,
            notes: detail.notes,
            reminder_minutes: detail.reminder_minutes,
            recurrence: detail.recurrence.map(Into::into),
            is_recurring: detail.is_recurring,
            can_write: detail.can_write,
            repeat_summary: detail.repeat_summary.map(Into::into),
            repeat_draft: detail.repeat_draft.map(Into::into),
            occurrence_start: detail.occurrence_start,
            attendees: detail
                .attendees
                .into_iter()
                .map(|attendee| EventAttendee {
                    name: attendee.name,
                    email: attendee.email,
                    is_organizer: attendee.is_organizer,
                    response: attendee.response.into(),
                })
                .collect(),
        }
    }
}

impl From<AppCalendarPage> for CalendarPage {
    fn from(page: AppCalendarPage) -> Self {
        Self {
            days: page.grid.days.into_iter().map(Into::into).collect(),
            timed: page.grid.timed.into_iter().map(Into::into).collect(),
            all_day: page.grid.all_day.into_iter().map(Into::into).collect(),
            all_day_lanes: page.grid.all_day_lanes,
            timezone: page.grid.timezone,
            calendars: page.calendars.into_iter().map(Into::into).collect(),
            is_materialized: page.is_materialized,
        }
    }
}

impl From<AppGridDay> for GridDay {
    fn from(day: AppGridDay) -> Self {
        Self { date: day.date }
    }
}

impl From<AppTimedSegment> for TimedSegment {
    fn from(segment: AppTimedSegment) -> Self {
        Self {
            account: segment.account,
            event: segment.event,
            calendar: segment.calendar,
            title: segment.title,
            day: segment.day,
            start_minutes: segment.start_minutes,
            end_minutes: segment.end_minutes,
            column: segment.column,
            columns: segment.columns,
            continues_before: segment.continues_before,
            continues_after: segment.continues_after,
            can_write: segment.can_write,
            can_move: segment.can_move,
            occurrence_start: segment.occurrence_start,
            participation: segment.participation.into(),
        }
    }
}

impl From<AppAllDayBand> for AllDayBand {
    fn from(band: AppAllDayBand) -> Self {
        Self {
            account: band.account,
            event: band.event,
            calendar: band.calendar,
            title: band.title,
            day: band.day,
            days: band.days,
            lane: band.lane,
            continues_before: band.continues_before,
            continues_after: band.continues_after,
            can_write: band.can_write,
            occurrence_start: band.occurrence_start,
            participation: band.participation.into(),
        }
    }
}

impl From<AppCalendarRow> for CalendarRow {
    fn from(row: AppCalendarRow) -> Self {
        Self {
            account: row.account,
            id: row.id,
            name: row.name,
            color: row.color.into(),
            visible: row.visible,
            can_write: row.can_write,
            is_default: row.is_default,
        }
    }
}

impl From<AppCalendarColor> for CalendarColor {
    fn from(color: AppCalendarColor) -> Self {
        Self {
            hex: color.hex,
            light: color.light.into(),
            dark: color.dark.into(),
        }
    }
}

impl From<AppSwatch> for Swatch {
    fn from(swatch: AppSwatch) -> Self {
        Self {
            background: swatch.background,
            text: swatch.text,
            border: swatch.border,
        }
    }
}
