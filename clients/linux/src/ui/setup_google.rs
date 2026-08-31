//! The Google route: the Early Access gate, then one browser sign-in. Never a password form,
//! Gmail and Google Calendar are native-API integrations (`docs/provider-oauth.md` → "## Google").

use std::rc::Rc;

use adw::prelude::*;

use super::{
    AppInput,
    setup_manual::FormSnapshot,
    setup_model::{ManualForm, OAuthForm},
    setup_widgets::{actions, body, edit_manually_button, entry, primary, show_error, waiting},
};
use crate::l10n;

/// The detected card: this address is a Google account, so the only step left is signing in.
pub(super) fn detected_fields(
    content: &gtk::Box,
    window: &gtk::Window,
    form: &OAuthForm,
    error: Option<&str>,
    required: bool,
    sender: &relm4::Sender<AppInput>,
) {
    content.append(&body(l10n::setup_detect_google_hint()));
    let gate = gated_note(content, error);
    let actions = actions(window, required, sender);
    actions.append(&edit_manually_button(sender));
    actions.append(&sign_in(window, &gate, form.email.clone(), sender));
    content.append(&actions);
}

/// The manual pane: the same gate and button, with the address typed rather than detected. It
/// targets the sign-in at that account (`login_hint`); left blank, Google shows its picker.
pub(super) fn manual_fields(
    content: &gtk::Box,
    window: &gtk::Window,
    form: &ManualForm,
    error: Option<&str>,
    required: bool,
    sender: &relm4::Sender<AppInput>,
) -> FormSnapshot {
    let email = entry(l10n::setup_field_email(), &form.email, false);
    content.append(&email);
    let gate = gated_note(content, error);
    let snapshot = address_snapshot(form, &email);
    let actions = actions(window, required, sender);
    let button = primary(l10n::setup_google_signin(), window);
    button.set_sensitive(false);
    gate_button(&gate, &button);
    let input = sender.clone();
    button.connect_clicked(move |_| {
        input.emit(AppInput::StartGoogleLogin(email.text().trim().to_owned()));
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
        l10n::setup_google_signing_in(),
        || AppInput::CancelGoogleLogin,
        sender,
    )
}

/// What Google's sign-in needs above the button: what the flow does, and the mandatory Early
/// Access confirmation while Google reviews the app for the restricted scopes.
fn gated_note(content: &gtk::Box, error: Option<&str>) -> gtk::CheckButton {
    content.append(&body(l10n::setup_google_note()));

    let early_access = body(l10n::setup_google_early_access_title());
    early_access.add_css_class("heading");
    content.append(&early_access);
    content.append(&body(l10n::setup_google_early_access_body()));
    let signup = gtk::LinkButton::with_label(
        l10n::setup_google_early_access_url(),
        l10n::setup_google_early_access_link(),
    );
    signup.set_halign(gtk::Align::Start);
    content.append(&signup);

    let confirmed = gtk::CheckButton::with_label(l10n::setup_google_early_access_confirm());
    content.append(&confirmed);
    show_error(content, error);
    confirmed
}

fn sign_in(
    window: &gtk::Window,
    gate: &gtk::CheckButton,
    email: String,
    sender: &relm4::Sender<AppInput>,
) -> gtk::Button {
    let button = primary(l10n::setup_google_signin(), window);
    button.set_sensitive(false);
    gate_button(gate, &button);
    let input = sender.clone();
    button.connect_clicked(move |_| input.emit(AppInput::StartGoogleLogin(email.clone())));
    button
}

fn gate_button(gate: &gtk::CheckButton, button: &gtk::Button) {
    let gated = button.clone();
    gate.connect_toggled(move |choice| {
        gated.set_sensitive(sign_in_enabled(choice.is_active()));
    });
}

const fn sign_in_enabled(early_access_confirmed: bool) -> bool {
    early_access_confirmed
}

#[cfg(test)]
mod tests {
    use super::sign_in_enabled;

    #[test]
    fn early_access_confirmation_is_the_google_sign_in_gate() {
        assert!(!sign_in_enabled(false));
        assert!(sign_in_enabled(true));
    }
}
