//! An account's Sign-in section, drawn from the core's record (`docs/accounts.md` rule 17): the
//! line saying how it signs in or what is wrong, the ways in through a browser, and the field a
//! password is typed into. Which of them it offers is the core's; this words and draws them.

use adw::prelude::*;
use mailcal_bindings::{
    AccountEntry, AccountPageIntent, AccountPasswordField, AccountSecretKind, AccountSignIn,
    AccountSignInLine, AccountSignInRoute, BrowserSignIn, BrowserSignInStage, TypedPassword,
};

use super::{PageContext, account_page::change};
use crate::{
    l10n,
    ui::{AppInput, account_settings::AccountsInput, controls},
};

/// The line under the heading, for an account read as `name`.
pub(super) fn line(line: AccountSignInLine, name: &str) -> String {
    match line {
        AccountSignInLine::Password => l10n::settings_account_signs_in_password().to_owned(),
        AccountSignInLine::Provider => l10n::settings_account_signs_in_provider().to_owned(),
        AccountSignInLine::Nextcloud => l10n::settings_account_signs_in_nextcloud().to_owned(),
        AccountSignInLine::AtProvider => l10n::settings_account_signin_description().to_owned(),
        AccountSignInLine::NeedsPermission => l10n::settings_account_needs_permission().to_owned(),
        AccountSignInLine::Refused => l10n::signin_expired_prompt(name),
    }
}

/// The words on a route's button.
pub(super) fn label(route: &AccountSignInRoute) -> String {
    match route {
        AccountSignInRoute::SignInAgain => l10n::signin_expired_action().to_owned(),
        AccountSignInRoute::ToProvider {
            label: Some(provider),
        } => l10n::settings_account_switch_to_provider_named(provider),
        AccountSignInRoute::ToProvider { label: None } => {
            l10n::settings_account_switch_to_provider().to_owned()
        }
        AccountSignInRoute::ToNextcloud => {
            l10n::settings_account_switch_to_provider_named("Nextcloud")
        }
    }
}

/// `entry`'s section, when the core gives it one.
pub(super) fn group(
    ctx: &PageContext,
    entry: &AccountEntry,
    section: &AccountSignIn,
) -> adw::PreferencesGroup {
    draw(
        section,
        &entry.id,
        &super::account_names::entry_name(entry),
        &ctx.sender,
    )
}

/// Draws `section` for `account`, read as `name`. Every route is the account's one browser
/// sign-in, which the core runs: a route runs while the core starts or completes it, and stays
/// live while the browser holds it, since a fresh press replaces it, under a line saying where it
/// waits with the Cancel that drops it (`docs/provider-oauth.md` rule 13).
pub(super) fn draw(
    section: &AccountSignIn,
    account: &str,
    name: &str,
    sender: &relm4::Sender<AppInput>,
) -> adw::PreferencesGroup {
    let group = super::group(
        l10n::settings_account_signin_heading(),
        &line(section.line, name),
    );
    if let Some(row) = section
        .browser
        .as_ref()
        .and_then(|browser| waiting_row(browser, name, sender))
    {
        group.add(&row);
    }
    for route in &section.routes {
        let (input, account) = (sender.clone(), account.to_owned());
        group.add(&controls::work_row(
            &label(route),
            section.control,
            move || {
                input.emit(AppInput::Accounts(AccountsInput::SignIn {
                    account: account.clone(),
                    adding: Vec::new(),
                }));
            },
        ));
    }
    if let Some(field) = section.password {
        password_field(&group, field, account, sender);
    }
    if section.switches {
        // Not a row: a group places anything else beneath its list, which is where this belongs.
        let note = gtk::Label::new(Some(l10n::settings_account_switch_description()));
        note.set_wrap(true);
        note.set_xalign(0.0);
        note.set_margin_top(6);
        note.add_css_class("dim-label");
        note.add_css_class("caption");
        group.add(&note);
    }
    group
}

/// Where `browser` waits, with the Cancel that drops it; nothing while it does not wait on the
/// browser.
fn waiting_row(
    browser: &BrowserSignIn,
    name: &str,
    sender: &relm4::Sender<AppInput>,
) -> Option<adw::ActionRow> {
    let BrowserSignInStage::InBrowser { redirect_uri, .. } = &browser.stage else {
        return None;
    };
    let row = adw::ActionRow::new();
    row.set_use_markup(false);
    row.set_title(&match redirect_uri {
        Some(_) => l10n::settings_account_signin_waiting(name),
        None => l10n::nextcloud_sign_in_waiting().to_owned(),
    });
    let (input, account, attempt) = (sender.clone(), browser.account.clone(), browser.attempt);
    let cancel = controls::button(l10n::action_cancel(), browser.cancel, false, move || {
        change(
            &input,
            AccountPageIntent::CancelSignIn {
                account: account.clone(),
                attempt,
            },
        );
    });
    row.add_suffix(&cancel);
    Some(row)
}

/// The password field, with the button that sends it. One that switches from the provider's
/// sign-in waits behind "Use a password instead" until it runs; one that replaces a refused
/// password is drawn at once. It never shows the account's credential, and is emptied as it is
/// sent; while it holds text, a redraw waits.
fn password_field(
    group: &adw::PreferencesGroup,
    field: AccountPasswordField,
    account: &str,
    sender: &relm4::Sender<AppInput>,
) {
    let title = match field.kind {
        AccountSecretKind::Password => l10n::setup_field_password(),
        AccountSecretKind::PasswordOrToken => l10n::setup_jmap_secret_placeholder(),
    };
    let state = field.save;
    let entry = adw::PasswordEntryRow::new();
    entry.set_use_markup(false);
    entry.set_title(title);
    controls::hold_field(&entry, state);
    controls::form_unsent(&entry, false);
    let label = if field.replaces {
        l10n::action_save()
    } else {
        l10n::settings_account_switch_password_save()
    };
    let save = {
        let (entry, input, account) = (entry.clone(), sender.clone(), account.to_owned());
        controls::FormSend::new(label, state, move || {
            let text = entry.text().to_string();
            if text.is_empty() {
                return;
            }
            entry.set_text("");
            change(
                &input,
                AccountPageIntent::UsePassword {
                    account: account.clone(),
                    password: TypedPassword { text },
                },
            );
        })
    };
    save.ready(Some(""));
    let save = std::rc::Rc::new(save);
    {
        let save = save.clone();
        entry.connect_changed(move |entry| {
            let empty = entry.text().is_empty();
            controls::form_unsent(entry, !empty);
            save.ready(empty.then_some(""));
        });
    }
    {
        let save = save.clone();
        entry.connect_entry_activated(move |_| save.press());
    }
    let shown = field.replaces || field.save.running;
    entry.set_visible(shown);
    save.row().set_visible(shown);
    if !shown {
        let reveal = controls::button_row(
            l10n::settings_account_switch_to_password(),
            state.enabled,
            {
                let (entry, row) = (entry.clone(), save.row().clone());
                move || {
                    entry.set_visible(true);
                    row.set_visible(true);
                    entry.grab_focus();
                }
            },
        );
        {
            let reveal = reveal.clone();
            reveal.connect_activated(|reveal| reveal.set_visible(false));
        }
        group.add(&reveal);
    }
    group.add(&entry);
    group.add(save.row());
}

#[cfg(test)]
#[path = "sign_in_section_tests.rs"]
pub(crate) mod tests;
