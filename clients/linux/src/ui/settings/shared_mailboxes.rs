//! The Shared mailboxes section of an account's page (`docs/shared-mailboxes.md`): a switch per
//! mailbox the account's sign-in opens besides its own. Switching one off is confirmed first,
//! because it deletes what the device holds of it.

use adw::prelude::*;
use mailcal_bindings::{
    AccountEntry, SharedMailboxEntry, SharedMailboxSupport, SharedMailboxesView,
};

use super::{PageContext, dialog_box, group};
use crate::{
    l10n,
    ui::{AppInput, account_settings::AccountsInput, account_shared::SharedInput},
};

/// One mailbox's row, as the rules decide it.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct SharedRow {
    pub(super) title: String,
    pub(super) subtitle: Option<String>,
    pub(super) active: bool,
}

/// The row for `mailbox`: named by its address where it has one, by the server's label beside it
/// when that says something else, and marked when the server no longer lists it.
pub(super) fn row(mailbox: &SharedMailboxEntry) -> SharedRow {
    let title = mailbox
        .address
        .clone()
        .unwrap_or_else(|| mailbox.name.clone());
    let subtitle = if mailbox.listed {
        (mailbox.name != title).then(|| mailbox.name.clone())
    } else {
        Some(l10n::settings_account_shared_withdrawn().to_owned())
    };
    SharedRow {
        title,
        subtitle,
        active: mailbox.selected,
    }
}

/// The line under the switches, when the answer needs one; `None` when every mailbox the server
/// listed is a switch above it.
pub(super) fn note(view: &SharedMailboxesView) -> Option<&'static str> {
    match view.support {
        SharedMailboxSupport::Listed if view.mailboxes.is_empty() => {
            Some(l10n::settings_account_shared_none())
        }
        SharedMailboxSupport::Listed | SharedMailboxSupport::Unsupported => None,
        SharedMailboxSupport::ByAddress => Some(l10n::settings_account_shared_by_address()),
        SharedMailboxSupport::NotConnected => Some(l10n::settings_account_shared_not_connected()),
        SharedMailboxSupport::Unreachable => Some(l10n::settings_account_shared_unreachable()),
    }
}

/// The section, once the server has answered, for an account whose sign-in can open a shared
/// mailbox; `None` otherwise, so an account that cannot has no section that says nothing.
pub(super) fn section(ctx: &PageContext, entry: &AccountEntry) -> Option<adw::PreferencesGroup> {
    let view = ctx.shared.view(&entry.id)?;
    if view.support == SharedMailboxSupport::Unsupported && view.mailboxes.is_empty() {
        return None;
    }
    let section = group(
        l10n::settings_account_shared_heading(),
        l10n::settings_account_shared_description(),
    );
    for mailbox in &view.mailboxes {
        let shown = row(mailbox);
        let switch = adw::SwitchRow::new();
        switch.set_use_markup(false);
        switch.set_title(&shown.title);
        if let Some(subtitle) = &shown.subtitle {
            switch.set_subtitle(subtitle);
        }
        switch.set_active(shown.active);
        let parent = ctx.window.clone();
        let sender = ctx.sender.clone();
        let account = entry.id.clone();
        let mailbox = mailbox.clone();
        switch.connect_active_notify(move |switch| {
            // A cancelled switch-off puts the switch back, which changes nothing.
            if switch.is_active() == mailbox.selected {
                return;
            }
            if switch.is_active() {
                sender.emit(set(&account, &mailbox, true));
            } else {
                confirm_off(
                    &parent,
                    switch,
                    set(&account, &mailbox, false),
                    sender.clone(),
                );
            }
        });
        section.add(&switch);
    }
    if view.support == SharedMailboxSupport::ByAddress {
        section.add(&address_row(ctx, entry));
    }
    if let Some(note) = note(view) {
        let line = adw::ActionRow::new();
        line.set_use_markup(false);
        line.set_title(note);
        line.add_css_class("dim-label");
        section.add(&line);
    }
    Some(section)
}

/// A field for the address of a mailbox shared with the account, for a server that lists none.
/// Committed with Enter or its apply button, never per keystroke: each commit asks the server.
fn address_row(ctx: &PageContext, entry: &AccountEntry) -> adw::EntryRow {
    let row = adw::EntryRow::new();
    row.set_title(l10n::settings_account_shared_address());
    row.set_input_purpose(gtk::InputPurpose::Email);
    row.set_show_apply_button(true);
    let sender = ctx.sender.clone();
    let account = entry.id.clone();
    row.connect_apply(move |row| {
        let address = row.text().trim().to_owned();
        if address.is_empty() {
            return;
        }
        sender.emit(AppInput::Accounts(AccountsInput::Shared(
            SharedInput::Add {
                account: account.clone(),
                address,
            },
        )));
    });
    row
}

fn set(account: &str, mailbox: &SharedMailboxEntry, on: bool) -> AppInput {
    AppInput::Accounts(AccountsInput::Shared(SharedInput::Set {
        account: account.to_owned(),
        mailbox: mailbox.clone(),
        on,
    }))
}

/// Asks before switching a shared mailbox off; however the dialog goes away without the
/// destructive button, the switch goes back on.
fn confirm_off(
    parent: &gtk::Window,
    switch: &adw::SwitchRow,
    off: AppInput,
    sender: relm4::Sender<AppInput>,
) {
    let title = l10n::settings_account_shared_off_title(&switch.title());
    let (dialog, _) = crate::ui::modal::new(parent, &title, 440, None);
    let content = dialog_box();
    let message = gtk::Label::new(Some(l10n::settings_account_shared_off_message()));
    message.set_wrap(true);
    message.set_xalign(0.0);
    content.append(&message);
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    actions.set_halign(gtk::Align::End);
    let cancel = gtk::Button::with_label(l10n::action_cancel());
    actions.append(&cancel);
    let confirm = gtk::Button::with_label(l10n::action_switch_off());
    confirm.add_css_class("destructive-action");
    actions.append(&confirm);
    content.append(&actions);
    dialog.set_child(Some(&content));

    let confirmed = std::rc::Rc::new(std::cell::Cell::new(false));
    let done = confirmed.clone();
    let window = dialog.clone();
    let off = std::cell::RefCell::new(Some(off));
    confirm.connect_clicked(move |_| {
        done.set(true);
        if let Some(off) = off.borrow_mut().take() {
            sender.emit(off);
        }
        window.close();
    });
    let window = dialog.clone();
    cancel.connect_clicked(move |_| window.close());
    let switch = switch.downgrade();
    dialog.connect_close_request(move |_| {
        if !confirmed.get()
            && let Some(switch) = switch.upgrade()
        {
            switch.set_active(true);
        }
        gtk::glib::Propagation::Proceed
    });
    dialog.present();
}

#[cfg(test)]
#[path = "shared_mailboxes_tests.rs"]
mod tests;
