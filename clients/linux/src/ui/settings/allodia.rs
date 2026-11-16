//! Settings → Allodia account: the whole category, and the only place the app draws one.
//!
//! Its Apple, Android and Windows twins are `AllodiaAccountSettings.swift`, `SettingsAllodia.kt`
//! and `SettingsDialog.Allodia.cs`: keep the states and the wording in step.
//!
//! A category of its own rather than a group under Accounts, because an Allodia account is not a
//! mail account: no mailbox, no switcher entry, and a token issued for it cannot touch anyone's
//! mail.
//!
//! The **category** is absent in a build carrying no Allodia registration, which is the ordinary
//! answer for a build from source; never a row that opens an empty page.
//!
//! Drawn from the core's `allodia_card`, which says who is signed in, the sign-in out and what the
//! last one came to. Each piece takes the sender rather than the whole `PageContext`: they need
//! nothing else from it, and a context cannot be built without an app.

use adw::prelude::*;
use gtk::accessible::Property as AccessibleProperty;
use mailcal_bindings::{
    AccountChangeRefusal, AllodiaCard, AllodiaCardState, AllodiaFailure, ControlState,
};

use super::{PageContext, group, page_box};
use crate::{
    l10n,
    ui::{AppInput, controls, mailbox::plain_text_row},
};

/// The category's page.
///
/// Only ever built for a build that carries the registration: `visible_categories` drops the
/// category entirely otherwise, so there is no empty-page case to draw.
pub(super) fn page(ctx: &PageContext) -> gtk::Box {
    let content = page_box(l10n::settings_allodia_heading());
    if mailcal_bindings::allodia_sign_in_available() {
        content.append(&allodia(&ctx.app.allodia_card(), &ctx.sender));
    }
    // The subscription sits under the account it belongs to, and is drawn only while somebody is
    // signed in on a grant the service has not refused (`super::subscription`).
    if let Some(section) = super::subscription::subscription(ctx) {
        content.append(&section);
    }
    content
}

/// The group, drawn from the core's card.
pub(super) fn allodia(
    card: &AllodiaCard,
    sender: &relm4::Sender<AppInput>,
) -> adw::PreferencesGroup {
    let section = group(
        l10n::settings_allodia_heading(),
        l10n::settings_allodia_description(),
    );
    if let Some(browser) = &card.browser {
        section.add(&signing_in_row(sender, browser.cancel));
    } else {
        match &card.state {
            AllodiaCardState::SignedIn { account } => {
                section.add(&signed_in_row(sender, account, card.sign_out));
            }
            AllodiaCardState::Ended { account } => {
                section.add(&session_ended_row(
                    sender,
                    account,
                    card.sign_in,
                    card.sign_out,
                ));
            }
            // Signed out offers both routes. Someone who has no account and someone returning to
            // one need different pages, and a lone "Sign in" sends the first of them through a
            // form asking for a password they never set. The header suffix is where libadwaita
            // puts a group-level action, and it holds both.
            AllodiaCardState::SignedOut => {
                let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 6);
                buttons.append(&create_button(sender, card.create));
                buttons.append(&sign_in_button(sender, card.sign_in));
                section.set_header_suffix(Some(&buttons));
            }
        }
    }
    if let Some(failure) = &card.failure {
        section.add(&failure_row(&failure_said(failure)));
    }
    section
}

/// Why the last sign-in or sign-out did not work, in the card's words: also the first-run card's.
pub(in crate::ui) fn failure_said(failure: &AllodiaFailure) -> String {
    match failure {
        AllodiaFailure::SignIn { refusal } => match refusal {
            AccountChangeRefusal::StaleSignIn => l10n::signin_stale().to_owned(),
            AccountChangeRefusal::BrowserNotOpened => {
                l10n::settings_allodia_browser_failed().to_owned()
            }
            AccountChangeRefusal::NotFinished => l10n::settings_allodia_timeout().to_owned(),
            other => l10n::settings_allodia_failed(&crate::ui::core_error::refusal_said(other)),
        },
        AllodiaFailure::SignOut { detail } => l10n::settings_allodia_failed(detail),
    }
}

/// The browser hop is outstanding: no button to press again, and a way out.
fn signing_in_row(sender: &relm4::Sender<AppInput>, cancel: ControlState) -> adw::ActionRow {
    let row = plain_text_row();
    row.set_title(l10n::settings_allodia_signing_in());
    let spinner = adw::Spinner::new();
    row.add_suffix(&spinner);
    let sender = sender.clone();
    row.add_suffix(&controls::button(
        l10n::action_cancel(),
        cancel,
        false,
        move || {
            sender.emit(AppInput::CancelAllodiaSignIn);
        },
    ));
    row
}

/// Who is signed in, and a way out. The name is what the person recognises, but the **address** is
/// what identifies the account, so the address is always the title and the name is the subtitle
/// only when the service holds one.
fn signed_in_row(
    sender: &relm4::Sender<AppInput>,
    account: &mailcal_bindings::AllodiaAccount,
    sign_out: ControlState,
) -> adw::ActionRow {
    let row = plain_text_row();
    row.set_title(&l10n::settings_allodia_signed_in(&account.email));
    if let Some(name) = account.name.as_deref().filter(|name| !name.is_empty()) {
        row.set_subtitle(name);
    }
    let sign_out = sign_out_button(sender, sign_out);

    // Managing and deleting are the same page, named twice on purpose: an account someone can
    // create has to offer deletion somewhere findable, and "Manage account" is not the word
    // anybody looks for when they want out.
    let manage = gtk::Button::with_label(l10n::settings_allodia_manage());
    manage.set_valign(gtk::Align::Center);
    let to_manage = sender.clone();
    manage.connect_clicked(move |_| to_manage.emit(AppInput::ManageAllodiaAccount));

    let delete = gtk::Button::with_label(l10n::settings_allodia_delete());
    delete.set_valign(gtk::Align::Center);
    delete.add_css_class("destructive-action");
    let to_delete = sender.clone();
    delete.connect_clicked(move |_| to_delete.emit(AppInput::ManageAllodiaAccount));

    // An AdwActionRow labels itself from its title, so an explicit accessible label would be
    // ignored; the hint goes in the description, which is announced after the name. Set through
    // the widget: libadwaita's binding does not declare `Accessible` on the row itself.
    row.upcast_ref::<gtk::Widget>()
        .update_property(&[AccessibleProperty::Description(
            l10n::settings_allodia_manage_hint(),
        )]);

    row.add_suffix(&manage);
    row.add_suffix(&delete);
    row.add_suffix(&sign_out);
    row
}

/// The service refused the stored grant: the address, the sign-in that ended, and the two ways on,
/// in place of the signed-in row and with no Manage account (`entitlement.md`, "Signing in"). The
/// grant stays stored until the person presses one of them.
fn session_ended_row(
    sender: &relm4::Sender<AppInput>,
    account: &mailcal_bindings::AllodiaAccount,
    sign_in: ControlState,
    sign_out: ControlState,
) -> adw::ActionRow {
    let row = plain_text_row();
    row.set_title(&l10n::settings_allodia_session_ended(&account.email));
    row.set_subtitle(l10n::settings_allodia_session_ended_hint());

    let to_sign_in = sender.clone();
    let again = controls::button(
        l10n::settings_allodia_reauth_action(),
        sign_in,
        false,
        move || {
            to_sign_in.emit(AppInput::StartAllodiaSignIn);
        },
    );
    again.add_css_class("suggested-action");
    let sign_out = sign_out_button(sender, sign_out);

    row.add_suffix(&again);
    row.add_suffix(&sign_out);
    row
}

/// Creating an account: its own button beside the sign-in, not a link inside the sign-in page.
fn create_button(sender: &relm4::Sender<AppInput>, control: ControlState) -> gtk::Button {
    let sender = sender.clone();
    controls::button(l10n::settings_allodia_create(), control, false, move || {
        sender.emit(AppInput::StartAllodiaRegistration);
    })
}

fn sign_in_button(sender: &relm4::Sender<AppInput>, control: ControlState) -> gtk::Button {
    let sender = sender.clone();
    let sign_in = controls::button(
        l10n::settings_allodia_sign_in(),
        control,
        false,
        move || {
            sender.emit(AppInput::StartAllodiaSignIn);
        },
    );
    sign_in.add_css_class("suggested-action");
    sign_in
}

fn sign_out_button(sender: &relm4::Sender<AppInput>, control: ControlState) -> gtk::Button {
    let sender = sender.clone();
    controls::button(
        l10n::settings_allodia_sign_out(),
        control,
        false,
        move || {
            sender.emit(AppInput::SignOutOfAllodia);
        },
    )
}

/// Why the last attempt did not work. Its own row rather than the group's description, which says
/// what an Allodia account *is* and stays true whatever happened.
fn failure_row(failure: &str) -> adw::ActionRow {
    let row = plain_text_row();
    row.set_title(failure);
    row.add_css_class("error");
    row
}

#[cfg(test)]
#[path = "allodia_tests.rs"]
pub(crate) mod tests;
