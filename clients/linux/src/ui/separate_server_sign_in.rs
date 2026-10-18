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
    separate_sign_in::{SeparateForm, SeparateFrom, SeparateInput},
};
use crate::l10n;

/// Where a sign-in at the server's own provider stands, by the attempt it is for.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum SignInStage {
    #[default]
    Idle,
    /// The core is finding the provider and registering with it.
    Starting(u64),
    /// The browser holds the sign-in.
    Waiting(u64),
    /// The browser came back; the core is exchanging the code and adding the account.
    Finishing(u64),
}

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
    /// Whether the server signs in at its own provider, for the uses it was asked about.
    Offered {
        account: String,
        uses: Vec<AccountCapability>,
        offered: bool,
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

impl SeparateForm {
    /// Whether every control is held: a connect, a skip or one of the sign-in's blocking core
    /// calls is running. The browser's wait holds nothing.
    pub(super) const fn held(&self) -> bool {
        self.busy
            || matches!(
                self.stage,
                SignInStage::Starting(_) | SignInStage::Finishing(_)
            )
    }

    /// The server answered whether it signs in at its own provider, for `uses`. Only an answer
    /// for the uses this form moves is its answer.
    pub(super) fn offer_answered(&mut self, uses: &[AccountCapability], offered: bool) {
        if uses == self.server.uses.as_slice() {
            self.offers_sign_in = offered;
        }
    }

    /// Starts `attempt`, superseding the one in the browser. `false` while there is nothing to
    /// start: no offer, or a core call running.
    pub(super) fn begin_sign_in(&mut self, attempt: u64) -> bool {
        if !self.offers_sign_in || self.held() {
            return false;
        }
        self.stage = SignInStage::Starting(attempt);
        self.error = None;
        true
    }

    /// The core prepared `attempt`. `true` when the browser should open for it: it is still
    /// the form's attempt and the core found the provider.
    pub(super) fn sign_in_prepared(&mut self, attempt: u64, result: Result<(), String>) -> bool {
        if self.stage != SignInStage::Starting(attempt) {
            return false;
        }
        match result {
            Ok(()) => {
                self.stage = SignInStage::Waiting(attempt);
                true
            }
            Err(error) => {
                self.stage = SignInStage::Idle;
                self.failed(error);
                false
            }
        }
    }

    /// The browser came back for `attempt`. `true` when it is still the form's attempt, so the
    /// exchange may add the account.
    pub(super) fn sign_in_redirected(&mut self, attempt: u64) -> bool {
        if self.stage != SignInStage::Waiting(attempt) {
            return false;
        }
        self.stage = SignInStage::Finishing(attempt);
        true
    }

    /// `attempt` ended: back to the offer. `false` when the form has moved on from it.
    pub(super) fn sign_in_ended(&mut self, attempt: u64) -> bool {
        let ours = matches!(
            self.stage,
            SignInStage::Starting(id) | SignInStage::Waiting(id) | SignInStage::Finishing(id)
                if id == attempt
        );
        if ours {
            self.stage = SignInStage::Idle;
        }
        ours
    }

    /// Takes the form off an attempt not yet exchanging. `true` when there was one, whose slot
    /// the caller releases. An exchange already running is a core call, and finishes.
    pub(super) fn retire_sign_in(&mut self) -> bool {
        let retired = matches!(
            self.stage,
            SignInStage::Starting(_) | SignInStage::Waiting(_)
        );
        if retired {
            self.stage = SignInStage::Idle;
        }
        retired
    }
}

const fn other(from: SeparateFrom) -> SeparateFrom {
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
            sender.emit(provider(
                from,
                ProviderInput::Offered {
                    account,
                    uses,
                    offered,
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
            } => {
                if let Some(form) = self
                    .separate_form(from)
                    .filter(|form| form.account == account)
                {
                    form.offer_answered(&uses, offered);
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
        }
    }

    /// Takes the form `from` names off its sign-in, when one is out, and releases the slot.
    pub(super) fn retire_provider_sign_in(&mut self, from: SeparateFrom) {
        if self
            .separate_form(from)
            .is_some_and(SeparateForm::retire_sign_in)
        {
            self.host_tasks.separate.cancel();
        }
    }

    fn start_provider_sign_in(&mut self, from: SeparateFrom, sender: relm4::Sender<AppInput>) {
        let Some(app) = self.app.clone() else {
            return;
        };
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

    fn provider_ended(
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

#[cfg(test)]
#[path = "separate_server_sign_in_tests.rs"]
mod tests;
