//! The model's IMAP sign-in actions: the pre-flight that decides what the card asks for, and
//! the browser flow when it asks for a sign-in.
//!
//! The shape is [`super::jmap_actions`]'s, because the flow is the same one. The pre-flight
//! differs in what it asks: JMAP's answers "does this server advertise sign-in", while this
//! one answers "what will this server accept", which has three answers rather than two.

use std::time::Duration;

use super::{
    AppInput, AppModel,
    imap_signin::{self, ImapOutcome, ImapPrepared},
    oauth_loopback::CallbackOutcome,
    setup_model::ImapForm,
};

/// How long the detected card waits for the pre-flight before falling back to the password
/// field.
///
/// The card shows nothing to act on while it asks, so a server that never answers must not be
/// able to hold the user there. Longer than the JMAP deadline because this one dials the mail
/// server before it fetches anything: a TLS handshake to a slow host, then up to a few
/// metadata requests.
const PROBE_DEADLINE: Duration = Duration::from_secs(10);

impl AppModel {
    /// Asks the core what this server accepts. Blocking, and fail-soft: any failure is the
    /// password field, which works everywhere.
    pub(super) fn probe_imap_sign_in(&mut self, form: &ImapForm, sender: relm4::Sender<AppInput>) {
        let Some(app) = self.app.clone() else {
            // With no core there is nothing to ask, and the card must not wait for an answer
            // that can never come.
            sender.emit(password_only(form));
            return;
        };
        deny_after_deadline(form, &sender);
        let request = imap_signin::login_request(form);
        let (email, host) = (form.email.clone(), form.imap_host.clone());
        std::thread::spawn(move || {
            let offer = app.imap_auth_options(request);
            sender.emit(AppInput::ImapAuthAnswered {
                email,
                imap_host: host,
                offer: Box::new(offer),
            });
        });
    }

    /// The manual pane's server field was left. Asks again when what is typed is new.
    pub(super) fn probe_manual_imap_sign_in(
        &mut self,
        form: crate::ui::setup_model::ManualForm,
        sender: relm4::Sender<AppInput>,
    ) {
        if let Some(form) = self.setup.adopt_manual_imap(form) {
            self.probe_imap_sign_in(&form.into(), sender);
        }
    }

    pub(super) fn imap_auth_answered(
        &mut self,
        email: &str,
        imap_host: &str,
        offer: mailcal_bindings::ImapAuthOffer,
    ) {
        self.setup.imap_auth_answered(email, imap_host, offer);
    }

    pub(super) fn start_imap_login(&mut self, form: &ImapForm, sender: relm4::Sender<AppInput>) {
        let (Some(app), Some(_)) = (self.app.clone(), self.secrets.clone()) else {
            return;
        };
        let Ok(loopback) = self.host_tasks.oauth_loopback() else {
            log::warn!("imap sign-in loopback bind failed");
            self.setup.imap_sign_in_failed();
            return;
        };
        let (attempt, _) = self.host_tasks.imap.start();
        self.setup.imap_signing_in();
        let request = imap_signin::login_request(form);
        std::thread::spawn(move || {
            let prepared = imap_signin::prepare(&app, loopback, request).map(Box::new);
            sender.emit(AppInput::ImapPrepared(attempt, prepared));
        });
    }

    pub(super) fn imap_prepared(
        &mut self,
        attempt: u64,
        prepared: Result<Box<ImapPrepared>, String>,
        sender: relm4::Sender<AppInput>,
    ) {
        if !self.host_tasks.imap.holds(attempt) {
            return;
        }
        let Ok(prepared) = prepared else {
            log::warn!("imap sign-in preparation failed");
            self.host_tasks.imap.finish(attempt);
            self.setup.imap_sign_in_failed();
            return;
        };
        let ImapPrepared {
            authorization_url,
            pending,
            expected_state,
            loopback,
        } = *prepared;
        let failed = sender.clone();
        imap_signin::launch_browser(&authorization_url, move || {
            failed.emit(AppInput::ImapFinished(attempt, ImapOutcome::Failed));
        });
        let (Some(app), Some(cancel)) =
            (self.app.clone(), self.host_tasks.imap.cancel_token(attempt))
        else {
            self.host_tasks.imap.finish(attempt);
            self.setup.imap_sign_in_failed();
            return;
        };
        std::thread::spawn(move || {
            let outcome = match imap_signin::wait(loopback, &cancel, &expected_state) {
                CallbackOutcome::Received(callback_url) => {
                    sender.emit(AppInput::ImapCallbackReceived(attempt));
                    imap_signin::complete(&app, pending, callback_url)
                }
                CallbackOutcome::Cancelled => ImapOutcome::Cancelled,
                CallbackOutcome::Failed(_) => ImapOutcome::Failed,
            };
            sender.emit(AppInput::ImapFinished(attempt, outcome));
        });
    }

    pub(super) fn cancel_imap_login(&mut self) {
        if self.host_tasks.imap.cancel() {
            self.setup.retry_form();
        }
    }

    pub(super) fn imap_callback_received(&mut self, attempt: u64) {
        if self.host_tasks.imap.holds(attempt) {
            self.setup.connecting();
        }
    }

    pub(super) fn imap_finished(
        &mut self,
        attempt: u64,
        outcome: ImapOutcome,
        sender: relm4::Sender<AppInput>,
    ) {
        if !self.host_tasks.imap.finish(attempt) {
            // Cancelling releases the slot immediately, but an exchange already in flight
            // still stores the account; adopt it rather than leaving the mailbox stale.
            if matches!(outcome, ImapOutcome::Added(_))
                && let Some(app) = self.app.clone()
            {
                self.snapshot = app.mailbox_list();
                self.sync_after_account_change(sender);
            }
            return;
        }
        match outcome {
            ImapOutcome::Added(account) => self.account_signed_in(account, sender),
            ImapOutcome::Cancelled => self.setup.retry_form(),
            ImapOutcome::Failed => {
                log::warn!("imap sign-in failed");
                self.setup.imap_sign_in_failed();
            }
        }
    }
}

/// The answer that means "ask for a password": what a failure, a missing core and a silent
/// server all come to.
fn password_only(form: &ImapForm) -> AppInput {
    AppInput::ImapAuthAnswered {
        email: form.email.clone(),
        imap_host: form.imap_host.clone(),
        offer: Box::new(mailcal_bindings::ImapAuthOffer::Password),
    }
}

/// Races the probe. Whichever answer lands first wins: the setup state takes only the first
/// for a given server, so a late real answer cannot reopen a decided card.
fn deny_after_deadline(form: &ImapForm, sender: &relm4::Sender<AppInput>) {
    let answer = password_only(form);
    let sender = sender.clone();
    gtk::glib::timeout_add_local_once(PROBE_DEADLINE, move || {
        sender.emit(answer);
    });
}
