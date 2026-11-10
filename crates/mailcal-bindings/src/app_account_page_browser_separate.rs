//! A mailbox's separate server signed in through the browser (`docs/accounts.md` rule 18): at
//! its own provider or with Nextcloud, whichever it offered, as the one browser sign-in the core
//! holds. What it came to is said on the section, as Connect's is, never on the page.

use std::sync::Arc;

use super::Way;
use crate::{
    AccountCapability, AccountChangeRefusal, AccountSeparateSignIn, MailcalApp, MailcalError,
    account_page::{
        browser::{Began, Completion, Pending, Target},
        separate::Failure,
    },
};

/// A sign-in to start.
pub(super) struct Launch {
    pub(super) account: String,
    pub(super) target: Target,
    pub(super) way: Way,
    pub(super) adding: Vec<AccountCapability>,
    pub(super) switching: bool,
}

impl MailcalApp {
    /// Starts signing in what `account` needs a sign-in of its own for, the way its server
    /// offered, in place of the sign-in out.
    pub(super) fn start_separate_sign_in(self: Arc<Self>, account: String, redirect_uri: String) {
        let Some(need) = self.separate_need(&account) else {
            log::info!("sign-in: nothing needs a sign-in of its own any more");
            return;
        };
        let offer = self.account_work.separate.offers.get(&account);
        let way = match offer
            .filter(|offer| offer.uses == need.uses)
            .and_then(|offer| offer.sign_in)
        {
            Some(AccountSeparateSignIn::Provider) => Way::SeparateProvider,
            Some(AccountSeparateSignIn::Nextcloud) => Way::SeparateNextcloud,
            None => {
                log::warn!("sign-in: the separate server offers no browser sign-in");
                return;
            }
        };
        let sign_in = Launch {
            account,
            target: Target::Separate { uses: need.uses },
            way,
            adding: Vec::new(),
            switching: false,
        };
        self.launch(sign_in, redirect_uri);
    }

    /// Asks `account`'s separate server to start a sign-in of `target`'s uses.
    pub(super) fn begin_separate(
        &self,
        account: &str,
        way: Way,
        target: &Target,
        redirect_uri: String,
    ) -> Result<Began, MailcalError> {
        let Target::Separate { uses } = target else {
            return Err(MailcalError::Config(
                "only a separate server signs in this way".to_owned(),
            ));
        };
        if way == Way::SeparateNextcloud {
            return self
                .begin_separate_server_nextcloud_sign_in(account.to_owned(), uses.clone())
                .map(|start| Began {
                    pending: Pending::Nextcloud(start.pending),
                    url: start.login_url,
                    redirect_uri: None,
                    unreached: Vec::new(),
                });
        }
        self.begin_separate_server_sign_in(account.to_owned(), uses.clone(), redirect_uri.clone())
            .map(|start| Began {
                pending: Pending::Separate(start.pending),
                url: start.authorization_url,
                redirect_uri: Some(redirect_uri),
                unreached: Vec::new(),
            })
    }

    /// Completes a separate server's sign-in for `account`, as a change on its page: the uses
    /// move to an account of their own, or the section says why not. Answers a Nextcloud grant
    /// to keep for the next try.
    pub(super) fn complete_separate(
        &self,
        account: &str,
        uses: Vec<AccountCapability>,
        completion: Completion,
    ) -> Option<String> {
        let (answer, kept) = match completion {
            Completion::Separate {
                pending,
                callback_url,
            } => (
                self.complete_separate_server_sign_in(account.to_owned(), pending, callback_url),
                None,
            ),
            // The server hands a grant out once: kept for another try unless it can never make
            // these uses an account, when its password is withdrawn.
            Completion::Nextcloud { grant } => {
                let answer =
                    self.add_linked_nextcloud_account(account.to_owned(), uses, grant.clone());
                let kept = match &answer {
                    Ok(_) => None,
                    Err(MailcalError::Config(_) | MailcalError::DifferentAccount) => {
                        self.abandon_nextcloud_sign_in(grant);
                        None
                    }
                    Err(_) => Some(grant),
                };
                (answer, kept)
            }
            Completion::Consent { .. } | Completion::Account { .. } => {
                return None;
            }
        };
        match answer {
            Ok(_) => self.account_work.separate.tried(account, None),
            Err(error) => {
                log::info!("sign-in: the separate server's sign-in did not complete ({error})");
                self.separate_ended(account, AccountChangeRefusal::from(&error));
            }
        }
        kept
    }

    /// Says on `account`'s section why its separate server's sign-in left the uses where they
    /// are.
    pub(super) fn separate_ended(&self, account: &str, refusal: AccountChangeRefusal) {
        let Some(need) = self.separate_need(account) else {
            return;
        };
        let failure = Failure {
            username: need.username,
            refusal,
        };
        self.account_work.separate.tried(account, Some(failure));
    }
}

#[cfg(test)]
#[path = "app_account_page_browser_separate_tests.rs"]
mod tests;
