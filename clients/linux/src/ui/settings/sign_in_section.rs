//! An account's Sign-in section, drawn from the core's record (`docs/accounts.md` rule 17): the
//! line saying how it signs in or what is wrong, the ways in through a browser, and the field a
//! password is typed into. Which of them it offers is the core's; this words and draws them.

use adw::prelude::*;
use mailcal_bindings::{
    AccountEntry, AccountKind, AccountPageIntent, AccountPasswordField, AccountSecretKind,
    AccountSignIn, AccountSignInLine, AccountSignInRoute, TypedPassword,
};

use super::{PageContext, account_page::change};
use crate::{
    l10n,
    ui::{
        AppInput,
        account_settings::AccountsInput,
        account_sign_in::{SignInInput, Stage, Switch},
        controls,
    },
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

/// What pressing `route` on an account of `kind` asks for. Microsoft and Google sign in again
/// through their own consent; every other account through its own sign-in.
fn pressed(kind: AccountKind, account: &str, route: &AccountSignInRoute) -> AppInput {
    let account = account.to_owned();
    if matches!(kind, AccountKind::Microsoft | AccountKind::Google) {
        return AppInput::Accounts(AccountsInput::SignInAgain {
            account,
            adding: Vec::new(),
        });
    }
    let switch = match route {
        AccountSignInRoute::SignInAgain => Switch::Again,
        AccountSignInRoute::ToProvider { .. } | AccountSignInRoute::ToNextcloud => {
            Switch::ToProvider
        }
    };
    AppInput::Accounts(AccountsInput::SignIn(SignInInput::Browser {
        account,
        switch,
    }))
}

/// `entry`'s section, when the core gives it one.
pub(super) fn group(
    ctx: &PageContext,
    entry: &AccountEntry,
    section: &AccountSignIn,
) -> adw::PreferencesGroup {
    draw(
        section,
        entry.kind,
        &entry.id,
        &super::account_names::entry_name(entry),
        ctx.sign_in.running(&entry.id),
        &ctx.sender,
    )
}

/// Draws `section` for `account`, of `kind` and read as `name`. A browser sign-in is still this
/// client's to run: while the core completes one every route waits, and while the browser holds
/// one they stay live, since a fresh press replaces it, under a line saying where it waits with
/// the Cancel that drops it (`docs/provider-oauth.md` rule 13).
pub(super) fn draw(
    section: &AccountSignIn,
    kind: AccountKind,
    account: &str,
    name: &str,
    running: Option<Stage>,
    sender: &relm4::Sender<AppInput>,
) -> adw::PreferencesGroup {
    let group = super::group(
        l10n::settings_account_signin_heading(),
        &line(section.line, name),
    );
    if running == Some(Stage::Browser) {
        group.add(&waiting_row(account, name, sender));
    }
    let enabled = section.control.enabled && running != Some(Stage::Core);
    for route in &section.routes {
        let (input, account, pressing) = (sender.clone(), account.to_owned(), route.clone());
        group.add(&controls::button_row(&label(route), enabled, move || {
            input.emit(pressed(kind, &account, &pressing));
        }));
    }
    if let Some(field) = section.password {
        password_field(&group, field, account, enabled, sender);
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

/// Where a sign-in waits, with the Cancel that drops it.
fn waiting_row(account: &str, name: &str, sender: &relm4::Sender<AppInput>) -> adw::ActionRow {
    let row = adw::ActionRow::new();
    row.set_use_markup(false);
    row.set_title(&l10n::settings_account_signin_waiting(name));
    let cancel = gtk::Button::with_label(l10n::action_cancel());
    cancel.set_valign(gtk::Align::Center);
    let (input, account) = (sender.clone(), account.to_owned());
    cancel.connect_clicked(move |_| {
        input.emit(AppInput::Accounts(AccountsInput::SignIn(
            SignInInput::Cancel {
                account: account.clone(),
            },
        )));
    });
    row.add_suffix(&cancel);
    row
}

/// The password field, with the button that sends it. One that switches from the provider's
/// sign-in waits behind "Use a password instead" until it runs; one that replaces a refused
/// password is drawn at once. It never shows the account's credential, and is emptied as it is
/// sent; while it holds text, a redraw waits.
fn password_field(
    group: &adw::PreferencesGroup,
    field: AccountPasswordField,
    account: &str,
    enabled: bool,
    sender: &relm4::Sender<AppInput>,
) {
    let title = match field.kind {
        AccountSecretKind::Password => l10n::setup_field_password(),
        AccountSecretKind::PasswordOrToken => l10n::setup_jmap_secret_placeholder(),
    };
    let entry = adw::PasswordEntryRow::new();
    entry.set_use_markup(false);
    entry.set_title(title);
    controls::hold_field(&entry, field.save);
    controls::form_unsent(&entry, false);
    let label = if field.replaces {
        l10n::action_save()
    } else {
        l10n::settings_account_switch_password_save()
    };
    let save = {
        let (entry, input, account) = (entry.clone(), sender.clone(), account.to_owned());
        controls::FormSend::new(label, field.save, move || {
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
        let reveal = controls::button_row(l10n::settings_account_switch_to_password(), enabled, {
            let (entry, row) = (entry.clone(), save.row().clone());
            move || {
                entry.set_visible(true);
                row.set_visible(true);
                entry.grab_focus();
            }
        });
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
