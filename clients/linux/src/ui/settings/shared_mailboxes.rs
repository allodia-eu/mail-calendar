//! The Shared mailboxes section of an account's page (`docs/shared-mailboxes.md`), drawn from the
//! page's `shared`: a switch per mailbox the account's sign-in opens besides its own, the line
//! under them and, on a server that lists none, a field for an address. Switching one off is
//! confirmed first, because it deletes what the device holds of it.

use std::rc::Rc;

use adw::prelude::*;
use mailcal_bindings::{
    AccountEntry, AccountPageIntent, AccountSharedMailbox, AccountSharedMailboxes,
    SharedMailboxesNote,
};

use super::{PageContext, account_page::change, group};
use crate::{l10n, ui::controls};

/// What a switch says beneath its title: that the server no longer lists it, or its label.
pub(super) fn subtitle(mailbox: &AccountSharedMailbox) -> Option<&str> {
    if mailbox.withdrawn {
        Some(l10n::settings_account_shared_withdrawn())
    } else {
        mailbox.label.as_deref()
    }
}

/// The line under the switches.
pub(super) fn note(note: SharedMailboxesNote) -> &'static str {
    match note {
        SharedMailboxesNote::NoneShared => l10n::settings_account_shared_none(),
        SharedMailboxesNote::ByAddress => l10n::settings_account_shared_by_address(),
        SharedMailboxesNote::NotConnected => l10n::settings_account_shared_not_connected(),
        SharedMailboxesNote::Unreachable => l10n::settings_account_shared_unreachable(),
    }
}

/// The section, when the page has one.
pub(super) fn section(
    ctx: &PageContext,
    entry: &AccountEntry,
    shared: &AccountSharedMailboxes,
) -> adw::PreferencesGroup {
    let section = group(
        l10n::settings_account_shared_heading(),
        l10n::settings_account_shared_description(),
    );
    for mailbox in &shared.mailboxes {
        section.add(&switch(ctx, entry, mailbox));
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
        let add = Rc::new(controls::FormSend::new(
            l10n::action_add(),
            field.add,
            move || {
                let Some(row) = typed.upgrade() else {
                    return;
                };
                controls::form_unsent(&form, false);
                change(
                    &sender,
                    AccountPageIntent::AddSharedMailbox {
                        account: account.clone(),
                        address: row.text().trim().to_owned(),
                    },
                );
            },
        ));
        section.add(add.row());
        let follow = {
            let (add, form) = (Rc::clone(&add), section.clone());
            move |row: &adw::EntryRow| {
                let text = row.text();
                controls::form_unsent(&form, text != drawn);
                // Nothing to add yet: the button waits, and the field says what it is for.
                add.ready((text.trim().is_empty()).then_some(""));
            }
        };
        follow(&row);
        row.connect_changed(move |row| follow(row));
        let send = Rc::clone(&add);
        row.connect_entry_activated(move |_| send.press());
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

/// One mailbox's switch; switching it off asks first.
fn switch(
    ctx: &PageContext,
    entry: &AccountEntry,
    mailbox: &AccountSharedMailbox,
) -> adw::SwitchRow {
    let (parent, sender, account) = (ctx.window.clone(), ctx.sender.clone(), entry.id.clone());
    let (handle, title) = (mailbox.handle.clone(), mailbox.title.clone());
    controls::switch_row(
        &mailbox.title,
        subtitle(mailbox),
        mailbox.on,
        mailbox.control,
        move |row, on| {
            let set = AccountPageIntent::SetSharedMailbox {
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
#[path = "shared_mailboxes_tests.rs"]
mod tests;
