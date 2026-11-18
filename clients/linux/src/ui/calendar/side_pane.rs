//! The calendar pane (`docs/calendar-pane.md`): the mini month and the calendars, in the folder
//! pane's column while the calendar is on screen.
//!
//! Everything it draws is one pull, `calendar_pane`. The one thing kept here of its own is the
//! month the person paged to, which follows the grid again as soon as the grid shows other days.

use adw::prelude::*;
use mailcal_bindings::{
    CalendarPane as PaneRecord, CalendarPaneQuery, MailcalApp, PreferenceIntent,
};

use super::{
    date::{add_days, month_start, parse_date, today_in},
    model::{CalendarMode, CalendarModel},
    side_pane_list::CalendarList,
    side_pane_month::MiniMonthView,
};
use crate::ui::{AppInput, preference};

/// What the pane asks the host to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PaneInput {
    /// A `MiniMonth::previous` or `next`, passed back as the query's month.
    Page(String),
    /// A mini-month day, `YYYY-MM-DD`, seated through `calendar_seat`.
    Seat(String),
    /// Opens or shuts one account's group.
    Expand { account: String, expanded: bool },
    /// Shows or hides one calendar.
    Show {
        account: String,
        calendar: String,
        visible: bool,
    },
    /// Colours one calendar from the palette, or `None` for the server's colour.
    Colour {
        account: String,
        calendar: String,
        hex: Option<String>,
    },
}

/// The pane's pulled record and the month paged to.
#[derive(Default)]
pub(crate) struct CalendarSide {
    /// The paged month; `None` follows the grid.
    month: Option<String>,
    /// The grid the last pull described: when it differs, the grid moved and the page is dropped.
    shown: Option<Shown>,
    pane: Option<PaneRecord>,
}

/// The days the grid draws, as the query names them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Shown {
    pub(super) mode: CalendarMode,
    pub(super) anchor: String,
    pub(super) columns: u32,
}

impl Shown {
    /// What `calendar` is drawing: its first shown day and how many, or for the month and the
    /// agenda the day it is focused on.
    pub(super) fn of(calendar: &CalendarModel) -> Self {
        let (anchor, columns) = if calendar.mode.is_grid() {
            let (start, columns) = calendar.visible_day_range();
            (
                add_days(calendar.week_anchor, i64::try_from(start).unwrap_or(0)),
                u32::try_from(columns).unwrap_or(0),
            )
        } else {
            (calendar.focus, 0)
        };
        Self {
            mode: calendar.mode,
            anchor: anchor.to_string(),
            columns,
        }
    }
}

impl CalendarSide {
    /// Reads the pane again: on a calendar or settings signal, and when the grid has moved.
    pub(crate) fn pull(&mut self, app: &MailcalApp, calendar: &CalendarModel) {
        let query = self.query(calendar);
        self.pane = Some(app.calendar_pane(query));
    }

    /// What to ask the core for `calendar`. The paged month is dropped as soon as the grid shows
    /// other days, so the mini month follows the grid again (rule 3).
    pub(super) fn query(&mut self, calendar: &CalendarModel) -> CalendarPaneQuery {
        let shown = Shown::of(calendar);
        if self.shown.as_ref() != Some(&shown) {
            self.month = None;
        }
        let query = CalendarPaneQuery {
            today: today_in(calendar.display_zone()).to_string(),
            layout: shown.mode.layout(),
            anchor: shown.anchor.clone(),
            columns: shown.columns,
            month: self.month.clone(),
        };
        self.shown = Some(shown);
        query
    }

    /// Answers what the pane asked for. A preference is applied before `dispatch` returns, so the
    /// pull that follows already draws it.
    pub(crate) fn input(
        &mut self,
        app: &MailcalApp,
        calendar: &mut CalendarModel,
        input: PaneInput,
    ) {
        match input {
            PaneInput::Page(month) => self.month = Some(month),
            PaneInput::Seat(date) => seat(app, calendar, &date),
            PaneInput::Expand { account, expanded } => preference::set(
                app,
                PreferenceIntent::SetCalendarGroupExpanded { account, expanded },
            ),
            PaneInput::Show {
                account,
                calendar: id,
                visible,
            } => preference::set(
                app,
                PreferenceIntent::SetCalendarVisible {
                    account,
                    calendar: id,
                    visible,
                },
            ),
            PaneInput::Colour {
                account,
                calendar: id,
                hex,
            } => preference::set(
                app,
                PreferenceIntent::SetCalendarColor {
                    account,
                    calendar: id,
                    hex,
                },
            ),
        }
        self.pull(app, calendar);
    }

    #[cfg(test)]
    pub(super) fn set_pane(&mut self, pane: PaneRecord) {
        self.pane = Some(pane);
    }
}

/// Puts the grid where pressing `date` puts it, through the path the layout menu takes: the seat's
/// layout is chosen as the menu chooses one, on the seat's day.
fn seat(app: &MailcalApp, calendar: &mut CalendarModel, date: &str) {
    let seat = app.calendar_seat(date.to_owned(), calendar.mode.layout());
    let Some(anchor) = parse_date(&seat.anchor) else {
        return;
    };
    let mode = CalendarMode::from(&seat.layout);
    calendar.focus = if mode == CalendarMode::Month {
        month_start(anchor)
    } else {
        anchor
    };
    calendar.all_day_expanded = false;
    if calendar.mode == mode {
        calendar.refresh(app);
    } else {
        calendar.set_mode(app, mode);
    }
}

/// The pane's widgets: the mini month over the calendars, which take the rest of the height and
/// scroll on their own. The destinations are the sidebar's bottom bar, under both.
pub(crate) struct SidePane {
    root: gtk::Box,
    month: MiniMonthView,
    calendars: CalendarList,
}

impl SidePane {
    pub(crate) fn new(sender: &relm4::Sender<AppInput>) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.set_widget_name("calendar-pane");
        let month = MiniMonthView::new(sender);
        let calendars = CalendarList::new(sender);
        root.append(month.widget());
        root.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        root.append(calendars.widget());
        Self {
            root,
            month,
            calendars,
        }
    }

    pub(crate) fn widget(&self) -> &gtk::Box {
        &self.root
    }

    /// Draws what was pulled. Nothing is drawn again that has not changed: the app renders on
    /// every input, and a rebuilt day or row loses the keyboard focus that is on it.
    pub(crate) fn render(&mut self, side: &CalendarSide) {
        let Some(pane) = &side.pane else {
            return;
        };
        self.month.render(&pane.month);
        self.calendars.render(&pane.groups);
    }

    #[cfg(test)]
    pub(super) const fn month(&self) -> &MiniMonthView {
        &self.month
    }

    #[cfg(test)]
    pub(super) const fn calendars(&self) -> &CalendarList {
        &self.calendars
    }
}

#[cfg(test)]
#[path = "side_pane_tests.rs"]
mod tests;
