//! Settings → Calendar → Default calendar: the calendars a new event can be put on, as the core
//! offers them (`new_event_calendars`), with the core's default checked.
//!
//! Follows the core's calendar signals: the list is drawn again when the calendars offered change,
//! and only the check moves when the default alone did, so the row just pressed keeps its focus.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use adw::prelude::*;
use mailcal_bindings::{CalendarRow, NewEventCalendars, PreferenceIntent};

use super::super::{PageContext, group};
use crate::{l10n, ui::preference};

/// What the list was drawn from: the accounts named, and each calendar's account, id and name.
type Shape = (bool, Vec<(String, String, String, String)>);

fn shape(offered: &NewEventCalendars) -> Shape {
    let calendars = offered
        .groups
        .iter()
        .flat_map(|group| {
            group.calendars.iter().map(|calendar| {
                (
                    group.address.clone(),
                    calendar.account.clone(),
                    calendar.id.clone(),
                    calendar.name.clone(),
                )
            })
        })
        .collect();
    (offered.named, calendars)
}

struct View {
    ctx: PageContext,
    holder: gtk::Box,
    drawn: RefCell<Option<Shape>>,
    /// Each calendar's check, by account and id, while the list is drawn.
    checks: RefCell<Vec<((String, String), gtk::CheckButton)>>,
    /// Set while a check is moved to the core's default, so the move stores nothing.
    moving: Rc<Cell<bool>>,
}

pub(super) fn default_calendar(ctx: &PageContext) -> gtk::Box {
    let holder = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let view = Rc::new(View {
        ctx: ctx.clone(),
        holder: holder.clone(),
        drawn: RefCell::new(None),
        checks: RefCell::new(Vec::new()),
        moving: Rc::new(Cell::new(false)),
    });
    view.follow();
    ctx.calendar.add(move || view.follow());
    holder
}

impl View {
    fn follow(&self) {
        let offered = self.ctx.app.new_event_calendars();
        let now = shape(&offered);
        if self.drawn.borrow().as_ref() == Some(&now) {
            self.moving.set(true);
            for calendar in offered.groups.iter().flat_map(|group| &group.calendars) {
                let key = (calendar.account.clone(), calendar.id.clone());
                if let Some((_, check)) = self.checks.borrow().iter().find(|(at, _)| *at == key)
                    && calendar.is_default
                    && !check.is_active()
                {
                    check.set_active(true);
                }
            }
            self.moving.set(false);
            return;
        }
        if let Some(drawn) = self.holder.first_child() {
            self.holder.remove(&drawn);
        }
        self.holder.append(&self.draw(&offered));
        self.drawn.replace(Some(now));
    }

    fn draw(&self, offered: &NewEventCalendars) -> adw::PreferencesGroup {
        let picker = group(
            l10n::settings_default_calendar_heading(),
            l10n::settings_default_calendar_description(),
        );
        let mut checks = Vec::new();
        if offered.groups.is_empty() {
            picker.add(
                &adw::ActionRow::builder()
                    .title(l10n::settings_default_calendar_none())
                    .use_markup(false)
                    .build(),
            );
        }
        let mut check_group: Option<gtk::CheckButton> = None;
        for account in &offered.groups {
            if offered.named {
                let heading = adw::ActionRow::builder()
                    .title(&account.address)
                    .sensitive(false)
                    .use_markup(false)
                    .build();
                heading.add_css_class("property");
                picker.add(&heading);
            }
            for calendar in &account.calendars {
                let (row, check) = self.row(calendar, &mut check_group);
                picker.add(&row);
                checks.push(((calendar.account.clone(), calendar.id.clone()), check));
            }
        }
        self.checks.replace(checks);
        picker
    }

    fn row(
        &self,
        calendar: &CalendarRow,
        check_group: &mut Option<gtk::CheckButton>,
    ) -> (adw::ActionRow, gtk::CheckButton) {
        let row = adw::ActionRow::builder()
            .title(&calendar.name)
            .use_markup(false)
            .build();
        let selected = gtk::CheckButton::builder()
            .active(calendar.is_default)
            .valign(gtk::Align::Center)
            .build();
        if let Some(first) = check_group {
            selected.set_group(Some(first));
        } else {
            *check_group = Some(selected.clone());
        }
        let app = self.ctx.app.clone();
        let account = calendar.account.clone();
        let id = calendar.id.clone();
        let moving = Rc::clone(&self.moving);
        selected.connect_toggled(move |choice| {
            if choice.is_active() && !moving.get() {
                preference::set(
                    &app,
                    PreferenceIntent::SetDefaultCalendar {
                        account: Some(account.clone()),
                        calendar: Some(id.clone()),
                    },
                );
            }
        });
        row.add_prefix(&selected);
        row.set_activatable_widget(Some(&selected));
        (row, selected)
    }
}
