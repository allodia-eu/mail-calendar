//! The calendar-and-contacts route: detection found a calendar or address-book server for the
//! address and no mail server, so the account is set up for those alone, with the address as its
//! login (`docs/account-autodetect.md` rule 8).

use std::rc::Rc;

use adw::prelude::*;
use mailcal_bindings::{AccountCapability, ConnectionSecurity, RejectedCertificate};

use super::{
    AppInput,
    setup_manual::FormSnapshot,
    setup_model::{AccountSubmission, DavForm, ImapSubmission, ManualForm},
    setup_pane::{ConnectPane, connecting_readout},
    setup_uses,
    setup_widgets::{actions, body, caption, edit_manually_button, entry, gate_connect, primary},
};
use crate::l10n;

/// The found card for that route.
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

/// The manual form for an account used for its calendar and contacts alone: the login, the
/// calendar server, the address-book server, or both, and the password.
pub(super) fn manual_fields(
    content: &gtk::Box,
    window: &gtk::Window,
    form: &ManualForm,
    error: Option<&str>,
    certificate: Option<&RejectedCertificate>,
    required: bool,
    sender: &relm4::Sender<AppInput>,
) -> (FormSnapshot, ConnectPane) {
    content.append(&body(l10n::setup_dav_note()));
    content.append(&caption(l10n::setup_credentials_note()));
    let login = entry(l10n::setup_field_username(), &form.email, false);
    content.append(&login);
    let password = entry(l10n::setup_field_password(), "", true);
    content.append(&password);
    let caldav = entry(l10n::setup_field_caldav_optional(), &form.caldav_url, false);
    content.append(&caldav);
    let carddav = entry(
        l10n::setup_field_carddav_optional(),
        &form.carddav_url,
        false,
    );
    content.append(&carddav);
    let feedback = gtk::Box::new(gtk::Orientation::Vertical, 14);
    content.append(&feedback);

    let snapshot: FormSnapshot = {
        let base = form.clone();
        let (login, caldav, carddav) = (login.clone(), caldav.clone(), carddav.clone());
        Rc::new(move || ManualForm {
            email: login.text().trim().to_owned(),
            caldav_url: caldav.text().trim().to_owned(),
            carddav_url: carddav.text().trim().to_owned(),
            ..base.clone()
        })
    };

    let actions = actions(window, required, sender);
    let connect = primary(l10n::action_connect(), window);
    let readout = connecting_readout(l10n::status_connecting());
    actions.append(&readout);
    let gate = gate_connect(&connect, None, true, &password);
    let pane = ConnectPane::new(&feedback, gate, &connect, &readout, true);
    pane.show_result(error, certificate);
    let refused = pane.accepted();
    let input = sender.clone();
    connect.connect_clicked(move |_| {
        let (caldav_url, carddav_url) = (
            caldav.text().trim().to_owned(),
            carddav.text().trim().to_owned(),
        );
        let login = login.text().trim().to_owned();
        let Some(uses) = dav_uses(&caldav_url, &carddav_url) else {
            return;
        };
        if login.is_empty() || password.text().is_empty() {
            return;
        }
        input.emit(AppInput::SubmitAccount(Box::new(AccountSubmission::Imap(
            ImapSubmission {
                email: login,
                imap_host: String::new(),
                smtp_host: String::new(),
                caldav_url,
                carddav_url,
                uses: Some(uses),
                imap_security: ConnectionSecurity::ImplicitTls,
                smtp_security: ConnectionSecurity::ImplicitTls,
                password: password.text().to_string(),
                accepted_certificate: refused.borrow().clone(),
            },
        ))));
    });
    actions.append(&connect);
    content.append(&actions);
    (snapshot, pane)
}

/// What the servers typed make the account used for: its calendar beside a calendar server,
/// and contacts beside either, since they are looked for at the calendar's server when no
/// address book is given. `None` when neither is typed, which is no account.
fn dav_uses(caldav: &str, carddav: &str) -> Option<Vec<AccountCapability>> {
    match (caldav.is_empty(), carddav.is_empty()) {
        (true, true) => None,
        (false, _) => Some(vec![
            AccountCapability::Calendar,
            AccountCapability::Contacts,
        ]),
        (true, false) => Some(vec![AccountCapability::Contacts]),
    }
}

#[cfg(test)]
mod tests {
    use mailcal_bindings::AccountCapability::{Calendar, Contacts};

    use super::dav_uses;

    #[test]
    fn the_servers_typed_decide_what_the_account_is_used_for() {
        assert_eq!(
            dav_uses("cloud.example", ""),
            Some(vec![Calendar, Contacts])
        );
        assert_eq!(
            dav_uses("cloud.example", "book.example"),
            Some(vec![Calendar, Contacts])
        );
        assert_eq!(dav_uses("", "book.example"), Some(vec![Contacts]));
        assert_eq!(dav_uses("", ""), None);
    }
}
