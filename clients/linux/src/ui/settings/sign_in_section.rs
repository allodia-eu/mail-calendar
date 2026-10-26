//! The Sign-in section of an IMAP or JMAP account's page: how it signs in now and, once its
//! server has answered, the switch to the other way in where the server takes both, or signing
//! in again where it signs in through its provider (`docs/accounts.md` rule 17). Microsoft and
//! Google keep their own section; an account for a calendar and contacts alone has none.

use adw::prelude::*;
use mailcal_bindings::{AccountEntry, AccountKind, AccountSignInOptions, SignInMethod};

use super::PageContext;
use crate::{
    l10n,
    ui::{
        AppInput,
        account_settings::AccountsInput,
        account_sign_in::{Password, SignInInput, Stage, Switch},
    },
};

/// One control the section offers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum SignInControl {
    /// Sign in through the provider again.
    SignInAgain,
    /// Switch to a password, typed into the field this reveals.
    ToPassword,
    /// Switch to signing in through the provider, named when the build knows its name.
    ToProvider(Option<String>),
}

/// What the section shows.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct SignInSection {
    /// How the account signs in now.
    pub(super) status: &'static str,
    /// Nothing until the server has answered, and nothing it does not take.
    pub(super) controls: Vec<SignInControl>,
}

impl SignInSection {
    /// Whether a switch is offered, which is when the line saying what it keeps belongs.
    pub(super) fn switches(&self) -> bool {
        self.controls
            .iter()
            .any(|control| *control != SignInControl::SignInAgain)
    }
}

/// The section for an account of `kind` that signs in with `sign_in`, given what its server
/// accepts once it has answered. `None` for a kind that has no such section.
pub(super) fn section(
    kind: AccountKind,
    sign_in: SignInMethod,
    options: Option<&AccountSignInOptions>,
) -> Option<SignInSection> {
    if !matches!(kind, AccountKind::Imap | AccountKind::Jmap) {
        return None;
    }
    let status = match sign_in {
        SignInMethod::Password => l10n::settings_account_signs_in_password(),
        SignInMethod::Provider => l10n::settings_account_signs_in_provider(),
    };
    let mut controls = Vec::new();
    if let Some(options) = options {
        match sign_in {
            SignInMethod::Provider => {
                if options.sign_in {
                    controls.push(SignInControl::SignInAgain);
                }
                if options.password {
                    controls.push(SignInControl::ToPassword);
                }
            }
            SignInMethod::Password if options.sign_in => {
                controls.push(SignInControl::ToProvider(options.provider_label.clone()));
            }
            SignInMethod::Password => {}
        }
    }
    Some(SignInSection { status, controls })
}

/// The words on a control's button.
pub(super) fn label(control: &SignInControl) -> String {
    match control {
        SignInControl::SignInAgain => l10n::signin_expired_action().to_owned(),
        SignInControl::ToPassword => l10n::settings_account_switch_to_password().to_owned(),
        SignInControl::ToProvider(Some(provider)) => {
            l10n::settings_account_switch_to_provider_named(provider)
        }
        SignInControl::ToProvider(None) => l10n::settings_account_switch_to_provider().to_owned(),
    }
}

/// The section for `entry`, when its kind has one.
pub(super) fn group(ctx: &PageContext, entry: &AccountEntry) -> Option<adw::PreferencesGroup> {
    let section = section(entry.kind, entry.sign_in, ctx.sign_in.options(&entry.id))?;
    Some(draw(
        &section,
        &entry.id,
        &super::account_names::entry_name(entry),
        ctx.sign_in.running(&entry.id),
        &ctx.sender,
    ))
}

/// Draws `section` for `account`, read as `name`. Every control waits while the core works on a
/// change. While the browser holds one they stay live, since a fresh press replaces it, and a
/// line above them says where it waits, with the Cancel that drops it
/// (`docs/provider-oauth.md` rule 13).
pub(super) fn draw(
    section: &SignInSection,
    account: &str,
    name: &str,
    running: Option<Stage>,
    sender: &relm4::Sender<AppInput>,
) -> adw::PreferencesGroup {
    let group = super::group(l10n::settings_account_signin_heading(), section.status);
    if running == Some(Stage::Browser) {
        group.add(&waiting_row(account, name, sender));
    }
    for control in &section.controls {
        let row = button_row(&label(control));
        row.set_sensitive(running != Some(Stage::Core));
        group.add(&row);
        let switch = match control {
            SignInControl::ToPassword => {
                password_field(&group, &row, account, sender);
                continue;
            }
            SignInControl::SignInAgain => Switch::Again,
            SignInControl::ToProvider(_) => Switch::ToProvider,
        };
        let (input, account) = (sender.clone(), account.to_owned());
        row.connect_activated(move |_| {
            input.emit(AppInput::Accounts(AccountsInput::SignIn(
                SignInInput::Browser {
                    account: account.clone(),
                    switch,
                },
            )));
        });
    }
    if section.switches() {
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

/// A row that acts, with its title as plain text: set before the title, since the builder
/// cannot promise the order (`docs/client-traps.md`).
fn button_row(title: &str) -> adw::ButtonRow {
    let row = adw::ButtonRow::new();
    row.set_use_markup(false);
    row.set_title(title);
    row
}

/// The password field "Use a password instead" reveals, with the row that sends it. It never
/// shows the account's current credential, and is emptied as it is sent.
fn password_field(
    group: &adw::PreferencesGroup,
    reveal: &adw::ButtonRow,
    account: &str,
    sender: &relm4::Sender<AppInput>,
) {
    let field = adw::PasswordEntryRow::new();
    field.set_use_markup(false);
    field.set_title(l10n::setup_field_password());
    field.set_visible(false);
    group.add(&field);
    let save = button_row(l10n::settings_account_switch_password_save());
    save.add_css_class("suggested-action");
    save.set_visible(false);
    group.add(&save);
    {
        let (field, save) = (field.clone(), save.clone());
        reveal.connect_activated(move |reveal| {
            reveal.set_visible(false);
            field.set_visible(true);
            save.set_visible(true);
            field.grab_focus();
        });
    }
    let send = {
        let (field, input, account) = (field.clone(), sender.clone(), account.to_owned());
        move || {
            let password = field.text().to_string();
            // Kept sensitive while empty: the arrow keys stop on an insensitive row.
            if password.is_empty() {
                return;
            }
            field.set_text("");
            input.emit(AppInput::Accounts(AccountsInput::SignIn(
                SignInInput::Password {
                    account: account.clone(),
                    password: Password(password),
                },
            )));
        }
    };
    let on_return = send.clone();
    field.connect_entry_activated(move |_| on_return());
    save.connect_activated(move |_| send());
}

#[cfg(test)]
#[path = "sign_in_section_tests.rs"]
pub(crate) mod tests;
