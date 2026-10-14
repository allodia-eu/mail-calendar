//! Settings → Reading's dark-background switch: the same preference as the reading pane's sun/moon
//! button (`docs/reading-dark-mode.md`, rule 2). A press of the button while this page is open
//! reaches the core and not this window, which is rebuilt only for Accounts, so the switch follows
//! the core itself.

use adw::prelude::*;
use gtk::glib::WeakRef;
use mailcal_bindings::MailcalApp;

use super::{PageContext, group};
use crate::l10n;

thread_local! {
    /// The switch on screen, if Reading is. One Settings window, built on the GTK thread.
    static SHOWN: WeakRef<adw::SwitchRow> = WeakRef::new();
}

/// The Message background group, its switch on the core's preference and remembered so a press
/// elsewhere moves it.
pub(super) fn section(ctx: &PageContext) -> adw::PreferencesGroup {
    let background = group(
        l10n::settings_dark_messages_heading(),
        l10n::settings_dark_messages_description(),
    );
    let app = ctx.app.clone();
    background.add(&switch(
        ctx.app.display_settings().dark_message_background,
        move |dark| app.set_dark_message_background(dark),
    ));
    background
}

fn switch(active: bool, on_change: impl Fn(bool) + 'static) -> adw::SwitchRow {
    let row = adw::SwitchRow::builder()
        .title(l10n::settings_dark_messages_toggle())
        .active(active)
        .use_markup(false)
        .build();
    row.connect_active_notify(move |row| on_change(row.is_active()));
    SHOWN.with(|shown| shown.set(Some(&row)));
    row
}

/// Moves the switch on screen, if there is one, to the core's preference.
pub(super) fn follow(app: &MailcalApp) {
    if SHOWN.with(WeakRef::upgrade).is_some() {
        follow_to(app.display_settings().dark_message_background);
    }
}

/// Only a switch that disagrees is moved: moving it calls back into the core.
fn follow_to(dark: bool) {
    if let Some(row) = SHOWN.with(WeakRef::upgrade)
        && row.is_active() != dark
    {
        row.set_active(dark);
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::{cell::Cell, rc::Rc};

    use super::{follow_to, switch};

    /// Run from the crate's one GTK test.
    pub(crate) fn the_switch_follows_a_press_made_elsewhere() {
        let changes = Rc::new(Cell::new(0));
        let counted = Rc::clone(&changes);
        let row = switch(true, move |_| counted.set(counted.get() + 1));
        follow_to(true);
        assert_eq!(changes.get(), 0, "a switch that agrees is left alone");
        follow_to(false);
        assert!(!row.is_active());
        assert_eq!(changes.get(), 1);
        drop(row);
        follow_to(true);
        assert_eq!(changes.get(), 1, "a closed page has no switch to move");
    }
}
