//! Exposes a list row's primary operation as a native button assistive technology can invoke,
//! carried by the row's own leading glyph (a folder's icon, a sender's avatar) rather than a
//! trailing arrow, so the only chevron a list draws is one that opens or shuts something.
//!
//! The button is named and stays where assistive technology finds it, but takes neither the
//! pointer nor focus: a click and the keyboard reach the row, which activates the button, so the
//! glyph draws no control of its own.

use std::sync::Once;

use adw::prelude::*;
use gtk::accessible::Property as AccessibleProperty;

/// The style class that takes a button's own frame off its glyph.
const GLYPH: &str = "mailcal-row-glyph";

fn install_styles() {
    static INSTALLED: Once = Once::new();
    if let Some(display) = gtk::gdk::Display::default() {
        INSTALLED.call_once(|| {
            let provider = gtk::CssProvider::new();
            provider.load_from_string(&format!(
                "button.{GLYPH} {{ min-width: 0; min-height: 0; padding: 0; }}"
            ));
            gtk::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        });
    }
}

fn glyph_button(
    glyph: &impl IsA<gtk::Widget>,
    name: &str,
    activate: impl Fn() + 'static,
) -> gtk::Button {
    install_styles();
    let button = gtk::Button::new();
    button.set_child(Some(glyph));
    button.add_css_class("flat");
    button.add_css_class(GLYPH);
    button.set_valign(gtk::Align::Center);
    button.set_can_target(false);
    button.set_focusable(false);
    button.update_property(&[AccessibleProperty::Label(name)]);
    button.connect_clicked(move |_| activate());
    button
}

/// The button a mail row's `glyph`, its avatar, carries: it opens the message or conversation.
/// The row places it, and a message row makes it its activatable widget.
pub(super) fn open_button(
    glyph: &impl IsA<gtk::Widget>,
    activate: impl Fn() + 'static,
) -> gtk::Button {
    let button = glyph_button(glyph, crate::l10n::action_open(), activate);
    // Its name is an accessible label, which nothing but assistive technology can read back.
    button.set_widget_name("row-open");
    button
}

/// A navigation row: its `icon` leads it, under the row's own `name`, and goes there.
pub(super) fn icon_action_row(
    row: &adw::ActionRow,
    icon: &str,
    name: &str,
    activate: impl Fn() + 'static,
) {
    let button = glyph_button(&gtk::Image::from_icon_name(icon), name, activate);
    row.add_prefix(&button);
    row.set_activatable_widget(Some(&button));
}
