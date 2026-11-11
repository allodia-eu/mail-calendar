//! The controls a page draws from the core's `ControlState` (`docs/waiting.md`): a switch row, a
//! button, a text row, a picker row and a confirmation. Each is held while it cannot be used and
//! shows a spinner beside itself while work it started runs, so every page waits the same way.

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

/// An action row titled `title` in plain text: a translated title may hold a bare ampersand.
fn plain_row(title: &str) -> adw::ActionRow {
    let row = super::mailbox::plain_text_row();
    row.set_title(title);
    row
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

/// A row that opens something, a question or a form, and starts no work of its own: it is only
/// held, while a change runs on the page, and the spinner is the control's that started it.
pub(super) fn button_row(
    title: &str,
    enabled: bool,
    activated: impl Fn() + 'static,
) -> adw::ButtonRow {
    let row = adw::ButtonRow::new();
    row.set_use_markup(false);
    row.set_title(title);
    row.set_sensitive(enabled);
    row.connect_activated(move |_| activated());
    row
}

/// A row that opens another page and starts no work: held while a change runs on the page.
pub(super) fn nav_row(
    title: &str,
    subtitle: &str,
    enabled: bool,
    activated: impl Fn() + 'static,
) -> adw::ActionRow {
    let row = plain_row(title);
    row.set_subtitle(subtitle);
    row.set_activatable(true);
    row.add_suffix(&gtk::Image::from_icon_name(super::icons::NEXT));
    row.set_sensitive(enabled);
    row.connect_activated(move |_| activated());
    row
}

/// A row that starts work: a button row while it can be pressed, and a row with a spinner before
/// its title while the work it started runs.
pub(super) fn work_row(
    title: &str,
    control: ControlState,
    activated: impl Fn() + 'static,
) -> gtk::Widget {
    match spinner(control) {
        Some(spinner) => {
            let row = plain_row(title);
            row.add_prefix(&spinner);
            row.set_sensitive(control.enabled);
            row.upcast()
        }
        None => button_row(title, control.enabled, activated).upcast(),
    }
}

/// Marks a field that sends what was typed itself, so a redraw of the page does not wait on it
/// once it has.
const SENDS_ITSELF: &str = "sends-itself";
/// Marks such a field while it holds text it has not sent.
const UNSENT: &str = "unsent";

/// Holds a form's `field` while `control` is held, without a spinner: the spinner is its send
/// button's.
pub(super) fn hold_field(field: &impl IsA<gtk::Widget>, control: ControlState) {
    field.set_sensitive(control.enabled);
}

/// Marks `form`, whose fields are sent together by a button, as holding edits it has not sent
/// (`unsent`) or not. A redraw waits while it does, wherever the caret is: the fields are the
/// person's until the button sends them.
pub(super) fn form_unsent(form: &impl IsA<gtk::Widget>, unsent: bool) {
    form.add_css_class(SENDS_ITSELF);
    if unsent {
        form.add_css_class(UNSENT);
    } else {
        form.remove_css_class(UNSENT);
    }
}

/// Whether anything under `root` holds edits it has not sent.
pub(super) fn holds_unsent(root: &gtk::Widget) -> bool {
    root.has_css_class(UNSENT)
        || std::iter::successors(root.first_child(), gtk::Widget::next_sibling)
            .any(|child| holds_unsent(&child))
}

/// A form's button that sends its fields, in a row whose title says what the fields still leave
/// out. Held while `control` is, or while the fields leave something out ([`FormSend::ready`]);
/// a spinner beside it while the work it started runs.
pub(super) struct FormSend {
    row: adw::ActionRow,
    button: gtk::Button,
    control: ControlState,
}

impl FormSend {
    pub(super) fn new(label: &str, control: ControlState, pressed: impl Fn() + 'static) -> Self {
        let row = plain_row("");
        let button = button(label, control, false, pressed);
        button.add_css_class("suggested-action");
        row.add_suffix(&button);
        Self {
            row,
            button,
            control,
        }
    }

    pub(super) fn row(&self) -> &adw::ActionRow {
        &self.row
    }

    /// Sends the form as its button would, when the button may be pressed: for a field whose
    /// Return sends it.
    pub(super) fn press(&self) {
        if self.button.is_sensitive() {
            self.button.emit_clicked();
        }
    }

    /// The fields as they stand: `missing` says what they leave out, `None` when they can be
    /// sent.
    pub(super) fn ready(&self, missing: Option<&str>) {
        self.row.set_title(missing.unwrap_or_default());
        self.button
            .set_sensitive(self.control.enabled && missing.is_none());
    }
}

/// Whether `widget` sits in a field that sends what was typed itself, and if so, whether that
/// field holds text it has not sent yet.
pub(super) fn unsent(widget: &gtk::Widget) -> Option<bool> {
    std::iter::successors(Some(widget.clone()), gtk::Widget::parent)
        .find(|widget| widget.has_css_class(SENDS_ITSELF))
        .map(|field| field.has_css_class(UNSENT))
}

/// A row with a text field, which tells `committed` what was typed on Return and when it loses
/// focus, and only when it differs from what it last told, starting from `text`: never on every
/// keystroke, which would store a half-typed value and redraw the page under the cursor, and never
/// twice for one edit, as Return and then the focus leaving would.
pub(super) fn text_row(
    title: &str,
    subtitle: &str,
    text: &str,
    control: ControlState,
    committed: impl Fn(String) + 'static,
) -> adw::ActionRow {
    let row = plain_row(title);
    row.set_subtitle(subtitle);
    if let Some(spinner) = spinner(control) {
        row.add_suffix(&spinner);
    }
    let entry = gtk::Entry::new();
    entry.set_placeholder_text(Some(title));
    entry.set_text(text);
    entry.set_valign(gtk::Align::Center);
    entry.set_width_chars(24);
    entry.set_sensitive(control.enabled);
    entry.add_css_class(SENDS_ITSELF);
    let sent = std::rc::Rc::new(std::cell::RefCell::new(text.to_owned()));
    let unsent_now = sent.clone();
    entry.connect_changed(move |entry| {
        if entry.text() == unsent_now.borrow().as_str() {
            entry.remove_css_class(UNSENT);
        } else {
            entry.add_css_class(UNSENT);
        }
    });
    let commit = std::rc::Rc::new(move |entry: &gtk::Entry| {
        let typed = entry.text().to_string();
        if typed != *sent.borrow() {
            sent.replace(typed.clone());
            entry.remove_css_class(UNSENT);
            committed(typed);
        }
    });
    let on_activate = commit.clone();
    entry.connect_activate(move |entry| on_activate(entry));
    let focus = gtk::EventControllerFocus::new();
    let left = entry.downgrade();
    focus.connect_leave(move |_| {
        if let Some(entry) = left.upgrade() {
            commit(&entry);
        }
    });
    entry.add_controller(focus);
    row.add_suffix(&entry);
    row
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
    let row = plain_row(title);
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
