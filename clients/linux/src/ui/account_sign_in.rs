//! Changing how an IMAP or JMAP account signs in, and signing one that signs in through its
//! provider in again (`docs/accounts.md` rule 17): what its server accepts, asked when its page
//! opens, and the browser and password routes, each a blocking core call off the main thread.
//! The account keeps its id, uses, links, settings and mail; the core replaces it in place and
//! signals Settings, so the page redraws from the next snapshot.

use mailcal_bindings::{AccountSignInOptions, AccountSignInStart};

use super::{
    AppInput, AppModel,
    account_consent::ConsentFrom,
    account_settings::AccountsInput,
    jmap,
    oauth_loopback::{self, CallbackOutcome},
    settings::notice::Notice,
};
use crate::l10n;

/// What the open account's Sign-in section knows. Here rather than in the window, which is
/// rebuilt on every change while a question to the server and a browser hop each outlive
/// several rebuilds.
#[derive(Debug, Clone, Default)]
pub(crate) struct SignInSwitchState {
    /// What one account's server accepts, and the account it is for.
    answered: Option<(String, AccountSignInOptions)>,
    /// The account whose question is out, so a page opened twice asks once.
    asking: Option<String>,
    /// The account whose sign-in is changing, while it is.
    running: Option<String>,
}

impl SignInSwitchState {
    /// What `account`'s server accepts, once it has answered.
    pub(crate) fn options(&self, account: &str) -> Option<&AccountSignInOptions> {
        self.answered
            .as_ref()
            .filter(|(id, _)| id == account)
            .map(|(_, options)| options)
    }

    /// Whether `account`'s sign-in is changing now, which holds its buttons.
    pub(crate) fn running(&self, account: &str) -> bool {
        self.running.as_deref() == Some(account)
    }

    /// Whether to ask about `account`: nothing is known or asked about it yet.
    fn ask(&mut self, account: &str) -> bool {
        if self.options(account).is_some() || self.asking.as_deref() == Some(account) {
            return false;
        }
        self.asking = Some(account.to_owned());
        true
    }

    fn answer(&mut self, account: String, options: AccountSignInOptions) {
        if self.asking.as_deref() == Some(account.as_str()) {
            self.asking = None;
        }
        self.answered = Some((account, options));
    }

    /// Forgets what `account`'s server was said to accept: its way in changed, and with it the
    /// question.
    pub(crate) fn forget(&mut self, account: &str) {
        if self.options(account).is_some() {
            self.answered = None;
        }
    }
}

/// Which change a sign-in makes, which decides what is said when it ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Switch {
    /// Signs an account that signs in through its provider in again.
    Again,
    /// Signs a password account in through its provider instead.
    ToProvider,
    /// Puts a password on an account that signs in through its provider.
    ToPassword,
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
    /// What `account`'s server accepts.
    Options {
        account: String,
        options: AccountSignInOptions,
    },
    /// Sign `account` in through its provider, from its page.
    Browser { account: String, switch: Switch },
    /// The core started that sign-in, or could not.
    Prepared {
        attempt: Attempt,
        start: Result<Box<AccountSignInStart>, String>,
    },
    /// Put `password` on `account`, in place of its sign-in.
    Password { account: String, password: Password },
    /// A change ended.
    Finished { attempt: Attempt, outcome: Outcome },
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
                Switch::ToPassword => l10n::settings_account_switched_to_password(),
            }
            .to_owned(),
        )),
        Outcome::Cancelled => None,
        Outcome::Failed(detail) => Some(Notice::Error {
            title: match switch {
                Switch::Again => l10n::settings_account_signin_failed(),
                Switch::ToProvider | Switch::ToPassword => l10n::settings_account_switch_failed(),
            }
            .to_owned(),
            detail: detail.clone(),
        }),
    }
}

impl AppModel {
    /// Asks what `account`'s server accepts, off the main thread: detection, a capability probe
    /// and issuer discovery, each bounded. The core answers nothing for an account it offers no
    /// switch on, without asking anyone.
    pub(super) fn ask_sign_in_options(&mut self, account: &str, sender: relm4::Sender<AppInput>) {
        let Some(app) = self.app.clone() else {
            return;
        };
        if !self.settings.sign_in.ask(account) {
            return;
        }
        let account = account.to_owned();
        std::thread::spawn(move || {
            let options = app.account_sign_in_options(account.clone());
            sender.emit(input(SignInInput::Options { account, options }));
        });
    }

    pub(super) fn sign_in_input(&mut self, message: SignInInput, sender: relm4::Sender<AppInput>) {
        match message {
            SignInInput::Options { account, options } => {
                let offers = options.sign_in || options.password;
                self.settings.sign_in.answer(account.clone(), options);
                if offers && self.settings.account.as_deref() == Some(account.as_str()) {
                    self.settings.refresh_in_place();
                }
            }
            SignInInput::Browser { account, switch } => {
                self.start_account_sign_in(account, switch, ConsentFrom::Settings, sender);
            }
            SignInInput::Prepared { attempt, start } => {
                self.account_sign_in_prepared(attempt, start, sender);
            }
            SignInInput::Password { account, password } => {
                self.switch_to_password(account, password.0, sender);
            }
            SignInInput::Finished { attempt, outcome } => {
                self.account_sign_in_finished(&attempt, &outcome, sender);
            }
        }
    }

    /// Starts signing `account` in through its provider: from its page, or from the banner of
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
        let (id, _) = self.host_tasks.account_sign_in.start();
        let attempt = Attempt {
            id,
            account: account.clone(),
            from,
            switch,
        };
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
        self.settings.sign_in.running = Some(account.clone());
        match from {
            ConsentFrom::Settings => {
                let address = app
                    .accounts_snapshot()
                    .accounts
                    .into_iter()
                    .find(|entry| entry.id == account)
                    .map_or_else(|| account.clone(), |entry| entry.address);
                self.settings
                    .notify(Notice::Toast(l10n::settings_account_signin_waiting(
                        &address,
                    )));
            }
            ConsentFrom::MainWindow => self.notice = None,
        }
        let redirect_uri = loopback.redirect_uri();
        std::thread::spawn(move || {
            let start = app
                .begin_account_sign_in(account, redirect_uri)
                .map(Box::new)
                .map_err(|error| super::core_error::said(&error));
            sender.emit(input(SignInInput::Prepared { attempt, start }));
        });
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
        let browser_failed = || Outcome::Failed(l10n::account_signin_browser_failed().to_owned());
        let start = match start {
            Ok(start) => start,
            Err(error) => {
                return self.account_sign_in_finished(&attempt, &Outcome::Failed(error), sender);
            }
        };
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
            return self.account_sign_in_finished(&attempt, &browser_failed(), sender);
        };
        let (failed, abandoned) = (sender.clone(), attempt.clone());
        oauth_loopback::launch_browser(url, move || {
            failed.emit(finished(abandoned, browser_failed()));
        });
        std::thread::spawn(move || {
            let outcome = match loopback.wait_for_state(
                &cancel,
                &state,
                l10n::account_signin_timeout(),
                l10n::account_signin_browser_failed(),
            ) {
                CallbackOutcome::Received(callback_url) => match app.complete_account_sign_in(
                    attempt.account.clone(),
                    start.pending,
                    callback_url,
                    None,
                ) {
                    Ok(()) => Outcome::Switched,
                    Err(error) => Outcome::Failed(super::core_error::said(&error)),
                },
                CallbackOutcome::Cancelled => Outcome::Cancelled,
                CallbackOutcome::Failed(error) => Outcome::Failed(error),
            };
            sender.emit(finished(attempt, outcome));
        });
    }

    /// Tests `password` against the account's servers and stores it in place of its grant.
    fn switch_to_password(
        &mut self,
        account: String,
        password: String,
        sender: relm4::Sender<AppInput>,
    ) {
        let Some(app) = self.app.clone() else {
            return;
        };
        let (id, _) = self.host_tasks.account_sign_in.start();
        let attempt = Attempt {
            id,
            account: account.clone(),
            from: ConsentFrom::Settings,
            switch: Switch::ToPassword,
        };
        self.settings.sign_in.running = Some(account.clone());
        self.settings.refresh_in_place();
        std::thread::spawn(move || {
            let outcome = match app.switch_account_to_password(account, password) {
                Ok(()) => Outcome::Switched,
                Err(error) => Outcome::Failed(super::core_error::said(&error)),
            };
            sender.emit(finished(attempt, outcome));
        });
    }

    /// A change ended. The core has already replaced the account; this says so where it was
    /// asked for, and asks its server again, since the question depends on the way in.
    fn account_sign_in_finished(
        &mut self,
        attempt: &Attempt,
        outcome: &Outcome,
        sender: relm4::Sender<AppInput>,
    ) {
        if !self.host_tasks.account_sign_in.finish(attempt.id) {
            return;
        }
        if self.settings.sign_in.running(&attempt.account) {
            self.settings.sign_in.running = None;
        }
        if *outcome == Outcome::Switched {
            self.settings.sign_in.forget(&attempt.account);
            if let Some(app) = &self.app {
                self.snapshot = app.mailbox_list();
                self.connectivity =
                    super::connectivity::ConnectivityState::pull(app, &self.snapshot.accounts);
            }
            if self.settings.account.as_deref() == Some(attempt.account.as_str()) {
                self.ask_sign_in_options(&attempt.account, sender);
            }
        }
        match attempt.from {
            ConsentFrom::Settings => match settings_notice(attempt.switch, outcome) {
                Some(notice) => self.settings.notify(notice),
                None => self.settings.refresh_in_place(),
            },
            ConsentFrom::MainWindow => {
                self.notice = matches!(outcome, Outcome::Failed(_))
                    .then(|| l10n::signin_expired_failed().to_owned());
                self.settings.refresh_in_place();
            }
        }
    }
}

#[cfg(test)]
#[path = "account_sign_in_tests.rs"]
mod tests;
