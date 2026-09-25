//! Every icon the Linux client draws, by name, and the bundle that provides the ones Adwaita lacks.
//!
//! A name no theme provides is not an error anywhere in GTK: the widget draws the broken-image
//! glyph and carries on. So every name lives here, where one test asks the theme for each of them,
//! and `cargo xtask check-icons` refuses a name written anywhere else. Which set each comes from
//! and why is `docs/icons.md`; that doc's Linux column is this file.
//!
//! Adwaita's names are themed. The `mailcal-…` names are compiled into the binary
//! (`icons/mailcal.gresource.xml`): icon-development-kit files and this app's own drawings, served
//! under a name no theme ships so they draw the same on every distribution.

use std::sync::Once;

/// Must equal the `prefix` in icons/mailcal.gresource.xml. It is a namespace inside the binary,
/// not an identity, so it does not follow the application id.
pub(super) const RESOURCE_PATH: &str = "/mailcal/icons";

/// Puts the bundled glyphs on the display's icon theme, once. Every surface that draws a
/// `mailcal-…` name calls this first; a second call is free.
pub(super) fn install(display: &gtk::gdk::Display) {
    static INSTALLED: Once = Once::new();
    INSTALLED.call_once(|| {
        gtk::gio::resources_register_include!("mailcal.gresource")
            .expect("the bundled icons are compiled into the binary");
        gtk::IconTheme::for_display(display).add_resource_path(RESOURCE_PATH);
    });
}

macro_rules! icons {
    ($($(#[$doc:meta])* $name:ident = $value:expr;)*) => {
        $($(#[$doc])* pub(crate) const $name: &str = $value;)*

        /// Every name above, for the test that each one draws.
        #[cfg(test)]
        pub(crate) const ALL: &[&str] = &[$($name),*];
    };
}

icons! {
    // Folders, by role (docs/folder-pane.md, rule 9).
    /// On All Inboxes and on every account's Inbox alike, because the unified row *is* those
    /// inboxes summed.
    INBOX = "mailcal-inbox-symbolic";
    DRAFTS = "document-edit-symbolic";
    SENT = "mail-send-symbolic";
    ARCHIVE = "mailcal-archive-symbolic";
    JUNK = "mail-mark-junk-symbolic";
    TRASH = "user-trash-symbolic";
    FOLDER = "folder-symbolic";
    /// A person rather than a mail glyph, so an account reads as a heading over its folders rather
    /// than as another folder among them.
    ACCOUNT = PERSON;

    // Disclosure: a tree row, and the composer's extra header fields.
    EXPANDED = "pan-down-symbolic";
    COLLAPSED = "pan-end-symbolic";
    HIDE_DETAILS = "pan-up-symbolic";
    SHOW_DETAILS = "pan-down-symbolic";

    // Acting on mail.
    NEW_MESSAGE = "mail-message-new-symbolic";
    REPLY = "mail-reply-sender-symbolic";
    REPLY_ALL = "mail-reply-all-symbolic";
    FORWARD = "mail-forward-symbolic";
    MARK_READ = "mail-read-symbolic";
    MARK_UNREAD = "mail-unread-symbolic";
    FLAG = "starred-symbolic";
    UNFLAG = "non-starred-symbolic";
    DELETE_PERMANENTLY = "edit-delete-symbolic";
    SELECT_ALL = "edit-select-all-symbolic";
    SYNC = "view-refresh-symbolic";
    MORE = "view-more-symbolic";

    // General controls.
    ADD = "list-add-symbolic";
    /// Taking one item out of something being edited: an attachment, a contact's field, an event.
    REMOVE = TRASH;
    CLOSE = "window-close-symbolic";
    PREVIOUS = "go-previous-symbolic";
    NEXT = "go-next-symbolic";

    // State a row or page reports.
    SENDING = SENT;
    WARNING = "dialog-warning-symbolic";
    WAITING = "document-open-recent-symbolic";
    /// A document going out. Not `mail-send`, which the pane gives Sent: an Outbox that looks like
    /// Sent is the one confusion that row exists to prevent.
    OUTBOX = "document-send-symbolic";
    ATTACHMENT = "mail-attachment-symbolic";
    /// A person with no photo, and the account a folder tree hangs from.
    PERSON = "avatar-default-symbolic";
    /// The count pane beside a selection: messages, not a folder.
    SELECTED_MAIL = "mail-unread-symbolic";

    // The destination switcher.
    MAIL = "mail-unread-symbolic";
    CALENDAR = "x-office-calendar-symbolic";
    /// From the same mimetype family as the calendar beside it; also the contacts surface's own
    /// empty states.
    CONTACTS = "x-office-address-book-symbolic";
    SETTINGS = "preferences-system-symbolic";

    // Why the mail list is empty.
    /// Apple draws its inbox tray here too: the folder holds nothing yet.
    EMPTY_FOLDER = INBOX;
    OUTSIDE_SYNC_DEPTH = "document-open-recent-symbolic";

    // Settings categories (docs/settings.md), one glyph each: a sidebar whose rows share an icon
    // loses the one thing its icons are for.
    SETTINGS_ALLODIA = ACCOUNT;
    SETTINGS_GENERAL = "mailcal-cogged-wheel-symbolic";
    SETTINGS_CALENDAR = CALENDAR;
    /// Not Adwaita's `mail-read`, which is drawn at half opacity to mean *already read*, and in a
    /// sidebar reads as a disabled row.
    SETTINGS_READING = "mailcal-mail-read-symbolic";
    SETTINGS_COMPOSING = DRAFTS;
    SETTINGS_SIGNATURES = "mailcal-signature-symbolic";
    SETTINGS_WRITING_STYLE = "format-text-rich-symbolic";
    SETTINGS_NOTIFICATIONS = "mailcal-bell-symbolic";
    SETTINGS_PRIVACY = "channel-secure-symbolic";
    SETTINGS_ACCOUNTS = INBOX;
    SETTINGS_ADVANCED = "mailcal-wrench-symbolic";
    SETTINGS_DIAGNOSTICS = "mailcal-stethoscope-symbolic";
    SETTINGS_ABOUT = "help-about-symbolic";

    // What a drafted reply leaves to do (docs/ai.md), one glyph per kind.
    TASK_FILL_IN = DRAFTS;
    TASK_ATTACH = ATTACHMENT;
    TASK_DO = "view-list-bullet-symbolic";
}

#[cfg(test)]
pub(crate) mod tests {
    use super::{ALL, RESOURCE_PATH, install};

    /// Every name the client draws resolves in Adwaita plus the bundle.
    ///
    /// `Image::from_icon_name` is content with a name nothing provides, so asking a widget for its
    /// icon name proves only that we asked. The theme has to be asked, which is also what fails
    /// when the GResource stops being compiled in or the resource path drifts.
    ///
    /// Adwaita by name, never the display's theme: that is whatever the desktop running the test
    /// chose, and Ubuntu's Yaru ships names Adwaita does not, so a name only Yaru has passes on an
    /// Ubuntu desktop and draws the broken-image glyph in the Flatpak.
    ///
    /// Called from the crate's single `gtk::init` test.
    pub(crate) fn every_icon_resolves_to_a_real_glyph() {
        let display = gtk::gdk::Display::default().expect("a display");
        install(&display);
        assert!(
            gtk::IconTheme::for_display(&display)
                .resource_path()
                .iter()
                .any(|path| path == RESOURCE_PATH),
            "the bundle must be on the display's icon theme"
        );

        let adwaita = gtk::IconTheme::new();
        adwaita.set_theme_name(Some("Adwaita"));
        adwaita.add_resource_path(RESOURCE_PATH);
        let missing = ALL
            .iter()
            .filter(|name| !adwaita.has_icon(name))
            .collect::<std::collections::BTreeSet<_>>();
        assert!(
            missing.is_empty(),
            "Adwaita and the bundle cannot draw {missing:?}"
        );
        assert!(
            !adwaita.has_icon("mailcal-not-an-icon-symbolic"),
            "a theme that answers yes to everything would make the check above meaningless"
        );
    }
}
