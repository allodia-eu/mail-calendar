//! The shell's destination switcher; mail, calendar, contacts and settings; pinned to the
//! bottom of the folder pane.
//!
//! **Pinned, not scrolled with the tree above it.** An account with a few dozen folders fills the
//! pane, and a switcher that scrolls away with them leaves no way to reach the calendar or
//! contacts without scrolling back to find it. The accounts take whatever height is left over,
//! which is what an [`adw::ToolbarView`] bottom bar gives for free.
//!
//! The three surfaces are **one toggle group**, so "the calendar and contacts at once" is
//! unrepresentable rather than merely unreached; the same move the other clients made when
//! contacts arrived. Settings is an ordinary button beside them: it opens a window, it does not
//! take the pane's surface.

use adw::prelude::*;
use gtk::accessible::Property as AccessibleProperty;

use super::AppInput;
use crate::l10n;

/// The shell's top-level surfaces. Exactly one is on screen at a time; an enum rather than a
/// flag per screen, so "the calendar and contacts at once" is a state that cannot be written down.
///
/// It lives here, with the switcher that is the only thing that moves between them, and `ui`
/// re-exports it for the model that holds the current one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PrimaryView {
    Mail,
    Calendar,
    Contacts,
}

/// The switcher's glyphs, in the order they sit on the bar.
///
/// Themed rather than bundled: Adwaita; the theme the GNOME runtime provides, and so the one the
/// Flatpak runs against; ships all four, and a name a theme lacks draws the broken-image icon
/// while the bar carries on as though nothing happened, so the widget tests assert each resolves.
/// Contacts takes the address book from the same mimetype family as the calendar beside it.
const ICONS: [&str; 4] = [
    "mail-unread-symbolic",
    "x-office-calendar-symbolic",
    CONTACTS_ICON,
    "preferences-system-symbolic",
];

/// The address book, also drawn on the contacts surface's own empty states.
pub(super) const CONTACTS_ICON: &str = "x-office-address-book-symbolic";

/// One destination button and the handler that navigates from it.
///
/// The handler id is kept so [`DestinationBar::sync`] can light a button **without** it: the
/// model reaches a surface by other routes too; opening a message, clicking a folder; and a
/// silent `set_active` would otherwise dispatch the navigation that has already happened.
struct Destination {
    button: gtk::ToggleButton,
    handler: gtk::glib::SignalHandlerId,
}

impl Destination {
    fn new(
        icon: &str,
        label: &'static str,
        sender: &relm4::Sender<AppInput>,
        message: fn() -> AppInput,
    ) -> Self {
        let button = gtk::ToggleButton::new();
        button.set_icon_name(icon);
        button.add_css_class("flat");
        button.set_tooltip_text(Some(label));
        button.update_property(&[AccessibleProperty::Label(label)]);
        let input = sender.clone();
        let handler = button.connect_toggled(move |button| {
            // Only the button that just lit up is a navigation. A grouped toggle also fires for
            // the one going dark, and that is not a request to go anywhere.
            if button.is_active() {
                input.emit(message());
            }
        });
        Self { button, handler }
    }

    fn light(&self) {
        if self.button.is_active() {
            return;
        }
        self.button.block_signal(&self.handler);
        self.button.set_active(true);
        self.button.unblock_signal(&self.handler);
    }
}

/// The switcher itself.
pub(crate) struct DestinationBar {
    root: gtk::Box,
    mail: Destination,
    calendar: Destination,
    contacts: Destination,
}

impl DestinationBar {
    pub(crate) fn new(sender: &relm4::Sender<AppInput>) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        root.add_css_class("toolbar");
        // Homogeneous so the four sit on an even rhythm at every pane width; the pane's floor is
        // 200px, which is too little for a label beside each glyph, so the name is the tooltip
        // and the accessible label rather than text that would truncate to nothing.
        root.set_homogeneous(true);

        let [mail_icon, calendar_icon, contacts_icon, settings_icon] = ICONS;
        let mail = Destination::new(mail_icon, l10n::nav_mail(), sender, || AppInput::ShowMail);
        let calendar = Destination::new(calendar_icon, l10n::nav_calendar(), sender, || {
            AppInput::ShowCalendar
        });
        let contacts = Destination::new(contacts_icon, l10n::nav_contacts(), sender, || {
            AppInput::ShowContacts
        });
        calendar.button.set_group(Some(&mail.button));
        contacts.button.set_group(Some(&mail.button));
        // Through `light`, not `set_active`: the shell opens on mail, and lighting the button to
        // say so is not the user asking to go there.
        mail.light();

        let settings = gtk::Button::from_icon_name(settings_icon);
        settings.add_css_class("flat");
        settings.set_tooltip_text(Some(l10n::settings_title()));
        settings.update_property(&[AccessibleProperty::Label(l10n::settings_title())]);
        let input = sender.clone();
        settings.connect_clicked(move |_| input.emit(AppInput::OpenSettings));

        root.append(&mail.button);
        root.append(&calendar.button);
        root.append(&contacts.button);
        root.append(&settings);

        Self {
            root,
            mail,
            calendar,
            contacts,
        }
    }

    pub(crate) fn widget(&self) -> &gtk::Box {
        &self.root
    }

    /// Lights the surface the model says is showing.
    pub(crate) fn sync(&self, primary: PrimaryView) {
        match primary {
            PrimaryView::Mail => self.mail.light(),
            PrimaryView::Calendar => self.calendar.light(),
            PrimaryView::Contacts => self.contacts.light(),
        }
    }
}

#[cfg(test)]
#[path = "destinations_tests.rs"]
pub(super) mod tests;
