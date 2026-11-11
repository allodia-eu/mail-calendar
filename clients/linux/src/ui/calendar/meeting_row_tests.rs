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
        through: None,
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
    let pick = append_meeting_row(&form, &editor, &picker).pick;
    let section = form.last_child().expect("the row is appended");
    let on_save = section
        .last_child()
        .and_downcast::<gtk::Label>()
        .expect("the row ends in the line about the save");
    let dropdown = on_save
        .prev_sibling()
        .and_downcast::<gtk::DropDown>()
        .expect("the picker is above that line");

    assert!(section.is_visible());
    assert_eq!(
        entries(&dropdown),
        ["None", "Microsoft Teams", "Jitsi Meet"]
    );
    assert_eq!(dropdown.selected(), 0, "a new event starts with no meeting");
    assert_eq!(*pick.borrow(), MeetingSelection::None);
    assert!(
        !on_save.is_visible(),
        "no meeting, nothing to add at the save"
    );

    dropdown.set_selected(1);
    assert_eq!(
        *pick.borrow(),
        MeetingSelection::Choice("server:teams".to_owned())
    );
    assert!(on_save.is_visible(), "a chosen meeting is made at the save");

    picker.set_selected(1);
    assert_eq!(entries(&dropdown), ["None", "Jitsi Meet"]);
    assert_eq!(
        *pick.borrow(),
        MeetingSelection::None,
        "a choice the new calendar lacks goes back to None"
    );
    assert!(
        !on_save.is_visible(),
        "a choice the new calendar lacks is no longer announced"
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

pub(crate) fn a_new_events_row_takes_the_default_while_somebody_is_invited_until_a_pick() {
    let form = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let work = CalendarChoice {
        can_invite: true,
        default_meeting: Some("jitsi".to_owned()),
        ..calendar(
            "work",
            vec![
                option("server:teams", "Microsoft Teams"),
                option("jitsi", "Jitsi Meet"),
            ],
        )
    };
    let editor = EventEditor::create(vec![work], String::new());
    let picker = gtk::DropDown::from_strings(&["work"]);
    let row = append_meeting_row(&form, &editor, &picker);
    let dropdown = form
        .last_child()
        .and_then(|section| section.last_child())
        .and_then(|on_save| on_save.prev_sibling())
        .and_downcast::<gtk::DropDown>()
        .expect("the picker is above the line about the save");
    assert_eq!(dropdown.selected(), 0, "nobody is invited yet");

    (row.invited)(true);
    assert_eq!(
        dropdown.selected(),
        2,
        "the first invitee brings the default"
    );
    assert_eq!(
        *row.pick.borrow(),
        MeetingSelection::Choice("jitsi".to_owned())
    );
    assert!(!row.chosen.get(), "the row's own selection is not a pick");

    (row.invited)(false);
    assert_eq!(dropdown.selected(), 0, "the last invitee takes it away");
    assert_eq!(*row.pick.borrow(), MeetingSelection::None);

    (row.invited)(true);
    dropdown.set_selected(0);
    assert!(row.chosen.get());
    (row.invited)(false);
    (row.invited)(true);
    assert_eq!(dropdown.selected(), 0, "a pick of None stands");
    assert_eq!(*row.pick.borrow(), MeetingSelection::None);
}
