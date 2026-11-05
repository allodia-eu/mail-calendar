//! Signing a separate calendar server in at its own provider, from the form that otherwise asks
//! for its password (`docs/mail-oauth.md` rule 13): whether the server offers it, asked once the
//! form is drawn, and the browser flow when it does. The flow is the IMAP sign-in's, through the
//! same loopback redirect, and each step is taken only for the attempt its form is still on, so
//! a fresh press supersedes the attempt still out and a way back retires it
//! (`docs/provider-oauth.md` rule 13).

use mailcal_bindings::{AccountCapability, MailcalApp, MailcalError};

use super::{
    AppInput, AppModel,
    account_settings::AccountsInput,
    imap_signin::{self, CallbackOutcome},
    oauth_loopback::OAuthLoopback,
    separate_sign_in::{SeparateFrom, SeparateInput},
};
use crate::l10n;

/// How an attempt ended on its worker.
#[derive(Debug)]
pub(crate) enum SignInEnd {
    /// The core answered the exchange, or what to say about why it refused.
    Done(Result<(), String>),
    /// The wait was retired or superseded.
    Cancelled,
    /// The browser did not open, or the redirect never came.
    Failed,
}

/// The browser half, once the core has found the provider and registered with it.
pub(crate) struct Prepared {
    authorization_url: String,
    pending: String,
    expected_state: String,
    loopback: OAuthLoopback,
}

/// The pending handle and the redirect it came back with. Both carry what signs the person in.
pub(crate) struct Redirect {
    pending: String,
    callback_url: String,
}

impl std::fmt::Debug for Prepared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Prepared(..)")
    }
}

impl std::fmt::Debug for Redirect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Redirect(..)")
    }
}

/// The sign-in's traffic, for the form it was asked from.
#[derive(Debug)]
pub(crate) enum ProviderInput {
    /// Whether the server signs in at its own provider, or with Nextcloud, for the uses it was
    /// asked about.
    Offered {
        account: String,
        uses: Vec<AccountCapability>,
        offered: bool,
        nextcloud: bool,
    },
    /// Sign in at the server's own provider.
    Start,
    Prepared {
        attempt: u64,
        prepared: Result<Box<Prepared>, String>,
    },
    Redirected {
        attempt: u64,
        redirect: Box<Redirect>,
    },
    Ended {
        attempt: u64,
        account: String,
        end: SignInEnd,
    },
    /// Give up on the sign-in the browser holds.
    Cancel,
    /// How a sign-in with Nextcloud is getting on.
    Nextcloud {
        attempt: u64,
        account: String,
        flight: super::nextcloud_flight::Flight,
    },
    /// The core made an account of a Nextcloud grant, or did not.
    NextcloudAdded {
        attempt: u64,
        account: String,
        result: Result<(), String>,
    },
}

fn provider(from: SeparateFrom, input: ProviderInput) -> AppInput {
    AppInput::Accounts(AccountsInput::Separate(SeparateInput::Provider {
        from,
        input,
    }))
}

/// A refusal as the form says it: the core's mapping, with a failed connect read as the
/// sign-in having failed, since the server's own words are for a developer.
fn said(error: &MailcalError) -> String {
    match error {
        MailcalError::Connect(_) => l10n::setup_imap_signin_failed().to_owned(),
        other => super::core_error::said(other),
    }
}

/// What an end comes to on its form: the answer a Connect would have given, so success goes
/// where Connect's goes, or `None` for an end that says nothing.
pub(super) fn as_connected(end: SignInEnd) -> Option<Result<(), String>> {
    match end {
        SignInEnd::Done(result) => Some(result),
        SignInEnd::Failed => Some(Err(l10n::setup_imap_signin_failed().to_owned())),
        SignInEnd::Cancelled => None,
    }
}

pub(super) const fn other(from: SeparateFrom) -> SeparateFrom {
    match from {
        SeparateFrom::Setup => SeparateFrom::Page,
        SeparateFrom::Page => SeparateFrom::Setup,
    }
}

/// Finds the provider, registers with it for the loopback redirect, and reads the `state` the
/// redirect must echo.
fn prepare(
    app: &MailcalApp,
    loopback: OAuthLoopback,
    account: String,
    uses: Vec<AccountCapability>,
) -> Result<Box<Prepared>, String> {
    let start = app
        .begin_separate_server_sign_in(account, uses, loopback.redirect_uri())
        .map_err(|error| said(&error))?;
    let expected_state = imap_signin::authorization_state(&start.authorization_url)
        .ok_or_else(|| l10n::setup_imap_signin_failed().to_owned())?;
    Ok(Box::new(Prepared {
        authorization_url: start.authorization_url,
        pending: start.pending,
        expected_state,
        loopback,
    }))
}

impl AppModel {
    /// Asks, off the main thread, whether the form's server signs in at its own provider. Until
    /// it answers the form stands as it is.
    pub(super) fn ask_separate_offer(
        &mut self,
        from: SeparateFrom,
        sender: relm4::Sender<AppInput>,
    ) {
        let Some(app) = self.app.clone() else {
            return;
        };
        let Some(form) = self.separate_form(from) else {
            return;
        };
        let (account, uses) = (form.account.clone(), form.server.uses.clone());
        std::thread::spawn(move || {
            let offered = app.separate_server_offers_sign_in(account.clone(), uses.clone());
            let nextcloud = !offered
                && app.separate_server_offers_nextcloud_sign_in(account.clone(), uses.clone());
            sender.emit(provider(
                from,
                ProviderInput::Offered {
                    account,
                    uses,
                    offered,
                    nextcloud,
                },
            ));
        });
    }

    pub(super) fn provider_input(
        &mut self,
        from: SeparateFrom,
        input: ProviderInput,
        sender: relm4::Sender<AppInput>,
    ) {
        match input {
            ProviderInput::Offered {
                account,
                uses,
                offered,
                nextcloud,
            } => {
                if let Some(form) = self
                    .separate_form(from)
                    .filter(|form| form.account == account)
                {
                    form.offer_answered(&uses, offered, nextcloud);
                    self.redraw_separate(from);
                }
            }
            ProviderInput::Start => self.start_provider_sign_in(from, sender),
            ProviderInput::Prepared { attempt, prepared } => {
                self.provider_prepared(from, attempt, prepared, sender);
            }
            ProviderInput::Redirected { attempt, redirect } => {
                self.provider_redirected(from, attempt, *redirect, sender);
            }
            ProviderInput::Ended {
                attempt,
                account,
                end,
            } => self.provider_ended(from, attempt, &account, end, sender),
            ProviderInput::Cancel => {
                self.retire_provider_sign_in(from);
                self.redraw_separate(from);
            }
            ProviderInput::Nextcloud {
                attempt,
                account,
                flight,
            } => self.separate_nextcloud_flight(from, attempt, account, flight, sender),
            ProviderInput::NextcloudAdded {
                attempt,
                account,
                result,
            } => self.separate_nextcloud_added(from, attempt, &account, result, sender),
        }
    }

    fn start_provider_sign_in(&mut self, from: SeparateFrom, sender: relm4::Sender<AppInput>) {
        let Some(app) = self.app.clone() else {
            return;
        };
        if self.separate_form(from).is_some_and(|form| form.nextcloud) {
            return self.start_separate_nextcloud(from, sender);
        }
        if !self
            .separate_form(from)
            .is_some_and(|form| form.offers_sign_in && !form.held())
        {
            return;
        }
        let Ok(loopback) = self.host_tasks.oauth_loopback() else {
            log::warn!("separate server sign-in loopback bind failed");
            if let Some(form) = self.separate_form(from) {
                form.failed(l10n::setup_imap_signin_failed().to_owned());
            }
            self.redraw_separate(from);
            return;
        };
        // One redirect serves both forms, so a press on one supersedes the other's attempt.
        self.retire_provider_sign_in(other(from));
        let (attempt, _) = self.host_tasks.separate.start();
        let Some(form) = self
            .separate_form(from)
            .and_then(|form| form.begin_sign_in(attempt).then_some(form))
        else {
            return;
        };
        let (account, uses) = (form.account.clone(), form.server.uses.clone());
        self.redraw_separate(from);
        std::thread::spawn(move || {
            let prepared = prepare(&app, loopback, account, uses);
            sender.emit(provider(
                from,
                ProviderInput::Prepared { attempt, prepared },
            ));
        });
    }

    /// Opens the browser for an attempt still on its form, and waits for the redirect off the
    /// main thread. A retired attempt opens nothing.
    fn provider_prepared(
        &mut self,
        from: SeparateFrom,
        attempt: u64,
        prepared: Result<Box<Prepared>, String>,
        sender: relm4::Sender<AppInput>,
    ) {
        let outcome = prepared.as_ref().map(|_| ()).map_err(Clone::clone);
        let form = self.separate_form(from);
        let account = form.as_ref().map(|form| form.account.clone());
        let opens = form.is_some_and(|form| form.sign_in_prepared(attempt, outcome));
        self.redraw_separate(from);
        let (true, Some(account), Ok(prepared), Some(cancel)) = (
            opens,
            account,
            prepared,
            self.host_tasks.separate.cancel_token(attempt),
        ) else {
            if self.host_tasks.separate.holds(attempt) {
                self.host_tasks.separate.cancel();
            }
            return;
        };
        let Prepared {
            authorization_url,
            pending,
            expected_state,
            loopback,
        } = *prepared;
        let (failed, unopened) = (sender.clone(), account.clone());
        imap_signin::launch_browser(&authorization_url, move || {
            failed.emit(provider(
                from,
                ProviderInput::Ended {
                    attempt,
                    account: unopened,
                    end: SignInEnd::Failed,
                },
            ));
        });
        std::thread::spawn(move || {
            let ended = |end| ProviderInput::Ended {
                attempt,
                account,
                end,
            };
            let input = match imap_signin::wait(loopback, &cancel, &expected_state) {
                CallbackOutcome::Received(callback_url) => ProviderInput::Redirected {
                    attempt,
                    redirect: Box::new(Redirect {
                        pending,
                        callback_url,
                    }),
                },
                CallbackOutcome::Cancelled => ended(SignInEnd::Cancelled),
                CallbackOutcome::Failed(_) => ended(SignInEnd::Failed),
            };
            sender.emit(provider(from, input));
        });
    }

    /// Exchanges the redirect, off the main thread, only for the attempt its form is still
    /// waiting on: a retired one adds no account.
    fn provider_redirected(
        &mut self,
        from: SeparateFrom,
        attempt: u64,
        redirect: Redirect,
        sender: relm4::Sender<AppInput>,
    ) {
        let Some(app) = self.app.clone() else {
            return;
        };
        if !self.host_tasks.separate.holds(attempt) {
            return;
        }
        let Some(account) = self.separate_form(from).and_then(|form| {
            form.sign_in_redirected(attempt)
                .then(|| form.account.clone())
        }) else {
            self.host_tasks.separate.cancel();
            return;
        };
        self.host_tasks.separate.finish(attempt);
        self.redraw_separate(from);
        std::thread::spawn(move || {
            let result = app
                .complete_separate_server_sign_in(
                    account.clone(),
                    redirect.pending,
                    redirect.callback_url,
                )
                .map(|_| ())
                .map_err(|error| said(&error));
            sender.emit(provider(
                from,
                ProviderInput::Ended {
                    attempt,
                    account,
                    end: SignInEnd::Done(result),
                },
            ));
        });
    }

    pub(super) fn provider_ended(
        &mut self,
        from: SeparateFrom,
        attempt: u64,
        account: &str,
        end: SignInEnd,
        sender: relm4::Sender<AppInput>,
    ) {
        if self.host_tasks.separate.holds(attempt) {
            self.host_tasks.separate.cancel();
        }
        let ours = self
            .separate_form(from)
            .filter(|form| form.account == account)
            .is_some_and(|form| form.sign_in_ended(attempt));
        if !ours {
            // An exchange whose form closed under it still added the account; the mailbox is
            // read again rather than left showing the uses it gave away.
            if matches!(end, SignInEnd::Done(Ok(()))) {
                self.accounts_changed(sender);
            }
            return;
        }
        match as_connected(end) {
            Some(result) => self.separate_connected(from, account, result, sender),
            None => self.redraw_separate(from),
        }
    }
}

#[path = "separate_nextcloud_sign_in.rs"]
mod nextcloud;
#[path = "separate_sign_in_stage.rs"]
mod stage;
pub(crate) use stage::SignInStage;

#[cfg(test)]
#[path = "separate_server_sign_in_tests.rs"]
mod tests;
