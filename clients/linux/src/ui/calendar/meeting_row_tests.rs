//! The Video meeting row as drawn: hidden while the calendar offers nothing, redrawn from the
//! calendar the picker moves to, and holding the pick a Save reads.
//!
//! Called from the crate's single `gtk::init` test.

use adw::prelude::*;
use mailcal_bindings::MeetingOption;

use super::append_meeting_row;
use crate::ui::calendar::{
    editor::{CalendarChoice, EventEditor, tests::choice},
    editor_meeting::MeetingSelection,
};

fn calendar(id: &str, meeting_choices: Vec<MeetingOption>) -> CalendarChoice {
    CalendarChoice {
        id: id.to_owned(),
        is_default: id == "work",
        meeting_choices,
        ..choice()
    }
}

fn option(id: &str, name: &str) -> MeetingOption {
    MeetingOption {
        id: id.to_owned(),
        name: name.to_owned(),
    }
}

fn entries(dropdown: &gtk::DropDown) -> Vec<String> {
    let model = dropdown
        .model()
        .and_downcast::<gtk::StringList>()
        .expect("the row draws a string list");
    (0..model.n_items())
        .filter_map(|index| model.string(index))
        .map(String::from)
        .collect()
}

pub(crate) fn the_meeting_row_follows_the_calendar_and_holds_the_pick() {
    let form = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let editor = EventEditor::create(
        vec![
            calendar(
                "work",
                vec![
                    option("server:teams", "Microsoft Teams"),
                    option("jitsi", "Jitsi Meet"),
                ],
            ),
            calendar("home", vec![option("jitsi", "Jitsi Meet")]),
            calendar("feeds", Vec::new()),
        ],
        String::new(),
    );
    let picker = gtk::DropDown::from_strings(&["work", "home", "feeds"]);
    picker.set_selected(0);
    let pick = append_meeting_row(&form, &editor, &picker);
    let section = form.last_child().expect("the row is appended");
    let dropdown = section
        .last_child()
        .and_downcast::<gtk::DropDown>()
        .expect("the row ends in its picker");

    assert!(section.is_visible());
    assert_eq!(
        entries(&dropdown),
        ["None", "Microsoft Teams", "Jitsi Meet"]
    );
    assert_eq!(dropdown.selected(), 0, "a new event starts with no meeting");
    assert_eq!(*pick.borrow(), MeetingSelection::None);

    dropdown.set_selected(1);
    assert_eq!(
        *pick.borrow(),
        MeetingSelection::Choice("server:teams".to_owned())
    );

    picker.set_selected(1);
    assert_eq!(entries(&dropdown), ["None", "Jitsi Meet"]);
    assert_eq!(
        *pick.borrow(),
        MeetingSelection::None,
        "a choice the new calendar lacks goes back to None"
    );

    dropdown.set_selected(1);
    picker.set_selected(0);
    assert_eq!(
        dropdown.selected(),
        2,
        "a choice both calendars offer is kept"
    );
    assert_eq!(*pick.borrow(), MeetingSelection::Choice("jitsi".to_owned()));

    picker.set_selected(2);
    assert!(
        !section.is_visible(),
        "a calendar that offers no meeting draws no row"
    );
    assert_eq!(*pick.borrow(), MeetingSelection::None);
}
