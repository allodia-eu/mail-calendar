//! The IMAP routes: a detected card the user confirms with a password, and the manual form.

use std::rc::Rc;

use adw::prelude::*;
use mailcal_bindings::RejectedCertificate;

use super::{
    AppInput,
    setup_manual::FormSnapshot,
    setup_model::{
        AccountSubmission, DetectedServer, ImapForm, ImapSignIn, ImapSubmission, ManualForm,
    },
    setup_pane::{ConnectPane, connecting_readout},
    setup_server_field::ServerPair,
    setup_server_row::manual_server_row,
    setup_uses::{self, ChosenUses},
    setup_widgets::{
        actions, body, caption, detected_row, edit_manually_button, entry, gate_connect,
        gate_on_trust, primary, show_error, trust_approved, trust_gate,
    },
};
use crate::l10n;

/// The detected card: what the account is used for, with the servers detection found shown to
/// be recognised rather than retyped, and the one thing only the person has, the password.
pub(super) fn detected_fields(
    content: &gtk::Box,
    window: &gtk::Window,
    form: &ImapForm,
    error: Option<&str>,
    certificate: Option<&RejectedCertificate>,
    required: bool,
    sender: &relm4::Sender<AppInput>,
) -> Option<ConnectPane> {
    let mut mail_rows: Vec<gtk::Widget> = vec![server_row(&form.incoming).upcast()];
    if let Some(outgoing) = &form.outgoing {
        mail_rows.push(server_row(outgoing).upcast());
    }
    let uses = Rc::new(setup_uses::append(content, &form.offer, &mail_rows));
    sign_in_explanation(content, &form.sign_in);

    // The trust gate belongs above whichever credential the user is about to hand over, and
    // it gates the sign-in button too: an untrusted config decides which *server* the browser
    // is sent to, so approving it matters at least as much there as for a typed password.
    let trust = trust_gate(content, form.trusted);
    // The sign-in leads, in the content under the line that explains it, with the password
    // behind "Use a password instead" beneath it (`docs/mail-oauth.md` rule 2).
    let sign_in = form.sign_in.show_offer().then(|| {
        let button = primary(l10n::setup_imap_signin_button(), window);
        button.set_halign(gtk::Align::Start);
        gate_on_trust(&trust, &button, form.trusted);
        let base = form.clone();
        let input = sender.clone();
        let approved = trust.clone();
        let chosen = Rc::clone(&uses);
        button.connect_clicked(move |_| {
            if trust_approved(base.trusted, approved.is_active()) {
                let form = with_chosen(&base, chosen.chosen());
                input.emit(AppInput::StartImapLogin(Box::new(form)));
            }
        });
        content.append(&button);
        button
    });
    let instead = (sign_in.is_some() && form.sign_in.show_password()).then(|| {
        let instead = gtk::Button::with_label(l10n::setup_imap_signin_password_instead());
        instead.set_halign(gtk::Align::Start);
        content.append(&instead);
        instead
    });
    let secret = form.sign_in.show_password().then(|| {
        let area = gtk::Box::new(gtk::Orientation::Vertical, 14);
        area.append(&caption(l10n::setup_detect_app_password_hint()));
        area.append(&caption(l10n::setup_credentials_note()));
        let password = entry(l10n::setup_field_password(), "", true);
        area.append(&password);
        content.append(&area);
        (area, password)
    });
    // Where a refusal draws itself, into the pane that asked; filled by `ConnectPane`.
    let feedback = gtk::Box::new(gtk::Orientation::Vertical, 14);
    content.append(&feedback);

    let actions = actions(window, required, sender);
    actions.append(&edit_manually_button(sender));
    let mut pane: Option<ConnectPane> = None;
    if let Some((area, password)) = secret {
        let connect = match &instead {
            Some(_) => gtk::Button::with_label(l10n::action_connect()),
            None => primary(l10n::action_connect(), window),
        };
        let readout = connecting_readout(l10n::status_connecting());
        let gate = gate_connect(&connect, Some(&trust), form.trusted, &password);
        let connected = ConnectPane::new(&feedback, gate, &connect, &readout, true);
        if let (Some(instead), Some(sign_in)) = (&instead, &sign_in) {
            // Connect sits in the field's own row, so it is on screen exactly when the field is.
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            row.set_halign(gtk::Align::End);
            row.append(&readout);
            row.append(&connect);
            area.append(&row);
            password_behind(window, instead, sign_in, &area, &connect, &password);
        } else {
            actions.append(&readout);
            actions.append(&connect);
        }
        let base = form.clone();
        let refused = connected.accepted();
        let input = sender.clone();
        connect.connect_clicked(move |_| {
            if !trust_approved(base.trusted, trust.is_active()) || password.text().is_empty() {
                return;
            }
            let chosen = uses.chosen();
            input.emit(AppInput::SubmitAccount(Box::new(AccountSubmission::Imap(
                ImapSubmission {
                    email: base.email.clone(),
                    imap_host: base.imap_host.clone(),
                    smtp_host: base.smtp_host.clone(),
                    caldav_url: chosen.caldav_url,
                    carddav_url: chosen.carddav_url,
                    uses: chosen.uses,
                    imap_security: base.imap_security,
                    smtp_security: base.smtp_security,
                    password: password.text().to_string(),
                    // Read now rather than captured when the pane was built: the pane outlives
                    // the refusal it is answering.
                    accepted_certificate: refused.borrow().clone(),
                },
            ))));
        });
        pane = Some(connected);
    }
    content.append(&actions);
    match &pane {
        Some(pane) => pane.show_result(error, certificate),
        // A card that only offers the sign-in has no Connect to answer for; what the sign-in's
        // own connect said still needs somewhere to land.
        None => show_error(&feedback, error),
    }
    pane
}

/// Keeps the password field and its Connect hidden until "Use a password instead" is pressed,
/// which draws both, makes Connect the form's action and leaves the sign-in as an ordinary
/// button.
fn password_behind(
    window: &gtk::Window,
    instead: &gtk::Button,
    sign_in: &gtk::Button,
    area: &gtk::Box,
    connect: &gtk::Button,
    password: &gtk::Entry,
) {
    area.set_visible(false);
    let (window, sign_in, area, connect, password) = (
        window.clone(),
        sign_in.clone(),
        area.clone(),
        connect.clone(),
        password.clone(),
    );
    instead.connect_clicked(move |instead| {
        area.set_visible(true);
        instead.set_visible(false);
        sign_in.remove_css_class("suggested-action");
        connect.add_css_class("suggested-action");
        window.set_default_widget(Some(&connect));
        password.set_activates_default(true);
        password.grab_focus();
    });
}

/// The card's form as the sign-in takes it: with what the person chose on the card.
fn with_chosen(form: &ImapForm, chosen: ChosenUses) -> ImapForm {
    ImapForm {
        caldav_url: chosen.caldav_url,
        carddav_url: chosen.carddav_url,
        uses: chosen.uses,
        ..form.clone()
    }
}

/// Asks again whenever the user leaves a field the answer depends on.
fn probe_on_leave(field: &gtk::Entry, snapshot: &FormSnapshot, sender: &relm4::Sender<AppInput>) {
    let focus = gtk::EventControllerFocus::new();
    let snapshot = Rc::clone(snapshot);
    let input = sender.clone();
    focus.connect_leave(move |_| {
        input.emit(AppInput::ProbeManualImapSignIn(Box::new(snapshot())));
    });
    field.add_controller(focus);
}

/// The screen shown while the browser holds the sign-in, with the one action left: cancel.
pub(super) fn signing_in(sender: &relm4::Sender<AppInput>) -> gtk::Box {
    super::setup_widgets::waiting(
        l10n::setup_imap_signin_button(),
        || AppInput::CancelImapLogin,
        sender,
    )
}

/// The line that says what the server answered, when it says something.
///
/// Silent in the ordinary case, which is a provider that takes a password and always did:
/// there is nothing to explain and a line saying so would be noise on every setup.
fn sign_in_explanation(content: &gtk::Box, sign_in: &ImapSignIn) {
    match sign_in {
        ImapSignIn::Checking => content.append(&caption(l10n::setup_imap_signin_checking())),
        ImapSignIn::Offered { .. } => content.append(&body(l10n::setup_imap_signin_note())),
        ImapSignIn::RegistrationNeeded {
            password_also_works: true,
        } => content.append(&body(l10n::setup_imap_signin_registration_needed())),
        ImapSignIn::RegistrationNeeded {
            password_also_works: false,
        } => content.append(&body(l10n::setup_imap_signin_unsupported())),
        ImapSignIn::Failed => {
            let message = body(l10n::setup_imap_signin_failed());
            message.add_css_class("error");
            content.append(&message);
        }
        ImapSignIn::Password => {}
    }
}

/// The manual form: every field typed by hand, for a server autodetection could not find.
pub(super) fn manual_fields(
    content: &gtk::Box,
    window: &gtk::Window,
    form: &ManualForm,
    error: Option<&str>,
    certificate: Option<&RejectedCertificate>,
    required: bool,
    sender: &relm4::Sender<AppInput>,
) -> (FormSnapshot, ConnectPane) {
    content.append(&caption(l10n::setup_credentials_note()));
    let email = entry(l10n::setup_field_email(), &form.email, false);
    content.append(&email);
    let imap = manual_server_row(
        content,
        l10n::setup_field_mail_server(),
        &form.imap_host,
        &form.servers.imap,
    );
    let password = entry(l10n::setup_field_password(), "", true);
    content.append(&password);
    let smtp = manual_server_row(
        content,
        l10n::setup_field_smtp_optional(),
        &form.smtp_host,
        &form.servers.smtp,
    );
    let caldav = entry(l10n::setup_field_caldav_optional(), &form.caldav_url, false);
    content.append(&caldav);
    let carddav = entry(
        l10n::setup_field_carddav_optional(),
        &form.carddav_url,
        false,
    );
    content.append(&carddav);
    content.append(&caption(l10n::setup_port_note()));
    let feedback = gtk::Box::new(gtk::Orientation::Vertical, 14);
    content.append(&feedback);

    // Leaving either field asks the server what it accepts for what is typed now. The
    // password field stays put throughout, so an answer can only ever *add* a sign-in button
    // or a line of explanation: rebuilding over a password being typed would erase it.
    let snapshot: FormSnapshot = {
        let base = form.clone();
        let (email, caldav, carddav) = (email.clone(), caldav.clone(), carddav.clone());
        let (imap_row, smtp_row) = (imap.clone(), smtp.clone());
        Rc::new(move || ManualForm {
            email: email.text().trim().to_owned(),
            imap_host: imap_row.host_text(),
            smtp_host: smtp_row.host_text(),
            caldav_url: caldav.text().trim().to_owned(),
            carddav_url: carddav.text().trim().to_owned(),
            servers: ServerPair {
                imap: imap_row.read(),
                smtp: smtp_row.read(),
            },
            ..base.clone()
        })
    };

    for field in [&email, &imap.host] {
        probe_on_leave(field, &snapshot, sender);
    }
    sign_in_explanation(content, &form.imap_sign_in);
    if form.imap_sign_in.show_offer() {
        let button = gtk::Button::with_label(l10n::setup_imap_signin_button());
        let snapshot = Rc::clone(&snapshot);
        let input = sender.clone();
        button.connect_clicked(move |_| {
            input.emit(AppInput::StartImapLogin(Box::new(snapshot().into())));
        });
        content.append(&button);
    }

    let actions = actions(window, required, sender);
    let connect = primary(l10n::action_connect(), window);
    let readout = connecting_readout(l10n::status_connecting());
    actions.append(&readout);
    let gate = gate_connect(&connect, None, true, &password);
    let pane = ConnectPane::new(&feedback, gate, &connect, &readout, true);
    pane.show_result(error, certificate);
    // The one answer that takes the field away: the server said a password does not work.
    if form.imap_sign_in.refuses_password() {
        password.set_visible(false);
        connect.set_visible(false);
    }
    let refused = pane.accepted();
    let input = sender.clone();
    connect.connect_clicked(move |_| {
        let submission = ImapSubmission {
            email: email.text().trim().to_owned(),
            imap_host: imap.dial(),
            smtp_host: smtp.dial(),
            caldav_url: caldav.text().trim().to_owned(),
            // The servers given decide: an address book beside the mail is used for contacts.
            carddav_url: carddav.text().trim().to_owned(),
            uses: None,
            imap_security: imap.read().security(),
            smtp_security: smtp.read().security(),
            password: password.text().to_string(),
            accepted_certificate: refused.borrow().clone(),
        };
        if submission.email.is_empty()
            || submission.imap_host.is_empty()
            || submission.password.is_empty()
        {
            return;
        }
        input.emit(AppInput::SubmitAccount(Box::new(AccountSubmission::Imap(
            submission,
        ))));
    });
    actions.append(&connect);
    content.append(&actions);
    (snapshot, pane)
}

fn server_row(row: &DetectedServer) -> gtk::Box {
    detected_row(
        &row.protocol,
        &format!("{}:{} · {}", row.hostname, row.port, row.security),
    )
}
