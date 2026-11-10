//! The editor's Video meeting row, drawn over the decisions in `editor_meeting.rs`.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use adw::prelude::*;
use gtk::accessible::Property as AccessibleProperty;

use super::{
    editor::EventEditor,
    editor_meeting::{MeetingRowOption, MeetingSelection},
};
use crate::l10n;

/// What a Save reads from the row, and how the invitee list reaches it.
pub(super) struct MeetingRow {
    /// The meeting the row holds.
    pub(super) pick: Rc<RefCell<MeetingSelection>>,
    /// Whether the person picked in the row, None included: such a pick outranks the calendar's
    /// default (`docs/conferencing.md`, rule 10).
    pub(super) chosen: Rc<Cell<bool>>,
    /// Told whether anybody is invited each time the invitee list changes: a new event's row
    /// takes its calendar's default from the first invitee on and gives it up with the last.
    pub(super) invited: Rc<dyn Fn(bool)>,
}

/// Appends the row beneath the calendar picker it follows. It is drawn only while that calendar
/// offers a meeting or the event has one to take off, and is redrawn from the new calendar's
/// choices when the picker changes, a pick the new calendar lacks going back to None. Beneath it,
/// while it holds a choice the event does not have yet, a line says the link comes at the save.
pub(super) fn append_meeting_row(
    form: &gtk::Box,
    editor: &EventEditor,
    calendar: &gtk::DropDown,
) -> MeetingRow {
    let pick = Rc::new(RefCell::new(editor.opened_meeting()));
    let chosen = Rc::new(Cell::new(false));
    let invited = Rc::new(Cell::new(false));
    let section = gtk::Box::new(gtk::Orientation::Vertical, 12);
    let heading = gtk::Label::new(Some(l10n::event_video_meeting()));
    heading.set_xalign(0.0);
    heading.add_css_class("heading");
    section.append(&heading);
    let choices = gtk::DropDown::from_strings(&[]);
    choices.update_property(&[AccessibleProperty::Label(l10n::event_video_meeting())]);
    section.append(&choices);
    let on_save = gtk::Label::new(Some(l10n::event_meeting_added_on_save()));
    on_save.set_xalign(0.0);
    on_save.set_wrap(true);
    on_save.add_css_class("dim-label");
    on_save.add_css_class("caption");
    section.append(&on_save);
    form.append(&section);

    let options: Rc<RefCell<Vec<MeetingRowOption>>> = Rc::default();
    // Set while the row redraws itself, so the selection it makes is not read as the person's.
    let redrawing = Rc::new(Cell::new(false));
    let redraw: Rc<dyn Fn(u32)> = {
        let editor = editor.clone();
        let pick = Rc::clone(&pick);
        let chosen = Rc::clone(&chosen);
        let invited = Rc::clone(&invited);
        let options = Rc::clone(&options);
        let redrawing = Rc::clone(&redrawing);
        let choices = choices.clone();
        let section = section.clone();
        let on_save = on_save.clone();
        Rc::new(move |index: u32| {
            section.set_visible(editor.shows_meeting_row(index));
            let offered = editor.meeting_options(index);
            let kept = editor.held_meeting(index, &pick.borrow(), chosen.get(), invited.get());
            let selected = offered
                .iter()
                .position(|option| option.selection == kept)
                .unwrap_or(0);
            let labels = offered
                .iter()
                .map(MeetingRowOption::label)
                .collect::<Vec<_>>();
            let labels = labels.iter().map(String::as_str).collect::<Vec<_>>();
            *options.borrow_mut() = offered;
            redrawing.set(true);
            choices.set_model(Some(&gtk::StringList::new(&labels)));
            choices.set_selected(u32::try_from(selected).unwrap_or(0));
            redrawing.set(false);
            on_save.set_visible(editor.announces_meeting(index, &kept));
            *pick.borrow_mut() = kept;
        })
    };
    {
        let editor = editor.clone();
        let pick = Rc::clone(&pick);
        let chosen = Rc::clone(&chosen);
        let calendar = calendar.clone();
        choices.connect_selected_notify(move |choices| {
            if redrawing.get() {
                return;
            }
            let picked = usize::try_from(choices.selected())
                .ok()
                .and_then(|index| options.borrow().get(index).cloned());
            if let Some(option) = picked {
                on_save
                    .set_visible(editor.announces_meeting(calendar.selected(), &option.selection));
                *pick.borrow_mut() = option.selection;
                chosen.set(true);
            }
        });
    }
    redraw(calendar.selected());
    {
        let redraw = Rc::clone(&redraw);
        calendar.connect_selected_notify(move |calendar| redraw(calendar.selected()));
    }
    let calendar = calendar.clone();
    MeetingRow {
        pick,
        chosen,
        invited: Rc::new(move |anybody: bool| {
            if invited.replace(anybody) != anybody {
                redraw(calendar.selected());
            }
        }),
    }
}

#[cfg(test)]
#[path = "meeting_row_tests.rs"]
pub(crate) mod tests;
