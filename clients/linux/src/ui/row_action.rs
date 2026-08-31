//! Exposes a list row's primary operation as a native button assistive technology can invoke.

use adw::prelude::*;
use gtk::accessible::Property as AccessibleProperty;

fn button(activate: impl Fn() + 'static) -> gtk::Button {
    let button = gtk::Button::from_icon_name("go-next-symbolic");
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

pub(super) fn expander_row(row: &adw::ExpanderRow, activate: impl Fn() + 'static) {
    row.add_suffix(&button(activate));
}
