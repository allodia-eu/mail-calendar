//! The "your name" row on an account's page: what recipients see beside the address on the mail
//! this account sends (`docs/sending.md`), as the core's page record says it.

use adw::prelude::*;
use mailcal_bindings::{AccountPageIntent, AccountSenderName};

use crate::{
    l10n,
    ui::{AppInput, controls},
};

/// The name's own group, drawn directly under the sharing control: the one thing on the page a
/// recipient can see, so it comes before how much of the account this device keeps
/// (`docs/settings.md`).
pub(super) fn group(
    account: &str,
    name: &AccountSenderName,
    sender: &relm4::Sender<AppInput>,
) -> adw::PreferencesGroup {
    let section = adw::PreferencesGroup::new();
    if !name.editable {
        // The name is the organisation's. Show it, and say why there is no field, rather than a
        // dead entry that reads as a bug.
        let row = adw::ActionRow::builder()
            .title(l10n::settings_sender_name_heading())
            .subtitle(l10n::settings_sender_name_managed())
            .use_markup(false)
            .build();
        let value = gtk::Label::new(Some(&name.name));
        value.set_valign(gtk::Align::Center);
        row.add_suffix(&value);
        section.add(&row);
        return section;
    }
    let (sender, account) = (sender.clone(), account.to_owned());
    section.add(&controls::text_row(
        l10n::settings_sender_name_heading(),
        l10n::settings_sender_name_description(),
        &name.name,
        name.control,
        move |typed| {
            super::account_page::change(
                &sender,
                AccountPageIntent::SetSenderName {
                    account: account.clone(),
                    name: typed,
                },
            );
        },
    ));
    section
}

#[cfg(test)]
#[path = "sender_name_tests.rs"]
pub(crate) mod tests;
