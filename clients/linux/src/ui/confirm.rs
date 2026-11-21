//! The question asked before a change that deletes something, drawn by every page through
//! [`super::controls::confirm`].

use adw::prelude::*;

use crate::l10n;

/// Asks before a change that deletes something: `action` does it, Cancel does not. `answered` is
/// told which, however the question goes away.
pub(crate) fn confirm(
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
