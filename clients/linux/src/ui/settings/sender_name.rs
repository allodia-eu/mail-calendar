//! The "your name" row on an account's settings card: what recipients see beside the address on
//! the mail this account sends (`docs/sending.md`).
//!
//! Its own file so `accounts.rs` stays under the line limit, and because this is the one control
//! on the card whose effect a stranger can see; everything else there is about how much of the
//! account this device keeps.

use adw::prelude::*;
use mailcal_bindings::AccountSyncRow;

use crate::{l10n, ui::AppInput};

/// Adds the name field, or the note that the name is not this person's to change.
///
/// The edit is sent on **activate** (return) and on losing focus, never on every keystroke: a
/// per-keystroke write would push a half-typed name to the provider and rebuild the snapshot
/// under the cursor.
pub(super) fn add_row(
    section: &adw::PreferencesGroup,
    account: &AccountSyncRow,
    sender: &relm4::Sender<AppInput>,
) {
    if !account.sender_name_editable {
        // The name is the organisation's. Show it, and say why there is no field, rather than a
        // dead entry that reads as a bug.
        let shown = if account.sender_name.is_empty() {
            account.email.clone()
        } else {
            account.sender_name.clone()
        };
        let row = adw::ActionRow::builder()
            .title(l10n::settings_sender_name_heading())
            .subtitle(l10n::settings_sender_name_managed())
            .use_markup(false)
            .build();
        let value = gtk::Label::new(Some(&shown));
        value.set_valign(gtk::Align::Center);
        row.add_suffix(&value);
        section.add(&row);
        return;
    }

    let row = adw::ActionRow::builder()
        .title(l10n::settings_sender_name_heading())
        .subtitle(l10n::settings_sender_name_description())
        .use_markup(false)
        .build();
    let entry = gtk::Entry::new();
    entry.set_placeholder_text(Some(l10n::settings_sender_name_heading()));
    entry.set_text(&account.sender_name);
    entry.set_valign(gtk::Align::Center);
    entry.set_width_chars(24);

    let commit = {
        let sender = sender.clone();
        let account_id = account.account_id.clone();
        let stored = account.sender_name.clone();
        move |entry: &gtk::Entry| {
            let typed = entry.text().to_string();
            // Nothing changed: tabbing through the card costs no write, and no snapshot
            // rebuild that would reseed the entry mid-edit.
            if typed == stored {
                return;
            }
            sender.emit(AppInput::SetAccountSenderName {
                account: account_id.clone(),
                name: typed,
            });
        }
    };
    let on_activate = commit.clone();
    entry.connect_activate(move |entry| on_activate(entry));
    let controller = gtk::EventControllerFocus::new();
    let entry_for_focus = entry.clone();
    controller.connect_leave(move |_| commit(&entry_for_focus));
    entry.add_controller(controller);

    row.add_suffix(&entry);
    section.add(&row);
}

#[cfg(test)]
#[path = "sender_name_tests.rs"]
pub(crate) mod tests;
