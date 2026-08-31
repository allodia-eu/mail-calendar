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
//! Each piece takes the sender rather than the whole `PageContext`, the shape `secret_remedy_row`
//! uses next door in `accounts`: they need nothing else from it, and a context cannot be built
//! without an app.

use adw::prelude::*;
use gtk::accessible::Property as AccessibleProperty;

use super::{PageContext, group, page_box};
use crate::{
    l10n,
    ui::{AppInput, mailbox::plain_text_row},
};

/// The category's page.
///
/// Only ever built for a build that carries the registration: `visible_categories` drops the
/// category entirely otherwise, so there is no empty-page case to draw.
pub(super) fn page(ctx: &PageContext) -> gtk::Box {
    let content = page_box(l10n::settings_allodia_heading());
    if let Some(section) = allodia(ctx) {
        content.append(&section);
    }
    content
}

/// The group, or `None` when this build carries no Allodia sign-in.
pub(super) fn allodia(ctx: &PageContext) -> Option<adw::PreferencesGroup> {
    if !mailcal_bindings::allodia_sign_in_available() {
        return None;
    }
    let section = group(
        l10n::settings_allodia_heading(),
        l10n::settings_allodia_description(),
    );
    if ctx.allodia_signing_in {
        section.add(&signing_in_row(&ctx.sender));
    } else if let Some(account) = ctx.app.allodia_account() {
        section.add(&signed_in_row(&ctx.sender, &account));
    } else {
        // Signed out offers both routes. Someone who has no account and someone returning to one
        // need different pages, and a lone "Sign in" sends the first of them through a form asking
        // for a password they never set. The header suffix is where libadwaita puts a group-level
        // action, and it holds both.
        let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        buttons.append(&create_button(&ctx.sender));
        buttons.append(&sign_in_button(&ctx.sender));
        section.set_header_suffix(Some(&buttons));
    }
    if let Some(failure) = &ctx.allodia_failure {
        section.add(&failure_row(failure));
    }
    Some(section)
}

/// The browser hop is outstanding: no button to press again, and a way out; a dismissed browser
/// gives this listener nothing until its five-minute cap.
fn signing_in_row(sender: &relm4::Sender<AppInput>) -> adw::ActionRow {
    let row = plain_text_row();
    row.set_title(l10n::settings_allodia_signing_in());
    let spinner = gtk::Spinner::new();
    spinner.start();
    row.add_suffix(&spinner);
    let cancel = gtk::Button::with_label(l10n::action_cancel());
    cancel.set_valign(gtk::Align::Center);
    let sender = sender.clone();
    cancel.connect_clicked(move |_| sender.emit(AppInput::CancelAllodiaSignIn));
    row.add_suffix(&cancel);
    row
}

/// Who is signed in, and a way out. The name is what the person recognises, but the **address** is
/// what identifies the account, so the address is always the title and the name is the subtitle
/// only when the service holds one.
fn signed_in_row(
    sender: &relm4::Sender<AppInput>,
    account: &mailcal_bindings::AllodiaAccount,
) -> adw::ActionRow {
    let row = plain_text_row();
    row.set_title(&l10n::settings_allodia_signed_in(&account.email));
    if let Some(name) = account.name.as_deref().filter(|name| !name.is_empty()) {
        row.set_subtitle(name);
    }
    let sign_out = gtk::Button::with_label(l10n::settings_allodia_sign_out());
    sign_out.set_valign(gtk::Align::Center);
    let out = sender.clone();
    sign_out.connect_clicked(move |_| out.emit(AppInput::SignOutOfAllodia));

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

/// Creating an account: its own button beside the sign-in, not a link inside the sign-in page.
fn create_button(sender: &relm4::Sender<AppInput>) -> gtk::Button {
    let create = gtk::Button::with_label(l10n::settings_allodia_create());
    create.set_valign(gtk::Align::Center);
    let sender = sender.clone();
    create.connect_clicked(move |_| sender.emit(AppInput::StartAllodiaRegistration));
    create
}

fn sign_in_button(sender: &relm4::Sender<AppInput>) -> gtk::Button {
    let sign_in = gtk::Button::with_label(l10n::settings_allodia_sign_in());
    sign_in.add_css_class("suggested-action");
    sign_in.set_valign(gtk::Align::Center);
    let sender = sender.clone();
    sign_in.connect_clicked(move |_| sender.emit(AppInput::StartAllodiaSignIn));
    sign_in
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
