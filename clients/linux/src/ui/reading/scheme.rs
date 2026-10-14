//! The page a message is drawn on, and the sun/moon button that changes it
//! (`../../../../docs/reading-dark-mode.md`).
//!
//! Dark exactly when the app is painted dark and the reader has left dark messages on. Both inputs
//! change while a message is open: the preference arrives with the core's Settings signal, and the
//! style manager's own flip is turned into one here, so either re-renders every reader.

use std::sync::{Arc, Once};

use adw::prelude::*;
use gtk::accessible::Property as AccessibleProperty;
use mailcal_bindings::{MailcalApp, MessageScheme, Surface};

use crate::{
    l10n,
    ui::{AppInput, icons},
};

/// The scheme the open message is drawn in.
pub(super) fn current(app: Option<&Arc<MailcalApp>>) -> MessageScheme {
    let dark_messages = app.is_none_or(|app| app.display_settings().dark_message_background);
    of(adw::StyleManager::default().is_dark(), dark_messages)
}

/// The rule itself, apart from where its inputs come from.
pub(super) fn of(app_dark: bool, dark_messages: bool) -> MessageScheme {
    if app_dark && dark_messages {
        MessageScheme::Dark
    } else {
        MessageScheme::Light
    }
}

/// The button: shown only in a dark app, its glyph and name saying what pressing it does.
pub(super) fn update_toggle(button: &gtk::Button, scheme: MessageScheme) {
    button.set_visible(adw::StyleManager::default().is_dark());
    let (icon, name) = match scheme {
        MessageScheme::Dark => (
            icons::LIGHT_BACKGROUND,
            l10n::reading_show_light_background(),
        ),
        MessageScheme::Light => (icons::DARK_BACKGROUND, l10n::reading_show_dark_background()),
    };
    button.set_icon_name(icon);
    button.set_tooltip_text(Some(name));
    button.update_property(&[AccessibleProperty::Label(name)]);
}

/// Re-renders every reader when the style manager flips, as an appearance set to follow the
/// desktop does while a message is open. Once per process: the shell renders every reader.
pub(super) fn follow_the_style_manager(sender: &relm4::Sender<AppInput>) {
    static CONNECTED: Once = Once::new();
    CONNECTED.call_once(|| {
        let sender = sender.clone();
        adw::StyleManager::default().connect_dark_notify(move |_| {
            sender.emit(AppInput::SurfaceChanged(Surface::Settings));
        });
    });
}

#[cfg(test)]
mod tests {
    use mailcal_bindings::MessageScheme;

    use super::of;

    #[test]
    fn a_message_is_dark_only_in_a_dark_app_with_dark_messages_on() {
        assert_eq!(of(true, true), MessageScheme::Dark);
        assert_eq!(of(true, false), MessageScheme::Light);
        assert_eq!(of(false, true), MessageScheme::Light);
        assert_eq!(of(false, false), MessageScheme::Light);
    }
}
