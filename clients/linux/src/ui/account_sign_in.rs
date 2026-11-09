//! Signing an IMAP or JMAP account in through its provider, again or instead of a password
//! (`docs/accounts.md` rule 17): the browser routes the account's page offers, each a blocking
//! core call off the main thread. The account keeps its id, uses, links, settings and mail; the
//! core replaces it in place and signals Settings, so the page redraws from the next snapshot.
//! Which routes the page offers, and a password typed there, are the core's
//! (`AccountPage::sign_in`).

use mailcal_bindings::{AccountSignInStart, MailcalApp, UnreachedUses};

use super::{
    AppInput, AppModel,
    account_consent::ConsentFrom,
    account_settings::AccountsInput,
    jmap,
    oauth_loopback::{self, CallbackOutcome},
    settings::notice::Notice,
    unreached::{self, Answer},
};
use crate::l10n;

#[path = "account_nextcloud_sign_in.rs"]
mod nextcloud;
#[path = "account_sign_in_state.rs"]
mod state;
pub(crate) use state::{SignInSwitchState, Stage};

/// Which change a sign-in makes, which decides what is said when it ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Switch {
    /// Signs an account that signs in through its provider in again.
    Again,
    /// Signs a password account in through its provider instead.
    ToProvider,
}

/// A password on its way to the core. Never printed: an input can reach a log through `Debug`.
pub(crate) struct Password(pub(crate) String);

impl std::fmt::Debug for Password {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Password(..)")
    }
}

/// One change in flight: its attempt, the account, where it was asked for and what it does.
#[derive(Debug, Clone)]
pub(crate) struct Attempt {
    id: u64,
    account: String,
    from: ConsentFrom,
    switch: Switch,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Outcome {
    Switched,
    Cancelled,
    Failed(String),
}

/// The Sign-in section's traffic, as one input of the account pages'.
#[derive(Debug)]
pub(crate) enum SignInInput {
    /// Sign `account` in through its provider, from its page.
    Browser { account: String, switch: Switch },
    /// The core started that sign-in, or could not.
    Prepared {
        attempt: Attempt,
        start: Result<Box<AccountSignInStart>, String>,
    },
    /// Drop `account`'s sign-in while the browser holds it.
    Cancel { account: String },
    /// The browser came back, and the core completes the sign-in.
    Completing { attempt: u64, account: String },
    /// The switch's question was answered.
    Answered { attempt: u64, answer: Answer },
    /// A change ended.
    Finished { attempt: Attempt, outcome: Outcome },
    /// How a sign-in with Nextcloud is getting on.
    Nextcloud {
        attempt: Attempt,
        flight: super::nextcloud_flight::Flight,
    },
    /// Settings closed, taking any page a Nextcloud grant was kept for with it.
    Closed,
    /// The core put a Nextcloud grant on the account, or did not. `spent` when the grant can
    /// never be put on it, so it is not kept for another try.
    NextcloudCompleted {
        attempt: Attempt,
        outcome: Outcome,
        spent: bool,
    },
}

fn input(input: SignInInput) -> AppInput {
    AppInput::Accounts(AccountsInput::SignIn(input))
}

fn finished(attempt: Attempt, outcome: Outcome) -> AppInput {
    input(SignInInput::Finished { attempt, outcome })
}

/// What the account's page says once a change has ended. A cancelled one says nothing: the
/// person closed the browser.
pub(super) fn settings_notice(switch: Switch, outcome: &Outcome) -> Option<Notice> {
    match outcome {
        Outcome::Switched => Some(Notice::Toast(
            match switch {
                Switch::Again => l10n::settings_account_signed_in(),
                Switch::ToProvider => l10n::settings_account_switched_to_provider(),
            }
            .to_owned(),
        )),
        Outcome::Cancelled => None,
        Outcome::Failed(detail) => Some(Notice::Error {
            title: match switch {
                Switch::Again => l10n::settings_account_signin_failed(),
                Switch::ToProvider => l10n::settings_account_switch_failed(),
            }
            .to_owned(),
            detail: detail.clone(),
        }),
    }
}

impl AppModel {
    pub(super) fn sign_in_input(&mut self, message: SignInInput, sender: relm4::Sender<AppInput>) {
        match message {
            SignInInput::Browser { account, switch } => {
                self.start_account_sign_in(account, switch, ConsentFrom::Settings, sender);
            }
            SignInInput::Prepared { attempt, start } => {
                self.account_sign_in_prepared(attempt, start, sender);
            }
            SignInInput::Answered { attempt, answer } => {
                self.account_sign_in_answered(attempt, answer, sender);
            }
            SignInInput::Cancel { account } => self.cancel_account_sign_in(&account),
            SignInInput::Completing { attempt, account } => {
                if self.host_tasks.account_sign_in.holds(attempt) {
                    self.settings.sign_in.completing(&account);
                    self.settings.refresh_in_place();
                }
            }
            SignInInput::Finished { attempt, outcome } => {
                self.account_sign_in_finished(&attempt, &outcome);
            }
            SignInInput::Nextcloud { attempt, flight } => {
                self.account_nextcloud_flight(attempt, flight, sender);
            }
            SignInInput::Closed => self.abandon_held_nextcloud_grant(),
            SignInInput::NextcloudCompleted {
                attempt,
                outcome,
                spent,
            } => self.account_nextcloud_completed(&attempt, &outcome, spent),
        }
    }

    /// Starts signing `account` in through its provider: from its page, or from the notice of
    /// an expired sign-in. A second request replaces the first.
    pub(super) fn start_account_sign_in(
        &mut self,
        account: String,
        switch: Switch,
        from: ConsentFrom,
        sender: relm4::Sender<AppInput>,
    ) {
        let Some(app) = self.app.clone() else {
            return;
        };
        if Self::signs_in_with_nextcloud(&app, &account) {
            return self.start_account_nextcloud_sign_in(account, switch, from, sender);
        }
        let (id, _) = self.host_tasks.account_sign_in.start();
        let attempt = Attempt {
            id,
            account: account.clone(),
            from,
            switch,
        };
        // A question the replaced sign-in was waiting on is no longer anyone's.
        self.settings.sign_in.question = None;
        // A password account is registered against the redirect it starts with, so the listener
        // is the one every discovered sign-in in this process shares; one that already signs in
        // keeps its grant's own, which the core names in the URL it hands back.
        let Ok(loopback) = self.host_tasks.oauth_loopback() else {
            sender.emit(finished(
                attempt,
                Outcome::Failed(l10n::account_signin_browser_failed().to_owned()),
            ));
            return;
        };
        self.settings.sign_in.begin(&account);
        self.say_waiting(&app, &account, from);
        let redirect_uri = loopback.redirect_uri();
        std::thread::spawn(move || {
            let start = app
                .begin_account_sign_in(account, redirect_uri)
                .map(Box::new)
                .map_err(|error| super::core_error::said(&error));
            sender.emit(input(SignInInput::Prepared { attempt, start }));
        });
    }

    /// Says where a sign-in asked for `from` waits: on the account's page, in the browser. From
    /// a notice, its button says it.
    fn say_waiting(&mut self, app: &MailcalApp, account: &str, from: ConsentFrom) {
        match from {
            ConsentFrom::Settings => {
                let address = app
                    .accounts_snapshot()
                    .accounts
                    .into_iter()
                    .find(|entry| entry.id == account)
                    .map_or_else(
                        || account.to_owned(),
                        |entry| super::settings::entry_name(&entry),
                    );
                self.settings
                    .notify(Notice::Toast(l10n::settings_account_signin_waiting(
                        &address,
                    )));
            }
            ConsentFrom::Notice => {}
        }
    }

    fn account_sign_in_prepared(
        &mut self,
        attempt: Attempt,
        start: Result<Box<AccountSignInStart>, String>,
        sender: relm4::Sender<AppInput>,
    ) {
        if !self.host_tasks.account_sign_in.holds(attempt.id) {
            return;
        }
        let start = match start {
            Ok(start) => start,
            Err(error) => {
                return self.account_sign_in_finished(&attempt, &Outcome::Failed(error));
            }
        };
        if unreached::asks(&start.unreached) {
            // Asked over Settings by the window, which reads it from here.
            self.settings.sign_in.question = Some((attempt, start));
            return;
        }
        self.open_sign_in_browser(attempt, start, None, sender);
    }

    /// The switch's question answered: on to the browser with the answer, or nothing changes.
    fn account_sign_in_answered(
        &mut self,
        attempt: u64,
        answer: Answer,
        sender: relm4::Sender<AppInput>,
    ) {
        let Some((asked, start)) = self
            .settings
            .sign_in
            .question
            .take_if(|(asked, _)| asked.id == attempt)
        else {
            return;
        };
        if !self.host_tasks.account_sign_in.holds(attempt) {
            return;
        }
        match answer.choice() {
            Some(choice) => self.open_sign_in_browser(asked, start, Some(choice), sender),
            None => self.account_sign_in_finished(&asked, &Outcome::Cancelled),
        }
    }

    /// Opens the browser on `start` and completes the sign-in with what came back, telling the
    /// core `unreached`, the answer to the switch's question when it asked one.
    fn open_sign_in_browser(
        &mut self,
        attempt: Attempt,
        start: Box<AccountSignInStart>,
        unreached: Option<UnreachedUses>,
        sender: relm4::Sender<AppInput>,
    ) {
        let browser_failed = || Outcome::Failed(l10n::account_signin_browser_failed().to_owned());
        let url = &start.authorization_url;
        let listening = jmap::authorization_redirect_uri(url)
            .and_then(|redirect| self.host_tasks.oauth_loopback_for_redirect(&redirect).ok())
            .zip(jmap::authorization_state(url));
        let (Some((loopback, state)), Some(app), Some(cancel)) = (
            listening,
            self.app.clone(),
            self.host_tasks.account_sign_in.cancel_token(attempt.id),
        ) else {
            log::warn!("sign-in: the browser could not be readied for the account");
            return self.account_sign_in_finished(&attempt, &browser_failed());
        };
        let (failed, abandoned) = (sender.clone(), attempt.clone());
        oauth_loopback::launch_browser(url, move || {
            failed.emit(finished(abandoned, browser_failed()));
        });
        self.settings.sign_in.in_browser(&attempt.account);
        self.settings.refresh_in_place();
        std::thread::spawn(move || {
            let outcome = match loopback.wait_for_state(
                &cancel,
                &state,
                l10n::account_signin_timeout(),
                l10n::account_signin_browser_failed(),
            ) {
                CallbackOutcome::Received(callback_url) => {
                    sender.emit(input(SignInInput::Completing {
                        attempt: attempt.id,
                        account: attempt.account.clone(),
                    }));
                    match app.complete_account_sign_in(
                        attempt.account.clone(),
                        start.pending,
                        callback_url,
                        unreached,
                    ) {
                        Ok(()) => Outcome::Switched,
                        Err(error) => Outcome::Failed(super::core_error::said(&error)),
                    }
                }
                CallbackOutcome::Cancelled => Outcome::Cancelled,
                CallbackOutcome::TimedOut(_) if attempt.from == ConsentFrom::Notice => {
                    Outcome::Cancelled
                }
                CallbackOutcome::TimedOut(error) | CallbackOutcome::Failed(error) => {
                    Outcome::Failed(error)
                }
            };
            sender.emit(finished(attempt, outcome));
        });
    }

    /// Cancel, while the browser holds the sign-in: the attempt is dropped and the page goes back
    /// to how it was, saying nothing. Its worker stops on the flag, and what it reports is
    /// ignored, because the slot no longer holds it.
    fn cancel_account_sign_in(&mut self, account: &str) {
        if self.settings.sign_in.cancel(account) {
            self.host_tasks.account_sign_in.cancel();
            self.settings.refresh_in_place();
        }
    }

    /// A change ended. The core has already replaced the account, and asks its server again what
    /// it takes, since that depends on the way in; this says so where it was asked for.
    fn account_sign_in_finished(&mut self, attempt: &Attempt, outcome: &Outcome) {
        if !self.host_tasks.account_sign_in.finish(attempt.id) {
            return;
        }
        self.settings.sign_in.end(&attempt.account);
        if *outcome == Outcome::Switched {
            if let Some(app) = &self.app {
                self.snapshot.replace(app.mailbox_list());
            }
            self.pull_notices();
        }
        match attempt.from {
            ConsentFrom::Settings => match settings_notice(attempt.switch, outcome) {
                Some(notice) => self.settings.notify(notice),
                None => self.settings.refresh_in_place(),
            },
            ConsentFrom::Notice => {
                let failed = matches!(outcome, Outcome::Failed(_));
                self.notice_remedy_ended(super::notices::Remedy::AccountSignIn, failed);
                self.settings.refresh_in_place();
            }
        }
    }
}

#[cfg(test)]
#[path = "account_sign_in_tests.rs"]
mod tests;
