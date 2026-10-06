//! One account's page in Settings → Accounts: how it travels, what it is used for, the accounts
//! it relies on, signing in again at its provider, its mail settings while it is used for mail,
//! and its removal, last.

use adw::prelude::*;
use mailcal_bindings::{AccountCapability, AccountEntry};

use super::{PageContext, account_mail, account_rows, dialog_box, group, page_box};
use crate::{
    l10n,
    ui::{AppInput, account_settings::AccountsInput},
};

pub(super) fn page(ctx: &PageContext, entry: &AccountEntry) -> gtk::Box {
    let content = page_box(&entry.address);
    content.prepend(&back_button(&ctx.sender));
    let kind = gtk::Label::new(Some(account_rows::kind_label(entry.kind)));
    kind.add_css_class("dim-label");
    kind.set_xalign(0.0);
    content.append(&kind);
    if let Some(notice) = &ctx.account_notice {
        let banner = adw::Banner::new(notice);
        banner.set_revealed(true);
        content.append(&banner);
    }
    // Whether this one travels: first, because it decides whether anything below it is anybody
    // else's business (`docs/settings.md`).
    if let Some(status) = ctx.allodia_accounts_synced.get(&entry.id).copied() {
        content.append(&super::account_sync_mode::synced_group(
            &ctx.sender,
            &entry.id,
            status,
        ));
    }
    content.append(&uses_group(ctx, entry));
    let pickers = account_rows::link_pickers(entry);
    if !pickers.is_empty() {
        content.append(&links_group(ctx, entry, pickers));
    }
    if account_rows::signs_in_at_provider(entry.kind) {
        content.append(&signin_group(ctx, entry));
    }
    if let Some(mail) = mail_group(ctx, entry) {
        content.append(&mail);
    }
    content.append(&remove_group(ctx, entry));
    content
}

/// Back to the accounts list.
fn back_button(sender: &relm4::Sender<AppInput>) -> gtk::Button {
    let back = gtk::Button::from_icon_name(crate::ui::icons::PREVIOUS);
    back.add_css_class("flat");
    back.set_halign(gtk::Align::Start);
    back.set_tooltip_text(Some(l10n::a11y_back()));
    back.update_property(&[gtk::accessible::Property::Label(l10n::a11y_back())]);
    let sender = sender.clone();
    back.connect_clicked(move |_| sender.emit(AppInput::Accounts(AccountsInput::Show(None))));
    back
}

/// A switch per use the account's kind offers. Switching one off is confirmed first, because it
/// deletes what the device holds of it.
fn uses_group(ctx: &PageContext, entry: &AccountEntry) -> adw::PreferencesGroup {
    let section = group(
        l10n::settings_account_uses_heading(),
        l10n::settings_account_uses_description(),
    );
    for use_ in &entry.uses {
        let capability = use_.capability;
        let switch = account_rows::use_switch(entry, capability);
        let row = adw::SwitchRow::new();
        row.set_use_markup(false);
        row.set_title(account_rows::use_name(capability));
        if let Some(note) = switch.note {
            row.set_subtitle(note);
        }
        row.set_active(switch.active);
        row.set_sensitive(switch.sensitive);
        let parent = ctx.window.clone();
        let sender = ctx.sender.clone();
        let account = entry.id.clone();
        row.connect_active_notify(move |row| {
            let on = row.is_active();
            if on {
                sender.emit(AppInput::Accounts(AccountsInput::SetUse {
                    account: account.clone(),
                    capability,
                    on,
                }));
            } else {
                confirm_off(&parent, row, account.clone(), capability, sender.clone());
            }
        });
        section.add(&row);
    }
    section
}

/// Asks before switching a use off; cancelling puts the switch back.
fn confirm_off(
    parent: &gtk::Window,
    row: &adw::SwitchRow,
    account: String,
    capability: AccountCapability,
    sender: relm4::Sender<AppInput>,
) {
    let title = l10n::account_use_off_title(account_rows::use_name(capability));
    let (dialog, _) = crate::ui::modal::new(parent, &title, 440, None);
    let content = dialog_box();
    let message = gtk::Label::new(Some(l10n::account_use_off_message()));
    message.set_wrap(true);
    message.set_xalign(0.0);
    content.append(&message);
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    actions.set_halign(gtk::Align::End);
    let cancel = gtk::Button::with_label(l10n::action_cancel());
    actions.append(&cancel);
    let off = gtk::Button::with_label(l10n::action_switch_off());
    off.add_css_class("destructive-action");
    actions.append(&off);
    content.append(&actions);
    dialog.set_child(Some(&content));

    let confirmed = std::rc::Rc::new(std::cell::Cell::new(false));
    let done = confirmed.clone();
    let window = dialog.clone();
    off.connect_clicked(move |_| {
        done.set(true);
        sender.emit(AppInput::Accounts(AccountsInput::SetUse {
            account: account.clone(),
            capability,
            on: false,
        }));
        window.close();
    });
    let window = dialog.clone();
    cancel.connect_clicked(move |_| window.close());
    // However the dialog goes away without the destructive button, the switch goes back on.
    let row = row.downgrade();
    dialog.connect_close_request(move |_| {
        if !confirmed.get()
            && let Some(row) = row.upgrade()
        {
            row.set_active(true);
        }
        gtk::glib::Propagation::Proceed
    });
    dialog.present();
}

/// A picker per link slot the account can hold.
fn links_group(
    ctx: &PageContext,
    entry: &AccountEntry,
    pickers: Vec<account_rows::LinkPicker>,
) -> adw::PreferencesGroup {
    let section = group(
        l10n::settings_account_links_heading(),
        l10n::settings_account_links_description(),
    );
    for picker in pickers {
        let labels = std::iter::once(l10n::settings_account_link_none())
            .chain(picker.options.iter().map(|option| option.address.as_str()))
            .collect::<Vec<_>>();
        let selected = picker
            .selected
            .and_then(|index| u32::try_from(index + 1).ok())
            .unwrap_or(0);
        let (row, dropdown) = super::choice(picker.title, &labels, selected);
        if let Some(index) = picker.suggested {
            suggest(&row, &dropdown, &picker.options[index].address, index);
        }
        let sender = ctx.sender.clone();
        let account = entry.id.clone();
        let slot = picker.slot;
        let targets = picker
            .options
            .iter()
            .map(|option| option.id.clone())
            .collect::<Vec<_>>();
        dropdown.connect_selected_notify(move |dropdown| {
            let target = (dropdown.selected() as usize)
                .checked_sub(1)
                .and_then(|index| targets.get(index).cloned());
            sender.emit(AppInput::Accounts(AccountsInput::SetLink {
                account: account.clone(),
                slot,
                target,
            }));
        });
        section.add(&row);
    }
    section
}

/// Names the suggested account under the picker, with a button that picks it: the person confirms
/// a suggestion, it is never linked for them (`docs/accounts.md` rule 12).
fn suggest(row: &adw::ActionRow, dropdown: &gtk::DropDown, address: &str, index: usize) {
    row.set_subtitle(&l10n::settings_account_link_suggested(address));
    let link = gtk::Button::with_label(l10n::action_link());
    link.set_valign(gtk::Align::Center);
    let dropdown = dropdown.downgrade();
    let position = u32::try_from(index + 1).unwrap_or(0);
    link.connect_clicked(move |_| {
        if let Some(dropdown) = dropdown.upgrade() {
            dropdown.set_selected(position);
        }
    });
    row.add_suffix(&link);
}

/// "Sign in again", for an account that signs in at its provider's page: the remedy for an
/// expired sign-in and for a permission the provider withheld.
fn signin_group(ctx: &PageContext, entry: &AccountEntry) -> adw::PreferencesGroup {
    let expired = ctx
        .app
        .connectivity()
        .signin_expired_accounts
        .contains(&entry.id);
    let section = group(
        l10n::settings_account_signin_heading(),
        &account_rows::signin_description(entry, expired),
    );
    let row = adw::ButtonRow::builder()
        .title(l10n::signin_expired_action())
        .use_markup(false)
        .build();
    let sender = ctx.sender.clone();
    let account = entry.id.clone();
    row.connect_activated(move |_| {
        sender.emit(AppInput::Accounts(AccountsInput::SignInAgain {
            account: account.clone(),
            adding: Vec::new(),
        }));
    });
    section.add(&row);
    section
}

/// The mail settings, while the account is used for mail and the core lists its mailbox.
fn mail_group(ctx: &PageContext, entry: &AccountEntry) -> Option<adw::PreferencesGroup> {
    let snapshot = ctx.app.sync_settings();
    let account = snapshot
        .accounts
        .iter()
        .find(|account| account.account_id == entry.id)?;
    let expired = ctx
        .app
        .connectivity()
        .signin_expired_accounts
        .contains(&entry.id);
    let provider = ctx.app.account_provider(entry.id.clone());
    Some(account_mail::mail_group(
        ctx,
        account,
        &snapshot,
        expired,
        provider.as_ref(),
    ))
}

fn remove_group(ctx: &PageContext, entry: &AccountEntry) -> adw::PreferencesGroup {
    let section = adw::PreferencesGroup::new();
    let row = adw::ActionRow::builder()
        .title(l10n::action_remove_account())
        .use_markup(false)
        .build();
    let remove = gtk::Button::with_label(l10n::action_remove());
    remove.add_css_class("destructive-action");
    remove.set_valign(gtk::Align::Center);
    let parent = ctx.window.clone();
    let sender = ctx.sender.clone();
    let id = entry.id.clone();
    let message = remove_message(entry);
    remove.connect_clicked(move |_| {
        confirm_remove(&parent, &message, id.clone(), sender.clone());
    });
    row.add_suffix(&remove);
    section.add(&row);
    section
}

/// What removing the account does, naming the accounts that lose their link to it.
fn remove_message(entry: &AccountEntry) -> String {
    let mut message = l10n::remove_account_message(&entry.address);
    if !entry.linked_from.is_empty() {
        let accounts = entry
            .linked_from
            .iter()
            .map(|linked| linked.address.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        message.push(' ');
        message.push_str(&l10n::remove_account_unlinks(&accounts));
    }
    message
}

fn confirm_remove(
    parent: &gtk::Window,
    message: &str,
    account_id: String,
    sender: relm4::Sender<AppInput>,
) {
    let (dialog, _) = crate::ui::modal::new(parent, l10n::remove_account_title(), 440, None);
    let content = dialog_box();
    let label = gtk::Label::new(Some(message));
    label.set_wrap(true);
    label.set_xalign(0.0);
    content.append(&label);
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    actions.set_halign(gtk::Align::End);
    let cancel = gtk::Button::with_label(l10n::action_cancel());
    let window = dialog.clone();
    cancel.connect_clicked(move |_| window.close());
    actions.append(&cancel);
    let remove = gtk::Button::with_label(l10n::action_remove());
    remove.add_css_class("destructive-action");
    let window = dialog.clone();
    remove.connect_clicked(move |_| {
        sender.emit(AppInput::RemoveAccount(account_id.clone()));
        window.close();
    });
    actions.append(&remove);
    content.append(&actions);
    dialog.set_child(Some(&content));
    dialog.present();
}

#[cfg(test)]
mod tests {
    use mailcal_bindings::{AccountEntry, AccountKind, LinkedAccount};

    use super::remove_message;
    use crate::l10n;

    #[test]
    fn removing_an_account_names_the_accounts_that_lose_their_link() {
        let mut entry = AccountEntry {
            id: "alice@dav:cloud.example".to_owned(),
            address: "alice@cloud.example".to_owned(),
            kind: AccountKind::Dav,
            uses: Vec::new(),
            links: mailcal_bindings::AccountLinksView::default(),
            linked_from: Vec::new(),
            link_candidates: mailcal_bindings::LinkCandidates::default(),
            endpoints: None,
        };
        assert_eq!(
            remove_message(&entry),
            l10n::remove_account_message("alice@cloud.example")
        );

        entry.linked_from = vec![LinkedAccount {
            id: "alice@example.org@imap.example.org".to_owned(),
            address: "alice@example.org".to_owned(),
        }];
        assert!(
            remove_message(&entry).ends_with(&l10n::remove_account_unlinks("alice@example.org"))
        );
    }
}
