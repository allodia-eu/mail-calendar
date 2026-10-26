//! The agenda and the month grid are drawn again only when what they show has changed: the app
//! renders on every input it receives, and a redraw replaces the buttons a press or the keyboard
//! focus is on.

use adw::prelude::*;
use mailcal_bindings::{EventRow, MonthCell, MonthChip, ResponseStatus};
use time::{Date, Month};

use super::{
    model::CalendarModel,
    views::{render_agenda, render_month},
};
use crate::ui::AppInput;

fn model() -> CalendarModel {
    let mut model =
        CalendarModel::empty(Date::from_calendar_date(2026, Month::October, 26).unwrap());
    model.page.is_materialized = true;
    model.month.is_materialized = true;
    model
}

fn agenda_showing(title: &str) -> CalendarModel {
    let mut model = model();
    model.agenda.events = vec![EventRow {
        account: "alice@test.local".to_owned(),
        key: "standup".to_owned(),
        title: title.to_owned(),
        start: "2026-10-26T08:00:00Z".to_owned(),
        can_write: true,
        participation: ResponseStatus::Accepted,
    }];
    model
}

fn month_showing(title: &str) -> CalendarModel {
    let mut model = model();
    model.month.cells = vec![MonthCell {
        date: "2026-10-26".to_owned(),
        in_month: true,
        chips: vec![MonthChip {
            account: "alice@test.local".to_owned(),
            event: "standup".to_owned(),
            calendar: "calendar-a".to_owned(),
            title: title.to_owned(),
            all_day: false,
            start_minutes: 540,
            can_write: true,
            occurrence_start: String::new(),
            participation: ResponseStatus::Accepted,
        }],
    }];
    model
}

pub(crate) fn an_unchanged_agenda_keeps_its_rows_and_a_changed_one_is_drawn_again() {
    let (sender, _receiver) = relm4::channel::<AppInput>();
    let list = gtk::ListBox::new();
    let mut drawn = None;
    render_agenda(&list, &agenda_showing("Standup"), &sender, &mut drawn);
    let row = list.row_at_index(0).expect("the agenda drew its event");

    render_agenda(&list, &agenda_showing("Standup"), &sender, &mut drawn);
    assert_eq!(
        list.row_at_index(0).as_ref(),
        Some(&row),
        "a render that changed nothing replaced the row a press may be landing on"
    );

    render_agenda(
        &list,
        &agenda_showing("Standup, moved"),
        &sender,
        &mut drawn,
    );
    assert_ne!(
        list.row_at_index(0).as_ref(),
        Some(&row),
        "the agenda went on showing an event that had changed"
    );
}

pub(crate) fn an_unchanged_month_keeps_its_cells_and_a_changed_one_is_drawn_again() {
    let (sender, _receiver) = relm4::channel::<AppInput>();
    let grid = gtk::Grid::new();
    let mut drawn = None;
    render_month(&grid, &month_showing("Standup"), &sender, &mut drawn);
    let cell = grid.child_at(0, 0).expect("the month drew its day");

    render_month(&grid, &month_showing("Standup"), &sender, &mut drawn);
    assert_eq!(
        grid.child_at(0, 0).as_ref(),
        Some(&cell),
        "a render that changed nothing replaced the day a press may be landing on"
    );

    render_month(&grid, &month_showing("Standup, moved"), &sender, &mut drawn);
    assert_ne!(
        grid.child_at(0, 0).as_ref(),
        Some(&cell),
        "the month went on showing an event that had changed"
    );
}
