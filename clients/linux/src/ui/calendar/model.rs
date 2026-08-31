//! Pure calendar navigation and dialog state owned by the Relm4 model.

use mailcal_bindings::{
    CalendarLayout, CalendarPage, CalendarSnapshot, CalendarWriteStatus, MailboxListSnapshot,
    MailcalApp, MonthPage, TimeFormat,
};
use time::Date;

pub(crate) use super::reference::{DeleteRequest, EventIdentity};
use super::{
    date::{
        add_days, add_months, date_heading, month_start, month_title, parse_date, period_title,
        today, today_in,
    },
    drag::CreateSlot,
    editor::{CalendarChoice, EventDetails, EventEditor, EventForm},
};

/// The six shapes selectable from the calendar header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CalendarMode {
    Day,
    ThreeDay,
    WorkWeek,
    Week,
    Month,
    Agenda,
}

impl CalendarMode {
    pub(super) const fn columns(self) -> usize {
        match self {
            Self::Day => 1,
            Self::ThreeDay => 3,
            Self::WorkWeek => 5,
            Self::Week => 7,
            Self::Month | Self::Agenda => 0,
        }
    }

    pub(super) const fn is_grid(self) -> bool {
        matches!(
            self,
            Self::Day | Self::ThreeDay | Self::WorkWeek | Self::Week
        )
    }

    pub(crate) const fn from_index(index: u32) -> Self {
        match index {
            0 => Self::Day,
            1 => Self::ThreeDay,
            2 => Self::WorkWeek,
            4 => Self::Month,
            5 => Self::Agenda,
            _ => Self::Week,
        }
    }

    const fn layout(self) -> CalendarLayout {
        match self {
            Self::Day => CalendarLayout::Day,
            Self::ThreeDay => CalendarLayout::ThreeDay,
            Self::WorkWeek => CalendarLayout::WorkWeek,
            Self::Week => CalendarLayout::Week,
            Self::Month => CalendarLayout::Month,
            Self::Agenda => CalendarLayout::Agenda,
        }
    }
}

impl From<&CalendarLayout> for CalendarMode {
    fn from(layout: &CalendarLayout) -> Self {
        match layout {
            CalendarLayout::Day => Self::Day,
            CalendarLayout::ThreeDay => Self::ThreeDay,
            CalendarLayout::WorkWeek => Self::WorkWeek,
            CalendarLayout::Week => Self::Week,
            CalendarLayout::Month => Self::Month,
            CalendarLayout::Agenda => Self::Agenda,
        }
    }
}

/// One modal surface. The generation lets GTK open it exactly once across model renders.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CalendarDialog {
    Detail(EventDetails),
    Editor(EventEditor),
    ConfirmDelete(DeleteRequest),
}

/// Calendar state; GTK reads it but never mutates it directly.
pub(crate) struct CalendarModel {
    pub(super) mode: CalendarMode,
    pub(super) focus: Date,
    pub(super) week_anchor: Date,
    pub(super) page: CalendarPage,
    pub(super) month: MonthPage,
    pub(super) agenda: CalendarSnapshot,
    pub(super) visible_hours: u8,
    pub(super) use_24_hour: bool,
    /// The app's active display zone. Held rather than fetched per render because the invitation
    /// card reads it too, and the card is drawn from the *mail* side; where there may be no
    /// calendar page yet to take a zone from (mail syncs before calendars).
    zone: String,
    pub(super) write_status: CalendarWriteStatus,
    pub(super) all_day_expanded: bool,
    pub(super) dialog: Option<CalendarDialog>,
    pub(super) dialog_generation: u64,
}

impl CalendarModel {
    pub(crate) fn new(app: Option<&MailcalApp>) -> Self {
        let today = today();
        let Some(app) = app else {
            return Self::empty(today);
        };
        let zone = app.timezone_settings().active;
        let today = today_in(&zone);
        let settings = app.display_settings();
        let mode = CalendarMode::from(&settings.layout);
        #[cfg(any(debug_assertions, feature = "dev-harness"))]
        let mode = if let Ok(value) = std::env::var("MAILCAL_CALENDAR_VIEW") {
            match value.as_str() {
                "day" => CalendarMode::Day,
                "three-day" => CalendarMode::ThreeDay,
                "work-week" => CalendarMode::WorkWeek,
                "month" => CalendarMode::Month,
                "agenda" => CalendarMode::Agenda,
                _ => CalendarMode::Week,
            }
        } else {
            mode
        };
        let week_anchor = align_week(app, today);
        Self {
            mode,
            focus: today,
            week_anchor,
            page: app.calendar_range(
                week_anchor.to_string(),
                range_columns(mode, today, week_anchor),
            ),
            month: app.month_page(today.to_string()),
            agenda: app.calendar_list(),
            visible_hours: settings.visible_hours,
            use_24_hour: matches!(settings.time_format, TimeFormat::TwentyFourHour),
            zone,
            write_status: app.calendar_write_status(),
            all_day_expanded: false,
            dialog: None,
            dialog_generation: 0,
        }
    }

    fn empty(today: Date) -> Self {
        Self {
            mode: CalendarMode::Week,
            focus: today,
            week_anchor: today,
            page: CalendarPage {
                days: Vec::new(),
                timed: Vec::new(),
                all_day: Vec::new(),
                all_day_lanes: 0,
                timezone: String::new(),
                calendars: Vec::new(),
                is_materialized: false,
            },
            month: MonthPage {
                cells: Vec::new(),
                timezone: String::new(),
                calendars: Vec::new(),
                is_materialized: false,
            },
            agenda: CalendarSnapshot {
                events: Vec::new(),
                timezone: String::new(),
            },
            visible_hours: 12,
            use_24_hour: true,
            zone: String::new(),
            write_status: CalendarWriteStatus::Idle,
            all_day_expanded: false,
            dialog: None,
            dialog_generation: 0,
        }
    }

    pub(crate) fn refresh(&mut self, app: &MailcalApp) {
        self.week_anchor = align_week(app, self.focus);
        self.page = app.calendar_range(self.week_anchor.to_string(), self.range_columns());
        self.month = app.month_page(self.focus.to_string());
        self.agenda = app.calendar_list();
    }

    pub(crate) fn refresh_settings(&mut self, app: &MailcalApp) {
        let settings = app.display_settings();
        self.zone = app.timezone_settings().active;
        self.visible_hours = settings.visible_hours;
        self.use_24_hour = matches!(settings.time_format, TimeFormat::TwentyFourHour);
        self.week_anchor = align_week(app, self.focus);
        self.page = app.calendar_range(self.week_anchor.to_string(), self.range_columns());
    }

    pub(crate) fn refresh_write_status(&mut self, app: &MailcalApp) {
        self.write_status = app.calendar_write_status();
    }

    /// The zone every instant on screen is localised into; the calendar's, so the invitation card
    /// and the grid cannot disagree about when a meeting is.
    pub(crate) fn display_zone(&self) -> &str {
        &self.zone
    }

    /// The app's 12/24-hour **setting**, not the locale's default: mail and calendar must not
    /// disagree about whether it is 14:05 or 2:05 PM (`docs/timestamps.md`).
    pub(crate) const fn uses_24_hour(&self) -> bool {
        self.use_24_hour
    }

    /// The calendar write currently settling; what the invitation card's respond row reports.
    pub(crate) const fn write_status(&self) -> CalendarWriteStatus {
        self.write_status
    }

    pub(crate) fn set_mode(&mut self, app: &MailcalApp, mode: CalendarMode) {
        if self.mode == mode {
            return;
        }
        self.mode = mode;
        self.all_day_expanded = false;
        if mode.is_grid() {
            self.week_anchor = align_week(app, self.focus);
        } else if mode == CalendarMode::Month {
            self.focus = month_start(self.focus);
        }
        app.set_calendar_layout(mode.layout());
        self.refresh(app);
    }

    pub(crate) fn step(&mut self, app: &MailcalApp, direction: i32) {
        match self.mode {
            CalendarMode::Day => self.focus = add_days(self.focus, i64::from(direction)),
            CalendarMode::ThreeDay => {
                self.focus = add_days(self.focus, i64::from(direction) * 3);
            }
            CalendarMode::WorkWeek | CalendarMode::Week => {
                self.focus = add_days(self.focus, i64::from(direction) * 7);
            }
            CalendarMode::Month => self.focus = add_months(self.focus, direction),
            CalendarMode::Agenda => return,
        }
        self.all_day_expanded = false;
        self.refresh(app);
    }

    pub(crate) fn jump_today(&mut self, app: &MailcalApp) {
        self.focus = today_in(&app.timezone_settings().active);
        self.all_day_expanded = false;
        self.refresh(app);
    }

    pub(crate) fn show_day(&mut self, app: &MailcalApp, date: &str) {
        let Some(date) = parse_date(date) else {
            return;
        };
        self.focus = date;
        self.mode = CalendarMode::Day;
        self.all_day_expanded = false;
        app.set_calendar_layout(CalendarLayout::Day);
        self.refresh(app);
    }

    pub(crate) fn toggle_all_day(&mut self) {
        self.all_day_expanded = !self.all_day_expanded;
    }

    pub(crate) fn visible_day_range(&self) -> (usize, usize) {
        let columns = self.mode.columns();
        if columns == 0 {
            return (0, 0);
        }
        let focus_index = (self.focus - self.week_anchor).whole_days().clamp(0, 6);
        let focus_index = usize::try_from(focus_index).unwrap_or(0);
        let start = match self.mode {
            CalendarMode::Day | CalendarMode::ThreeDay => focus_index,
            _ => 0,
        };
        (start, columns)
    }

    fn range_columns(&self) -> u32 {
        range_columns(self.mode, self.focus, self.week_anchor)
    }

    pub(super) fn period_title(&self) -> String {
        match self.mode {
            CalendarMode::Month => month_title(self.focus),
            CalendarMode::Agenda => crate::l10n::calendar_view_agenda().to_owned(),
            _ => {
                let (start, columns) = self.visible_day_range();
                let first = add_days(self.week_anchor, i64::try_from(start).unwrap_or(0));
                let last = add_days(first, i64::try_from(columns.saturating_sub(1)).unwrap_or(0));
                if first == last {
                    format!("{} · {}", date_heading(first), month_title(first))
                } else {
                    format!(
                        "{} – {} · {}",
                        date_heading(first),
                        date_heading(last),
                        period_title(first, last)
                    )
                }
            }
        }
    }

    pub(super) fn can_create(&self) -> bool {
        self.page
            .calendars
            .iter()
            .any(|calendar| calendar.can_write)
            || self
                .month
                .calendars
                .iter()
                .any(|calendar| calendar.can_write)
    }

    pub(crate) fn begin_create(&mut self, mailbox: &MailboxListSnapshot) {
        let choices = self.calendar_choices(mailbox, true);
        if choices.is_empty() {
            return;
        }
        self.open_dialog(CalendarDialog::Editor(EventEditor::create(
            choices,
            mailcal_bindings::device_time_zone(),
        )));
    }

    pub(crate) fn begin_create_at(&mut self, mailbox: &MailboxListSnapshot, slot: CreateSlot) {
        let choices = self.calendar_choices(mailbox, true);
        if choices.is_empty() {
            return;
        }
        self.open_dialog(CalendarDialog::Editor(EventEditor::create_slot(
            choices,
            self.zone.clone(),
            slot,
        )));
    }

    pub(crate) fn open_event(&mut self, app: &MailcalApp, event: EventIdentity) {
        // The token travels into the read, so the times are the occurrence's rather than the
        // series'; a series' own start is its *first* occurrence's. What comes back names what
        // the core actually resolved, which is what every question about scope is asked from.
        let Some(detail) = app.event_detail(event.account, event.key, Some(event.occurrence))
        else {
            return;
        };
        let calendar_name = self.calendar_name(&detail.account, &detail.calendar);
        self.open_dialog(CalendarDialog::Detail(EventDetails::from_binding(
            detail,
            calendar_name,
        )));
    }

    pub(crate) fn begin_edit(&mut self, mailbox: &MailboxListSnapshot) {
        let Some(CalendarDialog::Detail(detail)) = self.dialog.clone() else {
            return;
        };
        let choices = self.calendar_choices(mailbox, false);
        self.open_dialog(CalendarDialog::Editor(EventEditor::edit(detail, choices)));
    }

    pub(crate) fn request_delete(&mut self, app: &MailcalApp, mut event: EventIdentity) {
        let Some(detail) = app.event_detail(
            event.account.clone(),
            event.key.clone(),
            Some(event.occurrence.clone()),
        ) else {
            return;
        };
        if detail.can_write {
            // The core's answer, not the caller's: a token that has gone stale names no
            // occurrence, and a delete offered over it would remove the wrong thing.
            event.occurrence.clone_from(&detail.occurrence_start);
            self.open_dialog(CalendarDialog::ConfirmDelete(DeleteRequest {
                identity: event,
                is_recurring: detail.is_recurring,
            }));
        }
    }

    pub(crate) fn request_delete_current(&mut self) {
        let Some(CalendarDialog::Detail(detail)) = &self.dialog else {
            return;
        };
        if detail.can_write {
            self.open_dialog(CalendarDialog::ConfirmDelete(DeleteRequest {
                identity: EventIdentity {
                    account: detail.account.clone(),
                    key: detail.key.clone(),
                    occurrence: detail.occurrence.clone(),
                },
                is_recurring: detail.is_recurring,
            }));
        }
    }

    pub(crate) fn submit_form(
        &mut self,
        form: &EventForm,
        this_occurrence_only: bool,
    ) -> Result<mailcal_bindings::Intent, ()> {
        let Some(CalendarDialog::Editor(editor)) = &self.dialog else {
            return Err(());
        };
        editor.intent(form, this_occurrence_only).map_err(|_| ())
    }

    pub(crate) fn dismiss_dialog(&mut self) {
        self.dialog = None;
    }

    fn open_dialog(&mut self, dialog: CalendarDialog) {
        self.dialog = Some(dialog);
        self.dialog_generation = self.dialog_generation.wrapping_add(1);
    }

    fn calendar_name(&self, account: &str, calendar: &str) -> String {
        self.page
            .calendars
            .iter()
            .chain(self.month.calendars.iter())
            .find(|row| row.account == account && row.id == calendar)
            .map_or_else(|| calendar.to_owned(), |row| row.name.clone())
    }

    fn calendar_choices(
        &self,
        mailbox: &MailboxListSnapshot,
        writable_only: bool,
    ) -> Vec<CalendarChoice> {
        self.page
            .calendars
            .iter()
            .chain(self.month.calendars.iter())
            .filter(|calendar| !writable_only || calendar.can_write)
            .fold(Vec::new(), |mut choices, calendar| {
                if choices.iter().any(|choice: &CalendarChoice| {
                    choice.account == calendar.account && choice.id == calendar.id
                }) {
                    return choices;
                }
                let account = mailbox
                    .accounts
                    .iter()
                    .find(|row| row.id == calendar.account)
                    .map_or(calendar.account.as_str(), |row| row.email.as_str());
                choices.push(CalendarChoice {
                    account: calendar.account.clone(),
                    id: calendar.id.clone(),
                    label: format!("{account} · {}", calendar.name),
                    is_default: calendar.is_default,
                });
                choices
            })
    }
}

fn align_week(app: &MailcalApp, date: Date) -> Date {
    parse_date(&app.week_start_date(date.to_string())).unwrap_or(date)
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;

fn range_columns(mode: CalendarMode, focus: Date, week_anchor: Date) -> u32 {
    let visible = mode.columns();
    if !matches!(mode, CalendarMode::Day | CalendarMode::ThreeDay) {
        return 7;
    }
    let focus_index = usize::try_from((focus - week_anchor).whole_days().clamp(0, 6)).unwrap_or(0);
    u32::try_from(7_usize.max(focus_index.saturating_add(visible))).unwrap_or(7)
}
