//! The per-account choice of how it is shared with the person's other devices.
//!
//! Split from [`super::accounts`] to keep that file inside the 500-line limit; it is one control
//! with one rule, and the rule is in its doc comment.

use adw::prelude::*;
use mailcal_bindings::AllodiaAccountSyncMode;

use crate::{
    l10n,
    ui::{AppInput, mailbox::plain_text_row},
};

/// How one account is shared with the person's other devices, as three linked buttons.
///
/// A single choice rather than a switch and a button: the two questions underneath; is this
/// account on my other devices, and does this device exchange changes about it; are not
/// independent in any way somebody can act on, and splitting them produced a screen where turning
/// the switch off changed nothing the person could see. Its Apple, Android and Windows twins use
/// each platform's own equivalent of this control.
///
/// A **linked box** of toggle buttons rather than `AdwToggleGroup`, which needs libadwaita 1.7 and
/// the shipped runtime is 1.5. `linked` is the GNOME idiom the newer widget replaces, and it draws
/// the same thing.
///
/// The description carries the selected position's meaning only: three at once is a paragraph
/// nobody reads, and the one that matters is the one in force.
pub(super) fn synced_group(
    sender: &relm4::Sender<AppInput>,
    account_id: &str,
    mode: AllodiaAccountSyncMode,
) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title(gtk::glib::markup_escape_text(l10n::settings_account_sync_heading()).as_str())
        .description(gtk::glib::markup_escape_text(sync_mode_hint(mode)).as_str())
        .build();

    let row = plain_text_row();
    row.set_title(l10n::settings_account_sync_heading());
    let choices = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    choices.add_css_class("linked");
    choices.set_valign(gtk::Align::Center);

    let mut first: Option<gtk::ToggleButton> = None;
    for (option, label) in [
        (AllodiaAccountSyncMode::On, l10n::settings_account_sync_on()),
        (
            AllodiaAccountSyncMode::Paused,
            l10n::settings_account_sync_paused(),
        ),
        (
            AllodiaAccountSyncMode::Off,
            l10n::settings_account_sync_off(),
        ),
    ] {
        let button = gtk::ToggleButton::with_label(label);
        // One group, so picking one releases the others; and so the keyboard treats the three as
        // a single choice rather than three unrelated switches.
        match &first {
            Some(anchor) => button.set_group(Some(anchor)),
            None => first = Some(button.clone()),
        }
        button.set_active(mode == option);
        let sender = sender.clone();
        let account_id = account_id.to_owned();
        button.connect_toggled(move |button| {
            // Releasing one raises another, so only the press that *selects* is acted on; and a
            // press on the position already in force is not a change at all.
            if button.is_active() && mode != option {
                sender.emit(AppInput::SetAllodiaAccountSyncMode(
                    account_id.clone(),
                    option,
                ));
            }
        });
        choices.append(&button);
    }
    row.add_suffix(&choices);
    group.add(&row);
    group
}

/// What the selected position means, in one line.
fn sync_mode_hint(mode: AllodiaAccountSyncMode) -> &'static str {
    match mode {
        AllodiaAccountSyncMode::On => l10n::settings_account_sync_on_hint(),
        AllodiaAccountSyncMode::Paused => l10n::settings_account_sync_paused_hint(),
        AllodiaAccountSyncMode::Off => l10n::settings_account_sync_off_hint(),
    }
}

#[cfg(test)]
#[path = "account_sync_mode_tests.rs"]
pub(crate) mod tests;
