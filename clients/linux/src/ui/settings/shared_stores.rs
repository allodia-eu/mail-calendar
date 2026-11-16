//! The Shared mailboxes and calendars section of an account's page (`docs/shared-mailboxes.md`):
//! a switch per store the account's sign-in opens besides its own, saying what it holds, the line
//! under them and, on a server that lists none, a field for an address. Switching one off is
//! confirmed first, because it deletes what the device holds of it.

use adw::prelude::*;
use mailcal_bindings::{
    AccountEntry, AccountPageIntent, AccountSharedStore, AccountSharedStores, SharedStoreHolds,
    SharedStoresNote,
};

use super::{PageContext, account_page::change, group};
use crate::{l10n, ui::controls};

/// What a switch says beneath its title: that the server no longer lists it, or its label beside
/// what it holds.
pub(super) fn subtitle(store: &AccountSharedStore) -> String {
    if store.withdrawn {
        return l10n::settings_account_shared_withdrawn().to_owned();
    }
    let holds = holds(store.holds);
    match store.label.as_deref() {
        Some(label) => format!("{label} · {holds}"),
        None => holds.to_owned(),
    }
}

/// What a store holds, in words.
pub(super) fn holds(holds: SharedStoreHolds) -> &'static str {
    match holds {
        SharedStoreHolds::Mail => l10n::settings_account_shared_holds_mail(),
        SharedStoreHolds::Calendar => l10n::settings_account_shared_holds_calendar(),
        SharedStoreHolds::MailAndCalendar => l10n::settings_account_shared_holds_both(),
    }
}

/// The line under the switches.
pub(super) fn note(note: SharedStoresNote) -> &'static str {
    match note {
        SharedStoresNote::NoneShared => l10n::settings_account_shared_none(),
        SharedStoresNote::ByAddress => l10n::settings_account_shared_by_address(),
        SharedStoresNote::Unreachable => l10n::settings_account_shared_unreachable(),
        SharedStoresNote::NotConnected => l10n::settings_account_shared_not_connected(),
    }
}

/// The section, when the page has one.
pub(super) fn section(
    ctx: &PageContext,
    entry: &AccountEntry,
    shared: &AccountSharedStores,
) -> adw::PreferencesGroup {
    let section = group(
        l10n::settings_account_shared_heading(),
        l10n::settings_account_shared_description(),
    );
    for store in &shared.stores {
        section.add(&switch(ctx, entry, store));
    }
    if let Some(field) = &shared.address {
        let row = adw::EntryRow::new();
        row.set_title(l10n::settings_account_shared_address());
        row.set_input_purpose(gtk::InputPurpose::Email);
        row.set_text(&field.text);
        section.add(&row);
        let (sender, account, typed) = (ctx.sender.clone(), entry.id.clone(), row.downgrade());
        let drawn = field.text.clone();
        let form = section.clone();
        // Add sits in the field's own row, as it does beside the field on every other client: one
        // field has nothing a row of its own could say it still leaves out.
        let add = controls::button(l10n::action_add(), field.add, false, move || {
            let Some(row) = typed.upgrade() else {
                return;
            };
            controls::form_unsent(&form, false);
            change(
                &sender,
                AccountPageIntent::AddSharedStore {
                    account: account.clone(),
                    address: row.text().trim().to_owned(),
                },
            );
        });
        add.add_css_class("suggested-action");
        row.add_suffix(&add);
        let enabled = field.add.enabled;
        let follow = {
            let (add, form) = (add.clone(), section.clone());
            move |row: &adw::EntryRow| {
                let text = row.text();
                controls::form_unsent(&form, text != drawn);
                // Nothing to add yet: the button waits, and the field says what it is for.
                add.set_sensitive(enabled && !text.trim().is_empty());
            }
        };
        follow(&row);
        row.connect_changed(move |row| follow(row));
        row.connect_entry_activated(move |_| {
            if add.is_sensitive() {
                add.emit_clicked();
            }
        });
    }
    if let Some(said) = shared.note {
        let line = adw::ActionRow::new();
        line.set_use_markup(false);
        line.set_title(note(said));
        line.add_css_class("dim-label");
        section.add(&line);
    }
    section
}

/// One store's switch; switching it off asks first.
fn switch(ctx: &PageContext, entry: &AccountEntry, store: &AccountSharedStore) -> adw::SwitchRow {
    let (parent, sender, account) = (ctx.window.clone(), ctx.sender.clone(), entry.id.clone());
    let (handle, title) = (store.handle.clone(), store.title.clone());
    let said = subtitle(store);
    controls::switch_row(
        &store.title,
        Some(said.as_str()),
        store.on,
        store.control,
        move |row, on| {
            let set = AccountPageIntent::SetSharedStore {
                account: account.clone(),
                handle: handle.clone(),
                on,
            };
            if on {
                change(&sender, set);
                return;
            }
            let (sender, row) = (sender.clone(), row.downgrade());
            controls::confirm(
                &parent,
                &l10n::settings_account_shared_off_title(&title),
                l10n::settings_account_shared_off_message(),
                l10n::action_switch_off(),
                move |yes| {
                    if yes {
                        change(&sender, set.clone());
                    } else if let Some(row) = row.upgrade() {
                        row.set_active(true);
                    }
                },
            );
        },
    )
}

#[cfg(test)]
#[path = "shared_stores_tests.rs"]
mod tests;
