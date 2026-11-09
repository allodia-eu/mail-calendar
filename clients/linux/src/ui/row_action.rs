//! Exposes a list row's primary operation as a native button assistive technology can invoke.

use adw::prelude::*;
use gtk::accessible::Property as AccessibleProperty;

use crate::ui::icons;

fn button(activate: impl Fn() + 'static) -> gtk::Button {
    let button = gtk::Button::from_icon_name(icons::NEXT);
    button.add_css_class("flat");
    button.set_valign(gtk::Align::Center);
    button.set_tooltip_text(Some(crate::l10n::action_open()));
    button.update_property(&[AccessibleProperty::Label(crate::l10n::action_open())]);
    button.connect_clicked(move |_| activate());
    button
}

pub(super) fn action_row(row: &adw::ActionRow, activate: impl Fn() + 'static) {
    let button = button(activate);
    row.add_suffix(&button);
    row.set_activatable_widget(Some(&button));
}

/// The same native action for a navigation row, carried by the row's own leading `icon` rather
/// than a trailing arrow, so the only chevron a row draws is one that opens or shuts something.
///
/// The button is named after the row and stays the row's activatable widget, so assistive
/// technology invokes it as before. It takes neither the pointer nor focus: a click and the
/// keyboard reach the row, which activates the button, so the icon draws no control of its own.
pub(super) fn icon_action_row(
    row: &adw::ActionRow,
    icon: &str,
    name: &str,
    activate: impl Fn() + 'static,
) {
    let button = gtk::Button::new();
    button.set_child(Some(&gtk::Image::from_icon_name(icon)));
    button.add_css_class("flat");
    button.set_valign(gtk::Align::Center);
    button.set_can_target(false);
    button.set_focusable(false);
    button.update_property(&[AccessibleProperty::Label(name)]);
    button.connect_clicked(move |_| activate());
    row.add_prefix(&button);
    row.set_activatable_widget(Some(&button));
}

pub(super) fn expander_row(row: &adw::ExpanderRow, activate: impl Fn() + 'static) {
    row.add_suffix(&button(activate));
}
