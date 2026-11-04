//! The calendar-and-contacts route: detection found a calendar or address-book server for the
//! address and no mail server, so the account is set up for those alone, with the address as its
//! login (`docs/account-autodetect.md` rule 8).

use std::rc::Rc;

use adw::prelude::*;
use mailcal_bindings::{AccountCapability, ConnectionSecurity, RejectedCertificate};

use super::{
    AppInput, setup_imap,
    setup_manual::FormSnapshot,
    setup_model::{AccountSubmission, DavForm, ImapSubmission, ManualForm},
    setup_nextcloud::SetupNextcloudInput,
    setup_pane::{ConnectPane, connecting_readout, feedback_box},
    setup_uses::{self, ChosenUses},
    setup_widgets::{actions, body, caption, edit_manually_button, entry, gate_connect, primary},
};
use crate::l10n;

/// The found card for that route. At a Nextcloud server it leads with Nextcloud's own sign-in,
/// under the line that explains it, and the password waits behind "Use a password instead"
/// (`docs/nextcloud-sign-in.md` rule 2).
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
    let sign_in = form.nextcloud.then(|| {
        content.append(&body(l10n::nextcloud_sign_in_note()));
        let button = primary(l10n::nextcloud_sign_in_button(), window);
        button.set_halign(gtk::Align::Start);
        let (input, chosen) = (sender.clone(), Rc::clone(&uses));
        button.connect_clicked(move |_| {
            if let Some(field) = chosen.missing_server() {
                setup_uses::flag(&field);
                return;
            }
            if let Some(start) = nextcloud_start(chosen.chosen()) {
                input.emit(AppInput::SetupNextcloud(start));
            }
        });
        content.append(&button);
        let instead = gtk::Button::with_label(l10n::setup_imap_signin_password_instead());
        instead.set_halign(gtk::Align::Start);
        content.append(&instead);
        (button, instead)
    });
    let area = gtk::Box::new(gtk::Orientation::Vertical, 14);
    area.append(&caption(l10n::setup_credentials_note()));
    let password = entry(l10n::setup_field_password(), "", true);
    area.append(&password);
    content.append(&area);
    let feedback = feedback_box(content);

    let actions = actions(window, required, sender);
    actions.append(&edit_manually_button(sender));
    let connect = match &sign_in {
        Some(_) => gtk::Button::with_label(l10n::action_connect()),
        None => primary(l10n::action_connect(), window),
    };
    let readout = connecting_readout(l10n::status_connecting());
    if let Some((button, instead)) = &sign_in {
        // Connect sits in the field's own row, so it is on screen exactly when the field is.
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        row.set_halign(gtk::Align::End);
        row.append(&readout);
        row.append(&connect);
        area.append(&row);
        setup_imap::password_behind(window, instead, button, &area, &connect, &password);
    } else {
        actions.append(&readout);
        actions.append(&connect);
    }
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
        if let Some(field) = uses.missing_server() {
            setup_uses::flag(&field);
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
    content.append(&actions);
    pane
}

/// The sign-in with Nextcloud the found card's choices start: at the calendar's server, else the
/// address book's, for the uses chosen, or those the servers decide when the card offered no
/// choice.
fn nextcloud_start(chosen: ChosenUses) -> Option<SetupNextcloudInput> {
    let server = [&chosen.caldav_url, &chosen.carddav_url]
        .into_iter()
        .find(|url| !url.is_empty())?
        .clone();
    let uses = chosen
        .uses
        .or_else(|| dav_uses(&chosen.caldav_url, &chosen.carddav_url))?;
    Some(SetupNextcloudInput::Start { server, uses })
}

/// The screen shown while the browser holds a sign-in with Nextcloud, with the one action left:
/// cancel.
pub(super) fn nextcloud_signing_in(sender: &relm4::Sender<AppInput>) -> gtk::Box {
    super::setup_widgets::waiting(
        l10n::nextcloud_sign_in_waiting(),
        || AppInput::SetupNextcloud(SetupNextcloudInput::Cancel),
        sender,
    )
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
    // Above the fields, which stay as they are: the server typed answered as Nextcloud.
    let sign_in = form.nextcloud.as_ref().map(|server| {
        content.append(&body(l10n::nextcloud_sign_in_note()));
        let button = gtk::Button::with_label(l10n::nextcloud_sign_in_button());
        button.set_halign(gtk::Align::Start);
        content.append(&button);
        (button, server.clone())
    });
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

    for field in [&caldav, &carddav] {
        ask_on_leave(field, &snapshot, sender);
    }
    if let Some((button, server)) = sign_in {
        let (input, caldav, carddav) = (sender.clone(), caldav.clone(), carddav.clone());
        button.connect_clicked(move |_| {
            let (caldav, carddav) = (caldav.text(), carddav.text());
            if let Some(uses) = dav_uses(caldav.trim(), carddav.trim()) {
                input.emit(AppInput::SetupNextcloud(SetupNextcloudInput::Start {
                    server: server.clone(),
                    uses,
                }));
            }
        });
    }

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
        if login.text().trim().is_empty() {
            setup_uses::flag(&login);
            return;
        }
        let Some(uses) = dav_uses(&caldav_url, &carddav_url) else {
            setup_uses::flag(&caldav);
            return;
        };
        if password.text().is_empty() {
            return;
        }
        let login = login.text().trim().to_owned();
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

/// Asks whether the server typed is Nextcloud whenever the person leaves a server field.
fn ask_on_leave(field: &gtk::Entry, snapshot: &FormSnapshot, sender: &relm4::Sender<AppInput>) {
    let focus = gtk::EventControllerFocus::new();
    let (snapshot, input) = (Rc::clone(snapshot), sender.clone());
    focus.connect_leave(move |_| {
        input.emit(AppInput::SetupNextcloud(SetupNextcloudInput::Ask(
            Box::new(snapshot()),
        )));
    });
    field.add_controller(focus);
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
