//! The IMAP routes: a detected card the user confirms with a password, and the manual form.

use std::rc::Rc;

use adw::prelude::*;
use mailcal_bindings::ConnectionSecurity;
use url::Url;

use super::{
    AppInput,
    setup_manual::FormSnapshot,
    setup_model::{AccountSubmission, DetectedServer, ImapForm, ImapSubmission, ManualForm},
    setup_widgets::{
        actions, caption, detected_row, edit_manually_button, entry, gate_on_trust, primary,
        section, show_error, trust_approved, trust_gate,
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
    show_error(content, error);

    let actions = actions(window, required, sender);
    actions.append(&edit_manually_button(sender));
    let connect = primary(l10n::action_connect(), window);
    gate_on_trust(&trust, &connect, form.trusted);
    let base = form.clone();
    let input = sender.clone();
    let dialog = window.clone();
    connect.connect_clicked(move |_| {
        if !trust_approved(base.trusted, trust.is_active()) || password.text().is_empty() {
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
    required: bool,
    sender: &relm4::Sender<AppInput>,
) -> FormSnapshot {
    content.append(&caption(l10n::setup_credentials_note()));
    let email = entry(l10n::setup_field_email(), &form.email, false);
    let imap = entry(l10n::setup_field_mail_server(), &form.imap_host, false);
    let password = entry(l10n::setup_field_password(), "", true);
    let smtp = entry(l10n::setup_field_smtp_optional(), &form.smtp_host, false);
    let caldav = entry(l10n::setup_field_caldav_optional(), &form.caldav_url, false);
    for field in [&email, &imap, &password, &smtp, &caldav] {
        content.append(field);
    }
    content.append(&caption(l10n::setup_port_note()));
    show_error(content, error);

    let snapshot: FormSnapshot = {
        let base = form.clone();
        let (email, imap, smtp, caldav) =
            (email.clone(), imap.clone(), smtp.clone(), caldav.clone());
        Rc::new(move || ManualForm {
            email: email.text().trim().to_owned(),
            imap_host: imap.text().trim().to_owned(),
            smtp_host: smtp.text().trim().to_owned(),
            caldav_url: caldav.text().trim().to_owned(),
            ..base.clone()
        })
    };

    let actions = actions(window, required, sender);
    let connect = primary(l10n::action_connect(), window);
    let input = sender.clone();
    let dialog = window.clone();
    connect.connect_clicked(move |_| {
        let submission = ImapSubmission {
            email: email.text().trim().to_owned(),
            imap_host: imap.text().trim().to_owned(),
            smtp_host: smtp.text().trim().to_owned(),
            caldav_url: caldav.text().trim().to_owned(),
            // The manual form is implicit-TLS only; a STARTTLS server arrives through
            // autodetection (docs/account-autodetect.md → Known gaps).
            imap_security: ConnectionSecurity::ImplicitTls,
            smtp_security: ConnectionSecurity::ImplicitTls,
            password: password.text().to_string(),
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
