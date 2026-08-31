//! What the destination switcher must do, and what it must never do.
//!
//! The GTK halves are called from the crate's single `gtk::init` test (see
//! [`crate::ui::mailbox::tests`]).

use adw::prelude::*;

use super::{DestinationBar, ICONS};
use crate::ui::{AppInput, PrimaryView, shell};

/// A named glyph the theme lacks draws the broken-image icon, and the bar keeps drawing as though
/// nothing happened; so the names are asserted rather than looked at once.
pub(crate) fn every_destination_icon_resolves_to_a_real_glyph() {
    let display = gtk::gdk::Display::default().expect("a display");
    let theme = gtk::IconTheme::for_display(&display);
    for icon in ICONS {
        assert!(
            theme.has_icon(icon),
            "the switcher must be able to draw {icon}"
        );
    }
    assert!(
        !theme.has_icon("mailcal-not-an-icon-symbolic"),
        "a theme that answers yes to everything would make the check above meaningless"
    );
}

/// Pressing a destination navigates; the model arriving at one by another route does not.
///
/// The second half is the one worth a test: the shell also reaches a surface without the bar being
/// touched; opening a message, clicking a folder; and the bar then lights the button to match.
/// A `set_active` that dispatched would turn every such arrival into a second navigation, and on
/// contacts that is a redundant address-book sync on every refresh.
pub(crate) fn the_switcher_navigates_on_a_press_and_stays_quiet_when_the_model_moves() {
    let (sender, receiver) = relm4::channel::<AppInput>();
    let bar = DestinationBar::new(&sender);
    assert!(bar.mail.button.is_active(), "the shell opens on mail");

    bar.sync(PrimaryView::Contacts);
    assert!(bar.contacts.button.is_active());
    assert!(!bar.mail.button.is_active(), "one group, one lit button");
    bar.sync(PrimaryView::Calendar);
    assert!(bar.calendar.button.is_active());
    assert!(!bar.contacts.button.is_active());

    // Had either sync dispatched, this assertion would read its message instead of the press,
    // which is exactly how a silent extra navigation would show up.
    bar.mail.button.emit_clicked();
    assert!(matches!(receiver.recv_sync(), Some(AppInput::ShowMail)));
    bar.calendar.button.emit_clicked();
    assert!(matches!(receiver.recv_sync(), Some(AppInput::ShowCalendar)));
    bar.contacts.button.emit_clicked();
    assert!(matches!(receiver.recv_sync(), Some(AppInput::ShowContacts)));
    assert!(!bar.calendar.button.is_active());
}

/// The switcher is pinned under the accounts, not scrolled with them.
///
/// Asserted against the shell's own assembly rather than a copy of it: a rebuilt-in-the-test
/// arrangement would only prove that the test can build one. What must hold is that the bar is a
/// bottom bar of the pane; outside the scroller, so a fifty-folder account cannot carry it off
/// screen.
pub(crate) fn the_switcher_is_pinned_below_the_accounts_and_never_scrolls_with_them() {
    let (sender, _receiver) = relm4::channel::<AppInput>();
    let destinations = DestinationBar::new(&sender);
    let accounts = gtk::ScrolledWindow::new();
    accounts.set_child(Some(&gtk::ListBox::new()));
    let pane = shell::sidebar_pane(&sender, &accounts, &destinations);

    let bar = destinations.widget().clone().upcast::<gtk::Widget>();
    assert!(
        bar.ancestor(gtk::ScrolledWindow::static_type()).is_none(),
        "the switcher must sit outside the scroller, or it scrolls away with the folders"
    );
    assert_eq!(
        bar.ancestor(adw::ToolbarView::static_type())
            .and_downcast::<adw::ToolbarView>()
            .as_ref(),
        Some(&pane),
        "the switcher belongs to the folder pane's own toolbar view"
    );
    assert_eq!(
        pane.content().as_ref(),
        Some(accounts.upcast_ref::<gtk::Widget>()),
        "the accounts are the pane's content, so they take the height the bars leave"
    );
}
