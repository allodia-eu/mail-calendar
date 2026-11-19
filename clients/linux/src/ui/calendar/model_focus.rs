//! Where the calendar is: the focus date every view is drawn around, held in the core so the
//! four clients agree (`docs/calendar.md`, "The focus date"), and the steps that move it.

use mailcal_bindings::{CalendarLayout, MailcalApp};
use time::Date;

use super::{CalendarMode, CalendarModel};
use crate::ui::calendar::date::{parse_date, today_in};

impl CalendarModel {
    pub(crate) fn step(&mut self, app: &MailcalApp, direction: i32) {
        if self.mode == CalendarMode::Agenda {
            return;
        }
        let today = today_in(&self.zone).to_string();
        let landed = app.step_calendar_focus(self.mode.layout(), direction, today);
        self.focus = parse_date(&landed).unwrap_or(self.focus);
        self.all_day_expanded = false;
        self.refresh(app);
    }

    /// Puts the focus on `date`, in the core too, which every view is drawn around.
    pub(crate) fn focus_on(&mut self, app: &MailcalApp, date: Date) {
        app.set_calendar_focus(date.to_string());
        self.focus = date;
    }

    pub(crate) fn focus_day(&mut self, app: &MailcalApp, date: &str) {
        if let Some(date) = parse_date(date) {
            self.focus_on(app, date);
            self.refresh(app);
        }
    }

    pub(crate) fn jump_today(&mut self, app: &MailcalApp) {
        self.focus_on(app, today_in(&app.timezone_settings().active));
        self.all_day_expanded = false;
        self.refresh(app);
    }

    pub(crate) fn show_day(&mut self, app: &MailcalApp, date: &str) {
        let Some(date) = parse_date(date) else {
            return;
        };
        self.focus_on(app, date);
        self.mode = CalendarMode::Day;
        self.all_day_expanded = false;
        crate::ui::preference::set(
            app,
            mailcal_bindings::PreferenceIntent::SetCalendarLayout {
                layout: CalendarLayout::Day,
            },
        );
        self.refresh(app);
    }
}
