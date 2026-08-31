//! The JMAP routes: a detected card whose provider sign-in replaces the secret, and the manual
//! form, where the sign-in is offered beside the secret rather than instead of it.

use std::rc::Rc;

use adw::prelude::*;

use super::{
    AppInput,
    setup_manual::FormSnapshot,
    setup_model::{AccountSubmission, JmapForm, JmapSignIn, JmapSubmission, ManualForm},
    setup_widgets::{
        actions, body, caption, detected_row, edit_manually_button, entry, gate_on_trust, primary,
        section, show_error, trust_approved, trust_gate,
    },
};
use crate::l10n;

/// The detected card. Detection already found the server, so it is shown as a row rather than
/// an editable field; when the server's own metadata advertises sign-in, one button replaces
/// the secret entirely, and any failure hands it straight back.
pub(super) fn detected_fields(
    content: &gtk::Box,
    window: &gtk::Window,
    form: &JmapForm,
    error: Option<&str>,
    required: bool,
    sender: &relm4::Sender<AppInput>,
) {
    content.append(&section(l10n::setup_detect_section_email()));
    if !form.server_url.trim().is_empty() {
        content.append(&detected_row("JMAP", &host_of(&form.server_url)));
    }
    if form.sign_in == JmapSignIn::Checking {
        // Neither the offer nor the secret field belongs here until the server has answered.
        content.append(&caption(l10n::setup_jmap_signin_checking()));
    }
    if form.sign_in.show_offer() {
        content.append(&body(l10n::setup_jmap_signin_note()));
    }
    if form.sign_in == JmapSignIn::Failed {
        let message = body(l10n::setup_jmap_signin_failed());
        message.add_css_class("error");
        content.append(&message);
    }

    let secret = form.sign_in.show_manual().then(|| {
        content.append(&body(l10n::setup_detect_found_jmap_note()));
        let trust = trust_gate(content, form.trusted);
        let password = entry(l10n::setup_jmap_secret_placeholder(), "", true);
        content.append(&password);
        content.append(&caption(l10n::setup_jmap_secret_note()));
        (trust, password)
    });
    show_error(content, error);

    let actions = actions(window, required, sender);
    actions.append(&edit_manually_button(sender));
    if form.sign_in.show_offer() {
        let button = sign_in_button(window);
        let (email, server_url) = (form.email.clone(), form.server_url.clone());
        let input = sender.clone();
        button.connect_clicked(move |_| {
            if !email.trim().is_empty() {
                input.emit(AppInput::StartJmapLogin(email.clone(), server_url.clone()));
            }
        });
        actions.append(&button);
    }
    if let Some((trust, password)) = secret {
        let connect = if form.sign_in.show_offer() {
            gtk::Button::with_label(l10n::action_connect())
        } else {
            primary(l10n::action_connect(), window)
        };
        gate_on_trust(&trust, &connect, form.trusted);
        let base = form.clone();
        let input = sender.clone();
        let dialog = window.clone();
        connect.connect_clicked(move |_| {
            if !trust_approved(base.trusted, trust.is_active()) || password.text().is_empty() {
                return;
            }
            input.emit(AppInput::SubmitAccount(Box::new(AccountSubmission::Jmap(
                JmapSubmission {
                    email: base.email.clone(),
                    server_url: base.server_url.clone(),
                    password: password.text().to_string(),
                },
            ))));
            dialog.set_visible(false);
        });
        actions.append(&connect);
    }
    content.append(&actions);
}

/// The manual form. The secret always stays: the user came here to type one, and the
/// pre-flight runs against whatever address they end up entering.
pub(super) fn manual_fields(
    content: &gtk::Box,
    window: &gtk::Window,
    form: &ManualForm,
    error: Option<&str>,
    required: bool,
    sender: &relm4::Sender<AppInput>,
) -> FormSnapshot {
    content.append(&body(l10n::setup_jmap_note()));
    let email = entry(l10n::setup_field_email(), &form.email, false);
    let server = entry(
        l10n::setup_jmap_server_placeholder(),
        &form.jmap_server,
        false,
    );
    content.append(&email);
    content.append(&server);
    if form.sign_in == JmapSignIn::Failed {
        let message = body(l10n::setup_jmap_signin_failed());
        message.add_css_class("error");
        content.append(&message);
    }
    if form.sign_in.show_offer() {
        content.append(&body(l10n::setup_jmap_signin_note()));
    }
    let password = entry(l10n::setup_jmap_secret_placeholder(), "", true);
    content.append(&password);
    content.append(&caption(l10n::setup_jmap_secret_note()));
    show_error(content, error);

    let snapshot: FormSnapshot = {
        let base = form.clone();
        let (email, server) = (email.clone(), server.clone());
        Rc::new(move || ManualForm {
            email: email.text().trim().to_owned(),
            jmap_server: server.text().trim().to_owned(),
            ..base.clone()
        })
    };
    // The pre-flight blocks on network round trips, so it runs when the user moves on from the
    // address rather than per keystroke; and from the **address** only. Leaving the server
    // field too would put a second answer, and the rebuild that shows its button, right where
    // the user is typing the secret. A server typed afterwards costs nothing: the sign-in is
    // started with whatever is in the fields, and a server that turns out not to support it
    // fails soft back to the secret below.
    probe_on_leave(&email, &snapshot, sender);

    let actions = actions(window, required, sender);
    if form.sign_in.show_offer() {
        let button = sign_in_button(window);
        let (hint_email, hint_server) = (email.clone(), server.clone());
        let input = sender.clone();
        button.connect_clicked(move |_| {
            let address = hint_email.text().trim().to_owned();
            if !address.is_empty() {
                input.emit(AppInput::StartJmapLogin(
                    address,
                    hint_server.text().trim().to_owned(),
                ));
            }
        });
        actions.append(&button);
    }
    let connect = if form.sign_in.show_offer() {
        gtk::Button::with_label(l10n::action_connect())
    } else {
        primary(l10n::action_connect(), window)
    };
    let input = sender.clone();
    let dialog = window.clone();
    connect.connect_clicked(move |_| {
        let submission = JmapSubmission {
            email: email.text().trim().to_owned(),
            server_url: server.text().trim().to_owned(),
            password: password.text().to_string(),
        };
        if submission.email.is_empty() || submission.password.is_empty() {
            return;
        }
        input.emit(AppInput::SubmitAccount(Box::new(AccountSubmission::Jmap(
            submission,
        ))));
        dialog.set_visible(false);
    });
    actions.append(&connect);
    content.append(&actions);
    snapshot
}

pub(super) fn signing_in(sender: &relm4::Sender<AppInput>) -> gtk::Box {
    super::setup_widgets::waiting(
        l10n::setup_jmap_signin_button(),
        || AppInput::CancelJmapLogin,
        sender,
    )
}

/// The provider sign-in action, as the window's default. Each caller wires its own address:
/// the detected card the one detection found, the manual form whatever is typed.
fn sign_in_button(window: &gtk::Window) -> gtk::Button {
    primary(l10n::setup_jmap_signin_button(), window)
}

fn probe_on_leave(field: &gtk::Entry, snapshot: &FormSnapshot, sender: &relm4::Sender<AppInput>) {
    let focus = gtk::EventControllerFocus::new();
    let snapshot = Rc::clone(snapshot);
    let input = sender.clone();
    focus.connect_leave(move |_| {
        input.emit(AppInput::ProbeManualJmapSignIn(Box::new(snapshot())));
    });
    field.add_controller(focus);
}

/// The server as a name to recognise; the whole URL when it does not parse.
fn host_of(server_url: &str) -> String {
    url::Url::parse(server_url)
        .ok()
        .and_then(|parsed| parsed.host_str().map(str::to_owned))
        .unwrap_or_else(|| server_url.to_owned())
}

#[cfg(test)]
mod tests {
    use super::{JmapSignIn, host_of};

    #[test]
    fn a_detected_server_that_offers_sign_in_takes_the_secret_away_until_it_fails() {
        // While it asks, the card offers neither; nothing appears only to be taken away.
        assert!(!JmapSignIn::Checking.show_offer());
        assert!(!JmapSignIn::Checking.show_manual());
        assert!(JmapSignIn::Offered.show_offer());
        assert!(!JmapSignIn::Offered.show_manual());
        // A failed sign-in is not a dead end: the secret comes back beside the retry.
        assert!(JmapSignIn::Failed.show_offer());
        assert!(JmapSignIn::Failed.show_manual());
        assert!(!JmapSignIn::Unavailable.show_offer());
        assert!(JmapSignIn::Unavailable.show_manual());
    }

    #[test]
    fn a_detected_server_is_shown_by_host() {
        assert_eq!(
            host_of("https://api.fastmail.com/jmap/"),
            "api.fastmail.com"
        );
        assert_eq!(host_of(""), "");
    }
}
