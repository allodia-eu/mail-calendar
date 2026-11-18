//! The control components, drawn: held and spinning while their work runs, and telling only
//! what the person did.

use std::{cell::Cell, rc::Rc};

use adw::prelude::*;
use mailcal_bindings::ControlState;

const RUNNING: ControlState = ControlState {
    enabled: false,
    running: true,
};
const FREE: ControlState = ControlState {
    enabled: true,
    running: false,
};

pub(crate) fn spins(widget: &impl IsA<gtk::Widget>) -> bool {
    let mut child = widget.as_ref().first_child();
    while let Some(widget) = child {
        if widget.is::<adw::Spinner>() || spins(&widget) {
            return true;
        }
        child = widget.next_sibling();
    }
    false
}

/// Each control is held and spins while its work runs, is free otherwise, and tells only what
/// the person did.
pub(crate) fn a_control_spins_while_its_work_runs_and_tells_only_what_the_person_did() {
    let told = Rc::new(Cell::new(None));
    let heard = told.clone();
    let row = super::switch_row("Calendar", None, true, RUNNING, move |_, on| {
        heard.set(Some(on));
    });
    assert!(!row.is_sensitive() && spins(&row) && row.is_active());
    assert_eq!(told.get(), None, "drawing it said nothing");
    row.set_active(false);
    assert_eq!(told.get(), Some(false));
    told.set(None);
    row.set_active(true);
    assert_eq!(told.get(), None, "putting it back said nothing");
    let row = super::switch_row("Calendar", Some("note"), false, FREE, move |_, _| {});
    assert!(row.is_sensitive() && !spins(&row));

    let button = super::button("Remove", RUNNING, true, || {});
    assert!(!button.is_sensitive() && spins(&button));
    assert!(button.has_css_class("destructive-action"));
    let button = super::icon_button(super::super::icons::REMOVE, "Remove", RUNNING, || {});
    assert!(!button.is_sensitive() && spins(&button));
    let button = super::icon_button(super::super::icons::REMOVE, "Remove", FREE, || {});
    assert!(button.is_sensitive() && !spins(&button));
    assert_eq!(
        button.icon_name().as_deref(),
        Some(super::super::icons::REMOVE)
    );

    let picked = Rc::new(Cell::new(None));
    let heard = picked.clone();
    let (row, dropdown) = super::picker_row("Calendar", &["None", "a"], 1, FREE, move |at| {
        heard.set(Some(at));
    });
    assert!(dropdown.is_sensitive() && !spins(&row));
    assert_eq!(picked.get(), None, "drawing it said nothing");
    dropdown.set_selected(0);
    assert_eq!(picked.get(), Some(0));

    let picked = Rc::new(Cell::new(None));
    let heard = picked.clone();
    let row = super::combo_row("New", &["None", "a", "b"], 2, FREE, move |at| {
        heard.set(Some(at));
    });
    assert!(row.is_sensitive() && !spins(&row) && !row.uses_markup());
    assert_eq!(row.selected(), 2);
    assert_eq!(picked.get(), None, "drawing it said nothing");
    row.set_selected(1);
    assert_eq!(picked.get(), Some(1));
    let row = super::combo_row("New", &["None"], 0, RUNNING, |_| {});
    assert!(!row.is_sensitive() && spins(&row));
}

/// A confirmation answers `false` however it goes away without its action.
pub(crate) fn a_confirmation_closed_without_its_action_answers_no() {
    let parent = gtk::Window::new();
    let answer = Rc::new(Cell::new(None));
    let heard = answer.clone();
    let dialog = super::confirm(&parent, "Remove?", "Gone.", "Remove", move |yes| {
        heard.set(Some(yes));
    });
    dialog.close();
    assert_eq!(answer.get(), Some(false));
}
