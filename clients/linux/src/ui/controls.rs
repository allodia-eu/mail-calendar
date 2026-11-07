//! The controls a page draws from the core's `ControlState` (`docs/waiting.md`): a switch row, a
//! button, a picker row and a confirmation. Each is held while it cannot be used and shows a
//! spinner beside itself while work it started runs, so every page waits the same way.

use adw::prelude::*;
use mailcal_bindings::ControlState;

use crate::l10n;

/// A spinner beside a control whose work is running, or nothing.
fn spinner(control: ControlState) -> Option<adw::Spinner> {
    control.running.then(|| {
        let spinner = adw::Spinner::new();
        spinner.set_valign(gtk::Align::Center);
        spinner
    })
}

/// A switch row. `changed` is told when the person moves it from where it was drawn; putting it
/// back, as a cancelled confirmation does, tells nothing.
pub(super) fn switch_row(
    title: &str,
    subtitle: Option<&str>,
    on: bool,
    control: ControlState,
    changed: impl Fn(&adw::SwitchRow, bool) + 'static,
) -> adw::SwitchRow {
    let row = adw::SwitchRow::new();
    row.set_use_markup(false);
    row.set_title(title);
    if let Some(subtitle) = subtitle {
        row.set_subtitle(subtitle);
    }
    row.set_active(on);
    row.set_sensitive(control.enabled);
    if let Some(spinner) = spinner(control) {
        row.add_suffix(&spinner);
    }
    row.connect_active_notify(move |row| {
        if row.is_active() != on {
            changed(row, !on);
        }
    });
    row
}

/// A button. A destructive one is drawn as the platform draws one.
pub(super) fn button(
    label: &str,
    control: ControlState,
    destructive: bool,
    clicked: impl Fn() + 'static,
) -> gtk::Button {
    let button = gtk::Button::new();
    let text = gtk::Label::new(Some(label));
    if let Some(spinner) = spinner(control) {
        let content = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        content.append(&spinner);
        content.append(&text);
        button.set_child(Some(&content));
    } else {
        button.set_child(Some(&text));
    }
    button.update_property(&[gtk::accessible::Property::Label(label)]);
    if destructive {
        button.add_css_class("destructive-action");
    }
    button.set_valign(gtk::Align::Center);
    button.set_sensitive(control.enabled);
    button.connect_clicked(move |_| clicked());
    button
}

/// A row that picks one of `labels`. `picked` is told the position the person picked, and never
/// the one it was drawn with.
pub(super) fn picker_row(
    title: &str,
    labels: &[&str],
    selected: u32,
    control: ControlState,
    picked: impl Fn(u32) + 'static,
) -> (adw::ActionRow, gtk::DropDown) {
    let row = adw::ActionRow::builder()
        .use_markup(false)
        .title(title)
        .build();
    if let Some(spinner) = spinner(control) {
        row.add_suffix(&spinner);
    }
    let dropdown = gtk::DropDown::from_strings(labels);
    dropdown.set_selected(selected);
    dropdown.set_valign(gtk::Align::Center);
    dropdown.set_sensitive(control.enabled);
    dropdown.connect_selected_notify(move |dropdown| picked(dropdown.selected()));
    row.add_suffix(&dropdown);
    (row, dropdown)
}

/// Asks before a change that deletes something: `action` does it, Cancel does not. `answered` is
/// told which, however the question goes away.
pub(super) fn confirm(
    parent: &gtk::Window,
    title: &str,
    message: &str,
    action: &str,
    answered: impl Fn(bool) + 'static,
) -> gtk::Window {
    let (dialog, _) = super::modal::new(parent, title, 440, None);
    let content = gtk::Box::new(gtk::Orientation::Vertical, 18);
    content.set_margin_start(28);
    content.set_margin_end(28);
    content.set_margin_top(24);
    content.set_margin_bottom(28);
    let label = gtk::Label::new(Some(message));
    label.set_wrap(true);
    label.set_xalign(0.0);
    content.append(&label);
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    actions.set_halign(gtk::Align::End);
    let cancel = gtk::Button::with_label(l10n::action_cancel());
    actions.append(&cancel);
    let confirm = gtk::Button::with_label(action);
    confirm.add_css_class("destructive-action");
    actions.append(&confirm);
    content.append(&actions);
    dialog.set_child(Some(&content));

    let confirmed = std::rc::Rc::new(std::cell::Cell::new(false));
    let (done, window) = (confirmed.clone(), dialog.clone());
    confirm.connect_clicked(move |_| {
        done.set(true);
        window.close();
    });
    let window = dialog.clone();
    cancel.connect_clicked(move |_| window.close());
    dialog.connect_close_request(move |_| {
        answered(confirmed.get());
        gtk::glib::Propagation::Proceed
    });
    dialog.present();
    dialog
}

#[cfg(test)]
pub(crate) mod tests {
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

    fn spins(widget: &impl IsA<gtk::Widget>) -> bool {
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

        let picked = Rc::new(Cell::new(None));
        let heard = picked.clone();
        let (row, dropdown) = super::picker_row("Calendar", &["None", "a"], 1, FREE, move |at| {
            heard.set(Some(at));
        });
        assert!(dropdown.is_sensitive() && !spins(&row));
        assert_eq!(picked.get(), None, "drawing it said nothing");
        dropdown.set_selected(0);
        assert_eq!(picked.get(), Some(0));
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
}
