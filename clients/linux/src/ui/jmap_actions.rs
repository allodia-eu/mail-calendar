//! The model's JMAP actions: autodiscovery, and the discoverable-OAuth sign-in flow.
//!
//! Separate from [`super::jmap`], which is the protocol half: the loopback handoff and secure
//! storage. This file is the half that reaches the model, in the shape
//! [`super::calendar_actions`] uses for the calendar.

use std::time::Duration;

use super::{
    AppInput, AppModel,
    jmap::{self, JmapOutcome, JmapPrepared, JmapReauthOutcome, JmapReauthPrepared},
    oauth_loopback::CallbackOutcome,
    setup_model::{DetectedForm, ManualForm, SetupForm, recommendation_form},
};

/// How long the detected card waits for the pre-flight before offering the secret field
/// instead. The discovery chain has no overall timeout of its own, and this card shows nothing
/// to act on while it asks; so a server that never answers must not be able to hold the user
/// there. Fastmail, the shape this exists for, answers in under two seconds.
const PROBE_DEADLINE: Duration = Duration::from_secs(6);

/// The answer that means "use the secret field": what a failure, a missing core, and a silent
/// server all come to.
fn unavailable(email: String, server_url: String) -> AppInput {
    AppInput::JmapOAuthAvailable {
        email,
        server_url,
        available: false,
    }
}

/// Races the probe. Whichever answer lands first wins: `SetupState::jmap_oauth_available`
/// takes only the first for an address; so a late real answer cannot reopen a decided card.
fn deny_after_deadline(email: &str, server_url: &str, sender: &relm4::Sender<AppInput>) {
    let (email, server_url) = (email.to_owned(), server_url.to_owned());
    let sender = sender.clone();
    gtk::glib::timeout_add_local_once(PROBE_DEADLINE, move || {
        sender.emit(unavailable(email, server_url));
    });
}

impl AppModel {
    pub(super) fn account_detected(
        &mut self,
        fallback_email: String,
        recommendation: mailcal_bindings::SetupRecommendation,
        sender: relm4::Sender<AppInput>,
    ) {
        let form = recommendation_form(recommendation, fallback_email);
        let probe = match &form {
            SetupForm::Detected(DetectedForm::Jmap(form)) => {
                Some((form.email.clone(), form.server_url.clone()))
            }
            _ => None,
        };
        self.setup.show_form(form);
        if let Some((email, server_url)) = probe {
            self.probe_jmap_sign_in(email, server_url, sender);
        }
    }

    pub(super) fn jmap_oauth_available(&mut self, email: &str, server_url: &str, available: bool) {
        self.setup
            .jmap_oauth_available(email, server_url, available);
    }

    /// The manual form the user switched account type on, or typed a new address into. Both
    /// answer with the form to pre-flight, or `None` when there is nothing new to ask.
    pub(super) fn select_account_kind(
        &mut self,
        form: ManualForm,
        sender: relm4::Sender<AppInput>,
    ) {
        let probe = self.setup.select_account_kind(form);
        self.probe_manual(probe, sender);
    }

    pub(super) fn probe_manual_jmap_sign_in(
        &mut self,
        form: ManualForm,
        sender: relm4::Sender<AppInput>,
    ) {
        let probe = self.setup.adopt_manual_jmap(form);
        self.probe_manual(probe, sender);
    }

    pub(super) fn edit_detected_manually(&mut self, sender: relm4::Sender<AppInput>) {
        let probe = self.setup.edit_detected_manually();
        self.probe_manual(probe, sender);
    }

    fn probe_manual(&mut self, probe: Option<ManualForm>, sender: relm4::Sender<AppInput>) {
        if let Some(form) = probe {
            self.probe_jmap_sign_in(form.email, form.jmap_server, sender);
        }
    }

    /// Asks the core whether this server advertises sign-in at all. Blocking, and fail-soft:
    /// any failure is a `false`, which is the secret field.
    fn probe_jmap_sign_in(
        &mut self,
        email: String,
        server_url: String,
        sender: relm4::Sender<AppInput>,
    ) {
        let Some(app) = self.app.clone() else {
            // With no core there is nothing to ask, and the card must not wait for an answer
            // that can never come.
            sender.emit(unavailable(email, server_url));
            return;
        };
        deny_after_deadline(&email, &server_url, &sender);
        std::thread::spawn(move || {
            let available = app.jmap_oauth_available(
                email.clone(),
                (!server_url.trim().is_empty()).then_some(server_url.clone()),
            );
            sender.emit(AppInput::JmapOAuthAvailable {
                email,
                server_url,
                available,
            });
        });
    }

    pub(super) fn start_jmap_login(
        &mut self,
        email: String,
        server_url: String,
        sender: relm4::Sender<AppInput>,
    ) {
        let (Some(app), Some(_)) = (self.app.clone(), self.secrets.clone()) else {
            return;
        };
        let Ok(loopback) = self.host_tasks.jmap_loopback() else {
            log::warn!("jmap sign-in loopback bind failed");
            self.setup.jmap_sign_in_failed();
            return;
        };
        let (attempt, _) = self.host_tasks.jmap.start();
        self.setup.jmap_signing_in();
        std::thread::spawn(move || {
            let prepared = jmap::prepare(&app, loopback, email, server_url).map(Box::new);
            sender.emit(AppInput::JmapPrepared(attempt, prepared));
        });
    }

    pub(super) fn jmap_prepared(
        &mut self,
        attempt: u64,
        prepared: Result<Box<JmapPrepared>, String>,
        sender: relm4::Sender<AppInput>,
    ) {
        if !self.host_tasks.jmap.holds(attempt) {
            return;
        }
        let Ok(prepared) = prepared else {
            log::warn!("jmap sign-in preparation failed");
            self.host_tasks.jmap.finish(attempt);
            self.setup.jmap_sign_in_failed();
            return;
        };
        let JmapPrepared {
            authorization_url,
            pending,
            expected_state,
            loopback,
        } = *prepared;
        let failed = sender.clone();
        jmap::launch_browser(&authorization_url, move || {
            failed.emit(AppInput::JmapFinished(attempt, JmapOutcome::Failed));
        });
        let (Some(app), Some(cancel)) =
            (self.app.clone(), self.host_tasks.jmap.cancel_token(attempt))
        else {
            self.host_tasks.jmap.finish(attempt);
            self.setup.jmap_sign_in_failed();
            return;
        };
        std::thread::spawn(move || {
            let outcome = match jmap::wait(loopback, &cancel, &expected_state) {
                CallbackOutcome::Received(callback_url) => {
                    sender.emit(AppInput::JmapCallbackReceived(attempt));
                    jmap::complete(&app, pending, callback_url)
                }
                CallbackOutcome::Cancelled => JmapOutcome::Cancelled,
                CallbackOutcome::Failed(_) => JmapOutcome::Failed,
            };
            sender.emit(AppInput::JmapFinished(attempt, outcome));
        });
    }

    pub(super) fn cancel_jmap_login(&mut self) {
        if self.host_tasks.jmap.cancel() {
            self.setup.retry_form();
        }
    }

    pub(super) fn jmap_callback_received(&mut self, attempt: u64) {
        if self.host_tasks.jmap.holds(attempt) {
            self.setup.connecting();
        }
    }

    pub(super) fn jmap_finished(
        &mut self,
        attempt: u64,
        outcome: JmapOutcome,
        sender: relm4::Sender<AppInput>,
    ) {
        if !self.host_tasks.jmap.finish(attempt) {
            // Cancelling releases the slot immediately, but an exchange already in flight still
            // stores the account; adopt it instead of leaving the mailbox stale.
            if matches!(outcome, JmapOutcome::Added(_))
                && let Some(app) = self.app.clone()
            {
                self.snapshot = app.mailbox_list();
                self.sync_after_account_change(sender);
            }
            return;
        }
        match outcome {
            JmapOutcome::Added(account) => self.account_signed_in(account, sender),
            JmapOutcome::Cancelled => self.setup.retry_form(),
            JmapOutcome::Failed => {
                log::warn!("jmap sign-in failed");
                self.setup.jmap_sign_in_failed();
            }
        }
    }

    pub(super) fn start_jmap_reauth(
        &mut self,
        account_id: String,
        sender: relm4::Sender<AppInput>,
    ) {
        let Some(app) = self.app.clone() else {
            return;
        };
        let (attempt, _) = self.host_tasks.jmap.start();
        self.notice = None;
        std::thread::spawn(move || {
            let prepared = jmap::prepare_reauth(&app, account_id).map(Box::new);
            sender.emit(AppInput::JmapReauthPrepared(attempt, prepared));
        });
    }

    pub(super) fn jmap_reauth_prepared(
        &mut self,
        attempt: u64,
        prepared: Result<Box<JmapReauthPrepared>, String>,
        sender: relm4::Sender<AppInput>,
    ) {
        if !self.host_tasks.jmap.holds(attempt) {
            return;
        }
        let Ok(prepared) = prepared else {
            self.jmap_reauth_failed(attempt);
            return;
        };
        let JmapReauthPrepared {
            account_id,
            authorization_url,
            pending,
            expected_state,
            redirect_uri,
        } = *prepared;
        let Ok(loopback) = self.host_tasks.jmap_loopback_for_redirect(&redirect_uri) else {
            log::warn!("jmap re-authentication loopback bind failed");
            self.jmap_reauth_failed(attempt);
            return;
        };
        let failed = sender.clone();
        jmap::launch_browser(&authorization_url, move || {
            failed.emit(AppInput::JmapReauthFinished(
                attempt,
                JmapReauthOutcome::Failed,
            ));
        });
        let (Some(app), Some(cancel)) =
            (self.app.clone(), self.host_tasks.jmap.cancel_token(attempt))
        else {
            self.jmap_reauth_failed(attempt);
            return;
        };
        std::thread::spawn(move || {
            let outcome = match jmap::wait(loopback, &cancel, &expected_state) {
                CallbackOutcome::Received(callback_url) => {
                    jmap::complete_reauth(&app, account_id, pending, callback_url)
                }
                CallbackOutcome::Cancelled => JmapReauthOutcome::Cancelled,
                CallbackOutcome::Failed(_) => JmapReauthOutcome::Failed,
            };
            sender.emit(AppInput::JmapReauthFinished(attempt, outcome));
        });
    }

    pub(super) fn jmap_reauth_finished(&mut self, attempt: u64, outcome: JmapReauthOutcome) {
        if !self.host_tasks.jmap.finish(attempt) {
            return;
        }
        match outcome {
            JmapReauthOutcome::Reauthenticated => {
                self.notice = None;
                if let Some(app) = &self.app {
                    self.snapshot = app.mailbox_list();
                    self.connectivity =
                        super::connectivity::ConnectivityState::pull(app, &self.snapshot.accounts);
                }
            }
            JmapReauthOutcome::Cancelled => {}
            JmapReauthOutcome::Failed => {
                log::warn!("jmap re-authentication failed");
                self.notice = Some(crate::l10n::signin_expired_failed().to_owned());
            }
        }
    }

    fn jmap_reauth_failed(&mut self, attempt: u64) {
        self.host_tasks.jmap.finish(attempt);
        self.notice = Some(crate::l10n::signin_expired_failed().to_owned());
    }
}
