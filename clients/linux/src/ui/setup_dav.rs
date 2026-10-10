//! The calendar-and-contacts route: detection found a calendar or address-book server for the
//! address and no mail server, so the account is set up for those alone, with the address as its
//! login (`docs/account-autodetect.md` rule 8).

use std::rc::Rc;

use adw::prelude::*;
use mailcal_bindings::{ConnectionSecurity, RejectedCertificate};

use super::{
    AppInput,
    setup_model::{AccountSubmission, DavForm, ImapSubmission},
    setup_pane::{ConnectPane, connecting_readout},
    setup_uses,
    setup_widgets::{actions, body, caption, edit_manually_button, entry, gate_connect, primary},
};
use crate::l10n;

pub(super) fn detected_fields(
    content: &gtk::Box,
    window: &gtk::Window,
    form: &DavForm,
    error: Option<&str>,
    certificate: Option<&RejectedCertificate>,
    required: bool,
    sender: &relm4::Sender<AppInput>,
) -> ConnectPane {
    content.append(&body(l10n::setup_detect_dav_note()));
    let uses = Rc::new(setup_uses::append(content, &form.offer, &[]));
    content.append(&caption(l10n::setup_credentials_note()));
    let password = entry(l10n::setup_field_password(), "", true);
    content.append(&password);
    let feedback = gtk::Box::new(gtk::Orientation::Vertical, 14);
    content.append(&feedback);

    let actions = actions(window, required, sender);
    actions.append(&edit_manually_button(sender));
    let connect = primary(l10n::action_connect(), window);
    let readout = connecting_readout(l10n::status_connecting());
    actions.append(&readout);
    // Every endpoint here was found over HTTPS, so there are no untrusted settings to approve.
    let gate = gate_connect(&connect, None, true, &password);
    let pane = ConnectPane::new(&feedback, gate, &connect, &readout, true);
    pane.show_result(error, certificate);
    let refused = pane.accepted();
    let email = form.email.clone();
    let input = sender.clone();
    connect.connect_clicked(move |_| {
        if password.text().is_empty() {
            return;
        }
        let chosen = uses.chosen();
        input.emit(AppInput::SubmitAccount(Box::new(AccountSubmission::Imap(
            ImapSubmission {
                email: email.clone(),
                imap_host: String::new(),
                smtp_host: String::new(),
                caldav_url: chosen.caldav_url,
                carddav_url: chosen.carddav_url,
                uses: chosen.uses,
                imap_security: ConnectionSecurity::ImplicitTls,
                smtp_security: ConnectionSecurity::ImplicitTls,
                password: password.text().to_string(),
                accepted_certificate: refused.borrow().clone(),
            },
        ))));
    });
    actions.append(&connect);
    content.append(&actions);
    pane
}
