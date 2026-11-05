//! The editor's Video meeting row, drawn over the decisions in `editor_meeting.rs`.

use std::{cell::RefCell, rc::Rc};

use adw::prelude::*;
use gtk::accessible::Property as AccessibleProperty;

use super::{
    editor::EventEditor,
    editor_meeting::{MeetingRowOption, MeetingSelection},
};
use crate::l10n;

/// The pick a Save reads.
pub(super) type MeetingPick = Rc<RefCell<MeetingSelection>>;

/// Appends the row beneath the calendar picker it follows. It is drawn only while that calendar
/// offers a meeting or the event has one to take off, and is redrawn from the new calendar's
/// choices when the picker changes, a pick the new calendar lacks going back to None.
pub(super) fn append_meeting_row(
    form: &gtk::Box,
    editor: &EventEditor,
    calendar: &gtk::DropDown,
) -> MeetingPick {
    let pick: MeetingPick = Rc::new(RefCell::new(editor.opened_meeting()));
    let section = gtk::Box::new(gtk::Orientation::Vertical, 12);
    let heading = gtk::Label::new(Some(l10n::event_video_meeting()));
    heading.set_xalign(0.0);
    heading.add_css_class("heading");
    section.append(&heading);
    let choices = gtk::DropDown::from_strings(&[]);
    choices.update_property(&[AccessibleProperty::Label(l10n::event_video_meeting())]);
    section.append(&choices);
    form.append(&section);

    let options: Rc<RefCell<Vec<MeetingRowOption>>> = Rc::default();
    let redraw = {
        let editor = editor.clone();
        let pick = Rc::clone(&pick);
        let options = Rc::clone(&options);
        let choices = choices.clone();
        let section = section.clone();
        move |index: u32| {
            section.set_visible(editor.shows_meeting_row(index));
            let offered = editor.meeting_options(index);
            let kept = editor.effective_meeting(index, &pick.borrow());
            let selected = offered
                .iter()
                .position(|option| option.selection == kept)
                .unwrap_or(0);
            let labels = offered
                .iter()
                .map(MeetingRowOption::label)
                .collect::<Vec<_>>();
            let labels = labels.iter().map(String::as_str).collect::<Vec<_>>();
            // Replaced before the model, so the `selected` notification the new model raises
            // reads the new entries.
            *options.borrow_mut() = offered;
            choices.set_model(Some(&gtk::StringList::new(&labels)));
            choices.set_selected(u32::try_from(selected).unwrap_or(0));
            *pick.borrow_mut() = kept;
        }
    };
    {
        let pick = Rc::clone(&pick);
        let options = Rc::clone(&options);
        choices.connect_selected_notify(move |choices| {
            let chosen = usize::try_from(choices.selected())
                .ok()
                .and_then(|index| options.borrow().get(index).cloned());
            if let Some(option) = chosen {
                *pick.borrow_mut() = option.selection;
            }
        });
    }
    redraw(calendar.selected());
    calendar.connect_selected_notify(move |calendar| redraw(calendar.selected()));
    pick
}

#[cfg(test)]
#[path = "meeting_row_tests.rs"]
pub(crate) mod tests;
