//! The two provider browser sign-ins, Google and Microsoft, as the model sees them.
//!
//! Both are the same four steps; bind the loopback the redirect will come back to, open the
//! system browser, wait for the callback off the main thread, hand it to the core; so they
//! live together and differ only in which adapter ([`super::google`], [`super::microsoft`])
//! and which sign-in phase they drive. The JMAP flow is [`super::jmap_actions`]: it discovers
//! and registers before it can begin, which is a step neither of these has.

use super::{
    AppInput, AppModel,
    google::{self, GoogleOutcome},
    microsoft::{self, MicrosoftOutcome},
    oauth_loopback::CallbackOutcome,
};
use crate::l10n;

impl AppModel {
    pub(super) fn start_google_login(&mut self, email: String, sender: relm4::Sender<AppInput>) {
        let Some(app) = self.app.clone() else {
            return;
        };
        let (attempt, cancel) = self.host_tasks.google.start();
        let (loopback, start) = match google::begin(email) {
            Ok(start) => start,
            Err(error) => {
                self.host_tasks.google.finish(attempt);
                self.setup.failed(l10n::status_connect_failed(&error));
                return;
            }
        };
        self.setup.google_signing_in();
        let failed = sender.clone();
        google::launch_browser(&start.authorization_url, move |error| {
            failed.emit(AppInput::GoogleFinished(
                attempt,
                GoogleOutcome::Failed(error),
            ));
        });
        std::thread::spawn(move || {
            let outcome = match google::wait(loopback, &cancel) {
                CallbackOutcome::Received(callback_url) => {
                    sender.emit(AppInput::GoogleCallbackReceived(attempt));
                    google::complete(&app, start.pending, callback_url)
                }
                CallbackOutcome::Cancelled => GoogleOutcome::Cancelled,
                CallbackOutcome::Failed(error) => GoogleOutcome::Failed(error),
            };
            sender.emit(AppInput::GoogleFinished(attempt, outcome));
        });
    }

    pub(super) fn cancel_google_login(&mut self) {
        if self.host_tasks.google.cancel() {
            self.setup.retry_form();
        }
    }

    pub(super) fn google_callback_received(&mut self, attempt: u64) {
        if self.host_tasks.google.holds(attempt) {
            self.setup.connecting();
        }
    }

    pub(super) fn google_finished(
        &mut self,
        attempt: u64,
        outcome: GoogleOutcome,
        sender: relm4::Sender<AppInput>,
    ) {
        if !self.host_tasks.google.finish(attempt) {
            // A cancelled attempt whose exchange had already started still stores the account,
            // so pick it up rather than leaving the mailbox showing one account too few.
            self.adopt_abandoned(matches!(outcome, GoogleOutcome::Added(_)), sender);
            return;
        }
        match outcome {
            GoogleOutcome::Added(account) => self.account_signed_in(account, sender),
            GoogleOutcome::Cancelled => self.setup.retry_form(),
            GoogleOutcome::Failed(error) => {
                self.setup.failed(l10n::status_connect_failed(&error));
            }
        }
    }

    pub(super) fn start_microsoft_login(&mut self, email: String, sender: relm4::Sender<AppInput>) {
        let Some(app) = self.app.clone() else {
            return;
        };
        let (attempt, cancel) = self.host_tasks.microsoft.start();
        let (loopback, start) = match microsoft::begin(email) {
            Ok(start) => start,
            Err(error) => {
                self.host_tasks.microsoft.finish(attempt);
                self.setup.failed(l10n::status_connect_failed(&error));
                return;
            }
        };
        self.setup.microsoft_signing_in();
        let failed = sender.clone();
        microsoft::launch_browser(&start.authorization_url, move |error| {
            failed.emit(AppInput::MicrosoftFinished(
                attempt,
                MicrosoftOutcome::Failed(error),
            ));
        });
        std::thread::spawn(move || {
            let outcome = match microsoft::wait(loopback, &cancel) {
                CallbackOutcome::Received(callback_url) => {
                    sender.emit(AppInput::MicrosoftCallbackReceived(attempt));
                    microsoft::complete(&app, start.pending, callback_url)
                }
                CallbackOutcome::Cancelled => MicrosoftOutcome::Cancelled,
                CallbackOutcome::Failed(error) => MicrosoftOutcome::Failed(error),
            };
            sender.emit(AppInput::MicrosoftFinished(attempt, outcome));
        });
    }

    pub(super) fn cancel_microsoft_login(&mut self) {
        if self.host_tasks.microsoft.cancel() {
            self.setup.retry_form();
        }
    }

    pub(super) fn microsoft_callback_received(&mut self, attempt: u64) {
        if self.host_tasks.microsoft.holds(attempt) {
            self.setup.connecting();
        }
    }

    pub(super) fn microsoft_finished(
        &mut self,
        attempt: u64,
        outcome: MicrosoftOutcome,
        sender: relm4::Sender<AppInput>,
    ) {
        if !self.host_tasks.microsoft.finish(attempt) {
            self.adopt_abandoned(matches!(outcome, MicrosoftOutcome::Added(_)), sender);
            return;
        }
        match outcome {
            MicrosoftOutcome::Added(account) => self.account_signed_in(account, sender),
            MicrosoftOutcome::Cancelled => self.setup.retry_form(),
            MicrosoftOutcome::Failed(error) => {
                self.setup.failed(l10n::status_connect_failed(&error));
            }
        }
    }

    /// A completed sign-in: close setup, select the new account, and repaint from the core.
    ///
    /// Every provider route ends here; Google, Microsoft and JMAP; which is why the account
    /// list's own bookkeeping belongs here rather than at each of them. Without it an account
    /// added by a provider sign-in reaches the person's other devices only at the next launch,
    /// and Settings draws no sharing control for it until then.
    pub(super) fn account_signed_in(&mut self, account: String, sender: relm4::Sender<AppInput>) {
        self.setup.complete();
        self.dispatch(mailcal_bindings::Intent::SelectAccount {
            account: Some(account),
        });
        if let Some(app) = &self.app {
            self.snapshot = app.mailbox_list();
        }
        self.sync_after_account_change(sender);
    }

    /// A sign-in the user cancelled after the exchange had already stored the account. Nothing
    /// is on screen to update, but the mailbox would otherwise show one account too few; and the
    /// account is stored, so it travels like any other.
    fn adopt_abandoned(&mut self, added: bool, sender: relm4::Sender<AppInput>) {
        if added && let Some(app) = self.app.clone() {
            self.snapshot = app.mailbox_list();
            self.sync_after_account_change(sender);
        }
    }
}
