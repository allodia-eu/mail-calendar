//! The IMAP routes: a detected card the user confirms with a password, and the manual form.

use std::rc::Rc;

use adw::prelude::*;
use mailcal_bindings::RejectedCertificate;
use url::Url;

use super::{
    AppInput,
    setup_manual::FormSnapshot,
    setup_model::{AccountSubmission, DetectedServer, ImapForm, ImapSubmission, ManualForm},
    setup_server_field::ServerPair,
    setup_server_row::server_row,
    setup_widgets::{
        actions, caption, certificate_accepted, certificate_gate, detected_row,
        edit_manually_button, entry, gate_connect, primary, section, show_error, trust_approved,
        trust_gate,
    },
};
use crate::l10n;

/// The detected card: the servers detection found, shown to be recognised rather than retyped,
/// plus the one thing only the user has; the password; and the calendar it discovered.
pub(super) fn detected_fields(
    content: &gtk::Box,
    window: &gtk::Window,
    form: &ImapForm,
    error: Option<&str>,
    certificate: Option<&RejectedCertificate>,
    required: bool,
    sender: &relm4::Sender<AppInput>,
) {
    content.append(&section(l10n::setup_detect_section_email()));
    content.append(&server_row(&form.incoming));
    if let Some(outgoing) = &form.outgoing {
        content.append(&server_row(outgoing));
    }
    content.append(&caption(l10n::setup_detect_app_password_hint()));
    content.append(&caption(l10n::setup_credentials_note()));
    let trust = trust_gate(content, form.trusted);
    let password = entry(l10n::setup_field_password(), "", true);
    content.append(&password);

    let calendar = calendar_section(content, &form.caldav_url);
    // The panel says what the transport said, in the reader's language and with the certificate
    // beside it, so the raw message is not shown as well.
    let accepted = certificate_gate(content, certificate);
    show_error(content, error.filter(|_| certificate.is_none()));

    let actions = actions(window, required, sender);
    actions.append(&edit_manually_button(sender));
    let connect = primary(l10n::action_connect(), window);
    gate_connect(&connect, Some(&trust), form.trusted, accepted.as_ref());
    let base = form.clone();
    let refused = certificate.cloned();
    let input = sender.clone();
    let dialog = window.clone();
    connect.connect_clicked(move |_| {
        if !trust_approved(base.trusted, trust.is_active())
            || !certificate_accepted(accepted.as_ref())
            || password.text().is_empty()
        {
            return;
        }
        input.emit(AppInput::SubmitAccount(Box::new(AccountSubmission::Imap(
            ImapSubmission {
                email: base.email.clone(),
                imap_host: base.imap_host.clone(),
                smtp_host: base.smtp_host.clone(),
                caldav_url: calendar.effective_url(),
                imap_security: base.imap_security,
                smtp_security: base.smtp_security,
                password: password.text().to_string(),
                accepted_certificate: refused.clone(),
            },
        ))));
        dialog.set_visible(false);
    });
    actions.append(&connect);
    content.append(&actions);
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
) -> FormSnapshot {
    content.append(&caption(l10n::setup_credentials_note()));
    let email = entry(l10n::setup_field_email(), &form.email, false);
    content.append(&email);
    let imap = server_row(
        content,
        l10n::setup_field_mail_server(),
        &form.imap_host,
        &form.servers.imap,
    );
    let password = entry(l10n::setup_field_password(), "", true);
    content.append(&password);
    let smtp = server_row(
        content,
        l10n::setup_field_smtp_optional(),
        &form.smtp_host,
        &form.servers.smtp,
    );
    let caldav = entry(l10n::setup_field_caldav_optional(), &form.caldav_url, false);
    content.append(&caldav);
    content.append(&caption(l10n::setup_port_note()));
    let accepted = certificate_gate(content, certificate);
    show_error(content, error.filter(|_| certificate.is_none()));

    let snapshot: FormSnapshot = {
        let base = form.clone();
        let (email, caldav) = (email.clone(), caldav.clone());
        let (imap_row, smtp_row) = (imap.clone(), smtp.clone());
        Rc::new(move || ManualForm {
            email: email.text().trim().to_owned(),
            imap_host: imap_row.host_text(),
            smtp_host: smtp_row.host_text(),
            caldav_url: caldav.text().trim().to_owned(),
            servers: ServerPair {
                imap: imap_row.read(),
                smtp: smtp_row.read(),
            },
            ..base.clone()
        })
    };

    let actions = actions(window, required, sender);
    let connect = primary(l10n::action_connect(), window);
    gate_connect(&connect, None, true, accepted.as_ref());
    let refused = certificate.cloned();
    let input = sender.clone();
    let dialog = window.clone();
    connect.connect_clicked(move |_| {
        let submission = ImapSubmission {
            email: email.text().trim().to_owned(),
            imap_host: imap.dial(),
            smtp_host: smtp.dial(),
            caldav_url: caldav.text().trim().to_owned(),
            imap_security: imap.read().security(),
            smtp_security: smtp.read().security(),
            password: password.text().to_string(),
            accepted_certificate: refused.clone(),
        };
        if submission.email.is_empty()
            || submission.imap_host.is_empty()
            || submission.password.is_empty()
            || !certificate_accepted(accepted.as_ref())
        {
            return;
        }
        input.emit(AppInput::SubmitAccount(Box::new(AccountSubmission::Imap(
            submission,
        ))));
        dialog.set_visible(false);
    });
    actions.append(&connect);
    content.append(&actions);
    snapshot
}

/// The calendar half of a detected card: pre-checked when the CalDAV follow-on probe found an
/// endpoint (opt-out, showing its host), an opt-in field when it found none. Either way the
/// calendar reuses the IMAP credentials: `docs/account-autodetect.md` rule 8.
struct CalendarChoice {
    enabled: gtk::CheckButton,
    discovered: String,
    manual: gtk::Entry,
}

impl CalendarChoice {
    fn effective_url(&self) -> String {
        effective_caldav(
            self.enabled.is_active(),
            &self.discovered,
            &self.manual.text(),
        )
    }
}

/// What a detected card stores for the calendar: nothing when it is switched off, the
/// discovered endpoint when there is one, otherwise whatever was typed in its place.
fn effective_caldav(enabled: bool, discovered: &str, typed: &str) -> String {
    if !enabled {
        return String::new();
    }
    if discovered.is_empty() {
        typed.trim().to_owned()
    } else {
        discovered.to_owned()
    }
}

fn calendar_section(content: &gtk::Box, discovered: &str) -> CalendarChoice {
    content.append(&section(l10n::setup_detect_section_calendar()));
    let found = !discovered.is_empty();
    let enabled = gtk::CheckButton::with_label(if found {
        l10n::setup_detect_calendar_enable()
    } else {
        l10n::setup_detect_calendar_add()
    });
    enabled.set_active(found);
    content.append(&enabled);

    let detail = caption(&url_host(discovered));
    content.append(&detail);
    let manual = entry(l10n::setup_hint_caldav(), "", false);
    content.append(&manual);

    // Exactly one of the two belongs to this card; the discovered endpoint to confirm, or a
    // box to type one into; and it follows the toggle, so switching calendar off leaves
    // nothing behind claiming otherwise.
    let shown: gtk::Widget = if found {
        manual.set_visible(false);
        detail.clone().upcast()
    } else {
        detail.set_visible(false);
        manual.clone().upcast()
    };
    shown.set_visible(enabled.is_active());
    enabled.connect_toggled(move |choice| shown.set_visible(choice.is_active()));
    CalendarChoice {
        enabled,
        discovered: discovered.to_owned(),
        manual,
    }
}

fn server_row(row: &DetectedServer) -> gtk::Box {
    detected_row(
        &row.protocol,
        &format!("{}:{} · {}", row.hostname, row.port, row.security),
    )
}

/// The host of a discovered URL, for a line the user can eyeball; the whole URL is the fallback
/// when it does not parse.
fn url_host(url: &str) -> String {
    Url::parse(url)
        .ok()
        .and_then(|parsed| parsed.host_str().map(str::to_owned))
        .unwrap_or_else(|| url.to_owned())
}

#[cfg(test)]
mod tests {
    use super::{effective_caldav, url_host};

    #[test]
    fn a_discovered_calendar_is_opt_out_and_a_missing_one_opt_in() {
        // Found: pre-selected, and the discovered endpoint is what gets stored; never the
        // empty manual box beside it.
        assert_eq!(
            effective_caldav(true, "https://caldav.example.test/dav", ""),
            "https://caldav.example.test/dav"
        );
        // Switched off, a discovered endpoint is not stored.
        assert!(effective_caldav(false, "https://caldav.example.test/dav", "").is_empty());
        // Nothing found: whatever the user typed, trimmed.
        assert_eq!(
            effective_caldav(true, "", " https://dav.example.test "),
            "https://dav.example.test"
        );
        assert!(effective_caldav(true, "", "   ").is_empty());
    }

    #[test]
    fn a_discovered_endpoint_is_shown_by_host() {
        assert_eq!(
            url_host("https://caldav.example.test/dav/"),
            "caldav.example.test"
        );
        assert_eq!(url_host("not a url"), "not a url");
    }
}
