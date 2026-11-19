//! The calendar pane's mini month (`docs/calendar-pane.md`, rules 2 to 5 and 11).
//!
//! Built once and relabelled on each render rather than rebuilt: six weeks is always six weeks
//! (rule 2), and a rebuilt day would take the keyboard focus off the day a person is on.

use std::{cell::RefCell, rc::Rc};

use adw::prelude::*;
use gtk::accessible::Property as AccessibleProperty;
use mailcal_bindings::{MiniDay, MiniMonth, ShownSpan};

use super::{
    date::{long_date, month_title, parse_date, weekday_abbrev, weekday_full},
    repeat::iso_weekday,
    side_pane::PaneInput,
};
use crate::{
    l10n,
    ui::{AppInput, icons},
};

/// Rows of the mini month, always.
const WEEKS: usize = 6;
/// Days in a row.
const DAYS: usize = 7;

/// The mini month's widgets, and the dates its days stand for.
pub(crate) struct MiniMonthView {
    root: gtk::Box,
    title: gtk::Label,
    /// What the previous and next arrows pass back.
    pages: Rc<RefCell<(String, String)>>,
    weekdays: Vec<gtk::Label>,
    weeks: Vec<gtk::Label>,
    cells: Vec<DayCell>,
    /// The date each day stands for, in the order of `cells`.
    dates: Rc<RefCell<Vec<String>>>,
}

/// One day: the band behind it, and the round button on it.
struct DayCell {
    band: gtk::Box,
    day: gtk::Button,
}

impl MiniMonthView {
    pub(super) fn new(sender: &relm4::Sender<AppInput>) -> Self {
        install_styles();
        let root = gtk::Box::new(gtk::Orientation::Vertical, 6);
        root.add_css_class(MONTH);
        root.set_margin_start(8);
        root.set_margin_end(8);
        root.set_margin_top(8);
        root.set_margin_bottom(8);

        let pages = Rc::new(RefCell::new((String::new(), String::new())));
        let header = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let title = gtk::Label::new(None);
        title.set_xalign(0.0);
        title.set_hexpand(true);
        title.set_margin_start(6);
        title.add_css_class("heading");
        header.append(&title);
        for (icon, label, next) in [
            (icons::PREVIOUS, l10n::calendar_prev_month(), false),
            (icons::NEXT, l10n::calendar_next_month(), true),
        ] {
            let button = gtk::Button::from_icon_name(icon);
            button.add_css_class("flat");
            button.set_tooltip_text(Some(label));
            button.update_property(&[AccessibleProperty::Label(label)]);
            let (pages, input) = (Rc::clone(&pages), sender.clone());
            button.connect_clicked(move |_| {
                let pages = pages.borrow();
                let month = if next { &pages.1 } else { &pages.0 };
                if !month.is_empty() {
                    input.emit(AppInput::CalendarPane(PaneInput::Page(month.clone())));
                }
            });
            header.append(&button);
        }
        root.append(&header);

        let grid = gtk::Grid::new();
        grid.set_column_homogeneous(true);
        grid.set_row_spacing(2);
        let weekdays = (0..DAYS)
            .map(|column| {
                let label = caption();
                grid.attach(&label, column_of(column), 0, 1, 1);
                label
            })
            .collect();
        let dates = Rc::new(RefCell::new(vec![String::new(); WEEKS * DAYS]));
        let mut weeks = Vec::with_capacity(WEEKS);
        let mut cells = Vec::with_capacity(WEEKS * DAYS);
        for row in 0..WEEKS {
            let number = caption();
            grid.attach(&number, 0, row_of(row), 1, 1);
            weeks.push(number);
            for column in 0..DAYS {
                let cell = day_cell(cells.len(), &dates, sender);
                grid.attach(&cell.band, column_of(column), row_of(row), 1, 1);
                cells.push(cell);
            }
        }
        arrow_keys(&cells);
        root.append(&grid);

        Self {
            root,
            title,
            pages,
            weekdays,
            weeks,
            cells,
            dates,
        }
    }

    pub(super) fn widget(&self) -> &gtk::Box {
        &self.root
    }

    pub(super) fn render(&self, month: &MiniMonth) {
        if let Some(first) = parse_date(&month.first) {
            self.title.set_label(&month_title(first));
        }
        *self.pages.borrow_mut() = (month.previous.clone(), month.next.clone());
        for (label, weekday) in self.weekdays.iter().zip(&month.weekdays) {
            let iso = iso_weekday(weekday);
            label.set_label(&initial(iso));
            let name = weekday_full(iso);
            label.set_tooltip_text(Some(&name));
            label.update_property(&[AccessibleProperty::Label(&name)]);
        }
        let mut dates = self.dates.borrow_mut();
        for (row, week) in month.weeks.iter().enumerate().take(WEEKS) {
            if let Some(label) = self.weeks.get(row) {
                let number = week.number.to_string();
                label.set_label(&number);
                let spoken = l10n::calendar_week_number(&number);
                label.set_tooltip_text(Some(&spoken));
                label.update_property(&[AccessibleProperty::Label(&spoken)]);
            }
            for (column, day) in week.days.iter().enumerate().take(DAYS) {
                let index = row * DAYS + column;
                if let (Some(cell), Some(date)) = (self.cells.get(index), dates.get_mut(index)) {
                    cell.show(day);
                    date.clone_from(&day.date);
                }
            }
        }
    }

    /// The day button for `date`, as drawn.
    #[cfg(test)]
    pub(super) fn day(&self, date: &str) -> Option<gtk::Button> {
        let dates = self.dates.borrow();
        let index = dates.iter().position(|shown| shown == date)?;
        self.cells.get(index).map(|cell| cell.day.clone())
    }

    /// The band behind the day for `date`.
    #[cfg(test)]
    pub(super) fn band(&self, date: &str) -> Option<gtk::Box> {
        let dates = self.dates.borrow();
        let index = dates.iter().position(|shown| shown == date)?;
        self.cells.get(index).map(|cell| cell.band.clone())
    }

    #[cfg(test)]
    pub(super) fn title(&self) -> String {
        self.title.label().to_string()
    }
}

impl DayCell {
    fn show(&self, day: &MiniDay) {
        self.day.set_label(&day.day.to_string());
        // What the UI suite presses a day by.
        self.day.set_widget_name(&format!("mini-day-{}", day.date));
        let mut classes = vec!["flat", DAY];
        if day.is_today {
            classes.push(TODAY);
        }
        if day.is_focus {
            classes.push(FOCUS);
        }
        if !day.in_month {
            classes.push(OUTSIDE);
        }
        self.day.set_css_classes(&classes);
        self.band.set_css_classes(&[CELL, band_class(day.shown)]);
        self.day
            .update_property(&[AccessibleProperty::Label(&spoken(day))]);
    }
}

/// What a day says to a screen reader: the full date, then whether it is today and whether the
/// grid shows it.
pub(super) fn spoken(day: &MiniDay) -> String {
    let mut parts = vec![parse_date(&day.date).map_or_else(|| day.date.clone(), long_date)];
    if day.is_today {
        parts.push(l10n::calendar_today().to_owned());
    }
    if day.is_focus {
        parts.push(l10n::a11y_calendar_focus_day().to_owned());
    }
    if day.shown != ShownSpan::Outside {
        parts.push(l10n::a11y_calendar_pane_day_shown().to_owned());
    }
    parts.join(", ")
}

/// The band's style class: rounded at each end it has in this row.
pub(super) const fn band_class(span: ShownSpan) -> &'static str {
    match span {
        ShownSpan::Outside => "mailcal-mini-outside-band",
        ShownSpan::Only => "mailcal-mini-only",
        ShownSpan::Start => "mailcal-mini-start",
        ShownSpan::Middle => "mailcal-mini-middle",
        ShownSpan::End => "mailcal-mini-end",
    }
}

/// A weekday's initial: the first letter of the locale's own abbreviation, which is what a mini
/// month heads a column with on every platform. The full name is the column's spoken label.
pub(super) fn initial(iso: u8) -> String {
    weekday_abbrev(iso)
        .chars()
        .next()
        .map(|letter| letter.to_uppercase().collect())
        .unwrap_or_default()
}

fn day_cell(
    index: usize,
    dates: &Rc<RefCell<Vec<String>>>,
    sender: &relm4::Sender<AppInput>,
) -> DayCell {
    let band = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    band.set_hexpand(true);
    let day = gtk::Button::new();
    day.set_halign(gtk::Align::Center);
    day.set_hexpand(true);
    let (dates, input) = (Rc::clone(dates), sender.clone());
    day.connect_clicked(move |_| {
        if let Some(date) = dates.borrow().get(index).filter(|date| !date.is_empty()) {
            input.emit(AppInput::CalendarPane(PaneInput::Seat(date.clone())));
        }
    });
    band.append(&day);
    DayCell { band, day }
}

/// Arrow keys move between days: a day either side, a week up or down, and nowhere past the
/// mini month's edge.
fn arrow_keys(cells: &[DayCell]) {
    let days = Rc::new(
        cells
            .iter()
            .map(|cell| cell.day.downgrade())
            .collect::<Vec<_>>(),
    );
    for (index, cell) in cells.iter().enumerate() {
        let keys = gtk::EventControllerKey::new();
        let (days, button) = (Rc::clone(&days), cell.day.downgrade());
        keys.connect_key_pressed(move |_, key, _, _| {
            let rtl = button
                .upgrade()
                .is_some_and(|button| button.direction() == gtk::TextDirection::Rtl);
            let Some(target) = arrow_target(index, key, rtl) else {
                return gtk::glib::Propagation::Proceed;
            };
            if let Some(day) = days.get(target).and_then(gtk::glib::WeakRef::upgrade) {
                day.grab_focus();
            }
            gtk::glib::Propagation::Stop
        });
        cell.day.add_controller(keys);
    }
}

/// Where an arrow key moves the focus from the day at `index`, or `None` for a key that is not
/// an arrow. An arrow at the edge stays on its day.
pub(super) fn arrow_target(index: usize, key: gtk::gdk::Key, rtl: bool) -> Option<usize> {
    use gtk::gdk::Key;
    let last = WEEKS * DAYS - 1;
    let (back, forward) = if rtl {
        (Key::Right, Key::Left)
    } else {
        (Key::Left, Key::Right)
    };
    let target = if key == back {
        index.checked_sub(1)
    } else if key == forward {
        Some(index + 1)
    } else if key == Key::Up {
        index.checked_sub(DAYS)
    } else if key == Key::Down {
        Some(index + DAYS)
    } else {
        return None;
    };
    Some(target.filter(|target| *target <= last).unwrap_or(index))
}

fn caption() -> gtk::Label {
    let label = gtk::Label::new(None);
    label.add_css_class("dim-label");
    label.add_css_class(CAPTION);
    label
}

/// The grid column of a day: the week numbers lead.
fn column_of(day: usize) -> i32 {
    i32::try_from(day + 1).unwrap_or(1)
}

/// The grid row of a week: the weekday initials lead.
fn row_of(week: usize) -> i32 {
    i32::try_from(week + 1).unwrap_or(1)
}

const MONTH: &str = "mailcal-mini-month";
const CELL: &str = "mailcal-mini-cell";
const DAY: &str = "mailcal-mini-day";
const TODAY: &str = "mailcal-mini-today";
const FOCUS: &str = "mailcal-mini-focus";
const OUTSIDE: &str = "mailcal-mini-dimmed";
const CAPTION: &str = "mailcal-mini-caption";

/// A day is a 24 px circle, which is what an eighth of the narrowest pane holds. The columns take
/// the pane's width between them, so the band that joins the shown days stretches with the pane
/// while every day stays round.
fn css() -> String {
    let band = "background-color: color-mix(in srgb, var(--accent-bg-color) 22%, transparent);";
    format!(
        "box.{CELL} {{ min-height: 24px; }}
button.{DAY} {{ min-width: 24px; min-height: 24px; padding: 0; border-radius: 9999px; font-feature-settings: \"tnum\"; }}
button.{DAY}.{TODAY} {{ background-color: var(--accent-bg-color); color: var(--accent-fg-color); font-weight: 700; }}
button.{DAY}.{FOCUS} {{ box-shadow: inset 0 0 0 2px var(--accent-color); }}
button.{DAY}.{OUTSIDE} {{ opacity: 0.5; }}
label.{CAPTION} {{ font-size: 0.8em; font-feature-settings: \"tnum\"; }}
box.mailcal-mini-only {{ {band} border-radius: 12px; }}
box.mailcal-mini-middle {{ {band} border-radius: 0; }}
box.mailcal-mini-start {{ {band} border-radius: 12px 0 0 12px; }}
box.mailcal-mini-start:dir(rtl) {{ border-radius: 0 12px 12px 0; }}
box.mailcal-mini-end {{ {band} border-radius: 0 12px 12px 0; }}
box.mailcal-mini-end:dir(rtl) {{ border-radius: 12px 0 0 12px; }}"
    )
}

fn install_styles() {
    static INSTALLED: std::sync::Once = std::sync::Once::new();
    if let Some(display) = gtk::gdk::Display::default() {
        INSTALLED.call_once(|| {
            let provider = gtk::CssProvider::new();
            provider.load_from_string(&css());
            gtk::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        });
    }
}
