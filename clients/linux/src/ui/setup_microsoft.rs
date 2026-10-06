//! The Microsoft 365 route: one browser sign-in, no server details. Microsoft retired Basic
//! auth, so a detected Microsoft address never gets a password form
//! (`docs/account-autodetect.md` → Routing).

use std::rc::Rc;

use adw::prelude::*;

use super::{
    AppInput,
    setup_manual::FormSnapshot,
    setup_model::{ManualForm, OAuthForm},
    setup_uses,
    setup_widgets::{actions, body, edit_manually_button, entry, primary, show_error, waiting},
};
use crate::l10n;

/// The detected card: this address is a Microsoft account, so the only step left is signing in.
pub(super) fn detected_fields(
    content: &gtk::Box,
    window: &gtk::Window,
    form: &OAuthForm,
    error: Option<&str>,
    required: bool,
    sender: &relm4::Sender<AppInput>,
) {
    content.append(&body(l10n::setup_detect_microsoft_hint()));
    content.append(&body(l10n::setup_microsoft_choice_note()));
    // What the account is used for decides what the sign-in asks for (`docs/accounts.md`
    // rule 10).
    let uses = setup_uses::append(content, &form.offer, &[]);
    // A declined or org-blocked sign-in shows here rather than leaving a button that silently
    // does nothing (`docs/provider-oauth.md` rule 9).
    show_error(content, error);

    let actions = actions(window, required, sender);
    actions.append(&edit_manually_button(sender));
    let button = primary(l10n::setup_microsoft_signin(), window);
    let email = form.email.clone();
    let input = sender.clone();
    button.connect_clicked(move |_| {
        input.emit(AppInput::StartMicrosoftLogin(
            email.clone(),
            uses.chosen().uses,
        ));
    });
    actions.append(&button);
    content.append(&actions);
}

/// The manual pane: the same sign-in, with the address typed rather than detected. It targets
/// the sign-in at that account (`login_hint`); left blank, Microsoft shows its picker.
pub(super) fn manual_fields(
    content: &gtk::Box,
    window: &gtk::Window,
    form: &ManualForm,
    error: Option<&str>,
    required: bool,
    sender: &relm4::Sender<AppInput>,
) -> FormSnapshot {
    content.append(&body(l10n::setup_microsoft_choice_note()));
    let email = entry(l10n::setup_field_email(), &form.email, false);
    content.append(&email);
    let offer = setup_uses::provider_offer(mailcal_bindings::AccountKind::Microsoft, &form.email);
    let uses = setup_uses::append(content, &offer, &[]);
    show_error(content, error);

    let snapshot = address_snapshot(form, &email);
    let kind = mailcal_bindings::AccountKind::Microsoft;
    setup_uses::follow_address(&email, kind, &offer, &snapshot, sender);
    let actions = actions(window, required, sender);
    let button = primary(l10n::setup_microsoft_signin(), window);
    let input = sender.clone();
    button.connect_clicked(move |_| {
        let typed = email.text().trim().to_owned();
        let uses = setup_uses::provider_uses(kind, &typed, &uses);
        input.emit(AppInput::StartMicrosoftLogin(typed, uses));
    });
    actions.append(&button);
    content.append(&actions);
    snapshot
}

/// The address is all an OAuth pane holds, and it is worth carrying to the next account type.
fn address_snapshot(form: &ManualForm, email: &gtk::Entry) -> FormSnapshot {
    let base = form.clone();
    let email = email.clone();
    Rc::new(move || ManualForm {
        email: email.text().trim().to_owned(),
        ..base.clone()
    })
}

pub(super) fn signing_in(sender: &relm4::Sender<AppInput>) -> gtk::Box {
    waiting(
        l10n::setup_microsoft_signing_in(),
        || AppInput::CancelMicrosoftLogin,
        sender,
    )
}
