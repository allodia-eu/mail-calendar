//! The browser half of an account's sign-in, which the core runs (`docs/waiting.md`): a press
//! sends it with the loopback this client listens on, the browser is opened once for each attempt
//! the core holds, and what the loopback receives is handed back. Which step it is at, how long
//! it waits and what it came to are the core's.

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use mailcal_bindings::{
    AccountCapability, AccountPageIntent, BrowserSignIn, BrowserSignInStage, Intent,
    SignInBrowserEnd,
};

use super::{
    AppInput, AppModel,
    account_settings::AccountsInput,
    jmap,
    oauth_loopback::{self, CallbackOutcome},
};
use crate::l10n;

/// The attempt this client opened the browser for, and the flag that stops its loopback's wait
/// once the core no longer holds it.
#[derive(Debug)]
pub(super) struct BrowserDriver {
    opened: Option<(u64, Arc<AtomicBool>)>,
}

impl BrowserDriver {
    pub(super) const fn new() -> Self {
        Self { opened: None }
    }
}

fn ended(account: &str, attempt: u64, end: SignInBrowserEnd) -> AppInput {
    AppInput::Accounts(AccountsInput::Change(
        AccountPageIntent::SignInBrowserEnded {
            account: account.to_owned(),
            attempt,
            end,
        },
    ))
}

impl AppModel {
    /// Starts `account`'s browser sign-in, asking its provider for `adding` too: from its page
    /// or from a notice. It replaces one still out. Every account here returns to the one
    /// loopback this process listens on, and a calendar account's Nextcloud sign-in returns
    /// nothing, so the core ignores it there.
    pub(super) fn sign_in_in_browser(&mut self, account: String, adding: Vec<AccountCapability>) {
        let redirect_uri = self.loopback_redirect();
        self.dispatch(Intent::AccountPage {
            intent: AccountPageIntent::SignIn {
                account,
                adding,
                redirect_uri,
            },
        });
    }

    /// Starts signing in what `account` needs a sign-in of its own for, the way its server
    /// offers: at the server's own provider, which returns to the same loopback, or with
    /// Nextcloud, which returns nothing. It replaces a sign-in still out.
    pub(super) fn sign_in_separately(&mut self, account: String) {
        let redirect_uri = self.loopback_redirect();
        self.dispatch(Intent::AccountPage {
            intent: AccountPageIntent::SignInSeparateServer {
                account,
                redirect_uri,
            },
        });
    }

    /// Where the browser returns to this process; empty, which the core refuses as a browser
    /// that did not open, when no loopback could be bound.
    pub(super) fn loopback_redirect(&mut self) -> String {
        match self.host_tasks.oauth_loopback() {
            Ok(loopback) => loopback.redirect_uri(),
            Err(error) => {
                log::warn!("sign-in: no loopback could be bound for the browser's return: {error}");
                String::new()
            }
        }
    }

    /// The core's account-page signal, for the browser sign-in: a wait on an attempt it no longer
    /// holds stops, and the browser opens for one that now waits on it.
    pub(super) fn browser_sign_in_signalled(&mut self, sender: &relm4::Sender<AppInput>) {
        let Some(app) = self.app.clone() else {
            return;
        };
        let out = app.browser_sign_in();
        if let Some((opened, stop)) = &self.host_tasks.browser.opened
            && out.as_ref().map(|out| out.attempt) != Some(*opened)
        {
            stop.store(true, Ordering::Relaxed);
            self.host_tasks.browser.opened = None;
        }
        if let Some(out) = out
            && self.host_tasks.browser.opened.is_none()
            && let BrowserSignInStage::InBrowser { url, redirect_uri } = &out.stage
        {
            let stop = Arc::new(AtomicBool::new(false));
            self.host_tasks.browser.opened = Some((out.attempt, Arc::clone(&stop)));
            self.open_browser(&out, url, redirect_uri.as_deref(), stop, sender);
        }
    }

    /// Opens `url` for `out` and, when it returns to `redirect_uri`, listens there.
    fn open_browser(
        &mut self,
        out: &BrowserSignIn,
        url: &str,
        redirect_uri: Option<&str>,
        stop: Arc<AtomicBool>,
        sender: &relm4::Sender<AppInput>,
    ) {
        let (account, attempt) = (out.account.clone(), out.attempt);
        let not_opened = {
            let (sender, account) = (sender.clone(), account.clone());
            move || sender.emit(ended(&account, attempt, SignInBrowserEnd::NotOpened))
        };
        let Some(redirect_uri) = redirect_uri else {
            oauth_loopback::launch_browser(url, not_opened);
            return;
        };
        // A grant signed in again returns to the port it was registered with, which may not be
        // the one listened on now.
        let listening = self
            .host_tasks
            .oauth_loopback_for_redirect(redirect_uri)
            .ok()
            .zip(jmap::authorization_state(url));
        let Some((loopback, state)) = listening else {
            log::warn!("sign-in: the browser's return could not be listened for");
            not_opened();
            return;
        };
        oauth_loopback::launch_browser(url, not_opened);
        let sender = sender.clone();
        std::thread::spawn(move || {
            let end = match loopback.wait_for_state(
                &stop,
                &state,
                l10n::account_signin_timeout(),
                l10n::account_signin_browser_failed(),
            ) {
                CallbackOutcome::Received(callback_url) => {
                    SignInBrowserEnd::Redirected { callback_url }
                }
                CallbackOutcome::Failed(_) => SignInBrowserEnd::NotOpened,
                // Over already, or over once the core's own wait runs out.
                CallbackOutcome::Cancelled | CallbackOutcome::TimedOut(_) => return,
            };
            sender.emit(ended(&account, attempt, end));
        });
    }
}

/// The question a switch to the provider raises before the browser opens, while the core asks
/// it: the account, the attempt and what the switch would cost.
pub(super) fn question(out: Option<&BrowserSignIn>) -> Option<(&str, u64, &[AccountCapability])> {
    match out {
        Some(BrowserSignIn {
            account,
            attempt,
            stage: BrowserSignInStage::Asking { unreached },
            ..
        }) => Some((account.as_str(), *attempt, unreached.as_slice())),
        _ => None,
    }
}

#[cfg(test)]
#[path = "browser_sign_in_tests.rs"]
mod tests;
