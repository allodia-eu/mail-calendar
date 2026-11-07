//! One account's page in Settings → Accounts: how it travels, what it is used for, the accounts
//! it relies on, how it signs in, its own servers and password, its mail settings while it is
//! used for mail, and its removal, last.

use adw::prelude::*;
use mailcal_bindings::{
    AccountEntry, AccountLinkPicker, AccountPage, AccountPageIntent, AccountUseSwitch,
};

use super::{PageContext, account_mail, account_rows, account_servers, group, page_box};
use crate::{
    l10n,
    ui::{
        AppInput,
        account_settings::AccountsInput,
        controls,
        server_change::{self, Review, ReviewFrom},
    },
};

/// `entry`'s page: its uses, links and removal as the core decided them (`account`), the rest
/// from the entry.
pub(super) fn page(ctx: &PageContext, entry: &AccountEntry, account: &AccountPage) -> gtk::Box {
    let content = page_box(&entry.address);
    content.prepend(&back_button(&ctx.sender));
    let kind = gtk::Label::new(Some(&super::account_names::kind_line(entry)));
    kind.add_css_class("dim-label");
    kind.set_xalign(0.0);
    content.append(&kind);
    // What needs attention first, above what the person might change about it.
    if let Some(notices) = super::account_notices::group(ctx, &entry.id) {
        content.append(&notices);
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
    content.append(&uses_group(ctx, entry, &account.uses));
    if !account.links.is_empty() {
        content.append(&links_group(ctx, entry, &account.links));
    }
    if let Some(shared) = super::shared_mailboxes::section(ctx, entry) {
        content.append(&shared);
    }
    if account_rows::signs_in_at_provider(entry.kind) {
        content.append(&signin_group(ctx, entry));
    } else if let Some(section) = super::sign_in_section::group(ctx, entry) {
        content.append(&section);
    }
    if let Some(technology) = entry.mail_sync {
        content.append(&super::account_sync_technology::group(technology));
    }
    if let Some(endpoints) = &entry.endpoints {
        content.append(&account_servers::server_group(
            ctx,
            entry,
            endpoints,
            signin_expired(ctx, entry),
        ));
    }
    if let Some(mail) = mail_group(ctx, entry) {
        content.append(&mail);
    }
    content.append(&remove_group(ctx, entry, account.remove));
    content
}

/// Asks the core for a change on the page; it returns at once, and the page says it runs.
fn change(sender: &relm4::Sender<AppInput>, intent: AccountPageIntent) {
    sender.emit(AppInput::Accounts(AccountsInput::Change(intent)));
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

/// A switch per use the account's kind offers. Switching one off that deletes what the device
/// holds of it is confirmed first.
fn uses_group(
    ctx: &PageContext,
    entry: &AccountEntry,
    switches: &[AccountUseSwitch],
) -> adw::PreferencesGroup {
    let section = group(
        l10n::settings_account_uses_heading(),
        l10n::settings_account_uses_description(),
    );
    for switch in switches {
        let capability = switch.capability;
        let note = account_rows::switch_note(switch);
        let (parent, sender, account) = (ctx.window.clone(), ctx.sender.clone(), entry.id.clone());
        let (asks, confirms) = (switch.asks_provider, switch.confirms_off);
        let row = controls::switch_row(
            account_rows::use_name(capability),
            note.as_deref(),
            switch.on,
            switch.control,
            move |row, on| {
                let set = AccountPageIntent::SetUse {
                    account: account.clone(),
                    capability,
                    on,
                };
                if on && asks {
                    sender.emit(AppInput::Accounts(AccountsInput::SignInAgain {
                        account: account.clone(),
                        adding: vec![capability],
                    }));
                } else if on || !confirms {
                    change(&sender, set);
                } else {
                    let title = l10n::account_use_off_title(account_rows::use_name(capability));
                    let (sender, row) = (sender.clone(), row.downgrade());
                    controls::confirm(
                        &parent,
                        &title,
                        l10n::account_use_off_message(),
                        l10n::action_switch_off(),
                        move |yes| {
                            if yes {
                                change(&sender, set.clone());
                            } else if let Some(row) = row.upgrade() {
                                row.set_active(true);
                            }
                        },
                    );
                }
            },
        );
        section.add(&row);
        // Under the note that says why: a sign-in of its own for the server that needs one.
        if crate::ui::separate_sign_in_view::action_after(entry) == Some(capability)
            && let Some(action) = crate::ui::separate_sign_in_view::page_action(entry, &ctx.sender)
        {
            section.add(&action);
        }
        // Under the note that says it is paused: the question about the server found elsewhere.
        if switch.reviews_server_change
            && let Some(action) = review_row(ctx, entry)
        {
            section.add(&action);
        }
    }
    section
}

/// "Review", which asks whether to allow the server the calendar or contacts was found on.
fn review_row(ctx: &PageContext, entry: &AccountEntry) -> Option<adw::ButtonRow> {
    let review = Review {
        address: entry.address.clone(),
        change: ctx.app.pending_server_change(entry.id.clone())?,
    };
    let row = adw::ButtonRow::new();
    row.set_use_markup(false);
    row.set_title(l10n::server_changed_review());
    let (parent, sender, account) = (ctx.window.clone(), ctx.sender.clone(), entry.id.clone());
    row.connect_activated(move |_| {
        server_change::ask(&parent, &account, &review, ReviewFrom::Settings, &sender);
    });
    Some(row)
}

/// A picker per link slot the account can hold.
fn links_group(
    ctx: &PageContext,
    entry: &AccountEntry,
    pickers: &[AccountLinkPicker],
) -> adw::PreferencesGroup {
    let section = group(
        l10n::settings_account_links_heading(),
        l10n::settings_account_links_description(),
    );
    for picker in pickers {
        let names = account_rows::link_labels(picker);
        let labels = names.iter().map(String::as_str).collect::<Vec<_>>();
        let (sender, account, slot) = (ctx.sender.clone(), entry.id.clone(), picker.slot);
        let targets = picker
            .options
            .iter()
            .map(|option| option.id.clone())
            .collect::<Vec<_>>();
        let (row, dropdown) = controls::picker_row(
            account_rows::link_title(slot),
            &labels,
            picker.selected.map_or(0, |index| index + 1),
            picker.control,
            move |at| {
                let target = (at as usize)
                    .checked_sub(1)
                    .and_then(|index| targets.get(index).cloned());
                change(
                    &sender,
                    AccountPageIntent::SetLink {
                        account: account.clone(),
                        slot,
                        target,
                    },
                );
            },
        );
        if let (Some(index), Some(name)) = (picker.suggested, account_rows::suggested_name(picker))
        {
            suggest(&row, &dropdown, &name, index, picker.control.enabled);
        }
        section.add(&row);
    }
    section
}

/// Names the suggested account under the picker, with a button that picks it: the person confirms
/// a suggestion, it is never linked for them (`docs/accounts.md` rule 12).
fn suggest(
    row: &adw::ActionRow,
    dropdown: &gtk::DropDown,
    address: &str,
    index: u32,
    enabled: bool,
) {
    row.set_subtitle(&l10n::settings_account_link_suggested(address));
    let link = gtk::Button::with_label(l10n::action_link());
    link.set_valign(gtk::Align::Center);
    link.set_sensitive(enabled);
    let dropdown = dropdown.downgrade();
    let position = index + 1;
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
    let expired = signin_expired(ctx, entry);
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
    // An account whose servers are editable takes its new password in that section, so the mail
    // settings offer it only to one that has no such section.
    let expired = signin_expired(ctx, entry) && entry.endpoints.is_none();
    let provider = ctx.app.account_provider(entry.id.clone());
    Some(account_mail::mail_group(
        ctx,
        account,
        &super::account_names::entry_name(entry),
        &snapshot,
        expired,
        provider.as_ref(),
    ))
}

/// Whether the server refused the account's stored sign-in.
fn signin_expired(ctx: &PageContext, entry: &AccountEntry) -> bool {
    ctx.app
        .connectivity()
        .signin_expired_accounts
        .contains(&entry.id)
}

/// Removing the account, last, confirmed first.
fn remove_group(
    ctx: &PageContext,
    entry: &AccountEntry,
    control: mailcal_bindings::ControlState,
) -> adw::PreferencesGroup {
    let section = adw::PreferencesGroup::new();
    let row = adw::ActionRow::builder()
        .title(l10n::action_remove_account())
        .use_markup(false)
        .build();
    let (parent, sender, account) = (ctx.window.clone(), ctx.sender.clone(), entry.id.clone());
    let message = remove_message(entry);
    let remove = controls::button(l10n::action_remove(), control, true, move || {
        let (sender, account) = (sender.clone(), account.clone());
        controls::confirm(
            &parent,
            l10n::remove_account_title(),
            &message,
            l10n::action_remove(),
            move |yes| {
                if yes {
                    change(
                        &sender,
                        AccountPageIntent::Remove {
                            account: account.clone(),
                        },
                    );
                }
            },
        );
    });
    row.add_suffix(&remove);
    section.add(&row);
    section
}

/// What removing the account does, naming the accounts that lose their link to it.
fn remove_message(entry: &AccountEntry) -> String {
    let mut message = l10n::remove_account_message(&super::account_names::entry_name(entry));
    if !entry.linked_from.is_empty() {
        let accounts = entry
            .linked_from
            .iter()
            .map(super::account_names::linked_name)
            .collect::<Vec<_>>()
            .join(", ");
        message.push(' ');
        message.push_str(&l10n::remove_account_unlinks(&accounts));
    }
    message
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
            sign_in: mailcal_bindings::SignInMethod::Password,
            uses: Vec::new(),
            links: mailcal_bindings::AccountLinksView::default(),
            linked_from: Vec::new(),
            link_candidates: mailcal_bindings::LinkCandidates::default(),
            endpoints: None,
            separate_server: None,
            distinction: None,
            mail_sync: None,
            nextcloud: false,
        };
        assert_eq!(
            remove_message(&entry),
            l10n::remove_account_message("alice@cloud.example")
        );

        entry.linked_from = vec![LinkedAccount {
            id: "alice@example.org@imap.example.org".to_owned(),
            address: "alice@example.org".to_owned(),
            distinction: None,
        }];
        assert!(
            remove_message(&entry).ends_with(&l10n::remove_account_unlinks("alice@example.org"))
        );

        entry.linked_from[0].distinction = Some(mailcal_bindings::AccountDistinction::Kind {
            kind: AccountKind::Imap,
        });
        let named = l10n::settings_account_distinguished(
            "alice@example.org",
            l10n::setup_account_type_password(),
        );
        assert!(remove_message(&entry).ends_with(&l10n::remove_account_unlinks(&named)));

        entry.distinction = Some(mailcal_bindings::AccountDistinction::Server {
            host: "dav.cloud.example".to_owned(),
        });
        let removed =
            l10n::settings_account_distinguished("alice@cloud.example", "dav.cloud.example");
        assert!(remove_message(&entry).starts_with(&l10n::remove_account_message(&removed)));
    }
}
