//! The separate server's section on an account's page (`docs/accounts.md` rule 18): the server
//! asked in the background which ways in it takes besides a password, never on the thread that
//! reads the page, and Connect and Skip run as changes on the page.

use engine_api::AccountId;

use crate::{
    AccountCapability, AccountChangeRefusal, AccountSeparateServer, AccountSeparateSignIn,
    MailcalApp,
    account_page::separate::{Failure, Offer, SeparateNeed, need},
    account_work::Hold,
};

impl MailcalApp {
    /// What `account_id` needs a sign-in of its own for; `None` while every use connects.
    pub(crate) fn separate_need(&self, account_id: &str) -> Option<SeparateNeed> {
        let facts = self.registry.account_facts(account_id)?;
        let id = AccountId::try_from(account_id).ok()?;
        need(&facts, self.app.use_problems(&id))
    }

    /// `account_id`'s separate server as its page draws it, asking the server first when it has
    /// not answered for these uses. A failure kept for a need that is over is forgotten, so a
    /// later one does not open saying it failed.
    pub(crate) fn separate_section(
        &self,
        account_id: &str,
        hold: &Hold,
    ) -> Option<AccountSeparateServer> {
        let Some(need) = self.separate_need(account_id) else {
            self.account_work.separate.tried(account_id, None);
            return None;
        };
        self.ask_separate(account_id, &need.uses, false);
        Some(self.account_work.separate.section(account_id, need, hold))
    }

    /// Asks `account_id`'s separate server, on a thread of its own, whether it signs in at its own
    /// provider or with Nextcloud, unless it is being asked, or has answered for `uses` and is not
    /// asked `again`.
    pub(crate) fn ask_separate(&self, account_id: &str, uses: &[AccountCapability], again: bool) {
        let offers = &self.account_work.separate.offers;
        let again = again
            || offers
                .get(account_id)
                .is_some_and(|offer| offer.uses != uses);
        // The app first: a ticket taken with no thread to answer it would hold every later ask.
        let Some(app) = self.rediscovery.app() else {
            return;
        };
        let Some(ticket) = offers.ask(account_id, again) else {
            return;
        };
        let (account, uses) = (account_id.to_owned(), uses.to_vec());
        std::thread::spawn(move || {
            let sign_in = if app.separate_server_offers_sign_in(account.clone(), uses.clone()) {
                Some(AccountSeparateSignIn::Provider)
            } else if app.separate_server_offers_nextcloud_sign_in(account.clone(), uses.clone()) {
                Some(AccountSeparateSignIn::Nextcloud)
            } else {
                None
            };
            let offer = Offer { uses, sign_in };
            // An answer the page already draws changes nothing on it.
            if app
                .account_work
                .separate
                .offers
                .answered(&account, ticket, offer)
            {
                app.app.accounts_changed();
            }
        });
    }

    /// Moves what `account` needs a sign-in of its own for to an account signing in as `username`
    /// with `password`. What it came to is drawn on the section: the form, saying why, or no form.
    pub(crate) fn connect_separate_server(
        &self,
        account: &str,
        username: String,
        password: String,
    ) {
        // Moved by another surface meanwhile, or connected again by itself.
        let Some(need) = self.separate_need(account) else {
            return;
        };
        let moved = need.uses.clone();
        let failed = self
            .add_linked_calendar_account(account.to_owned(), need.uses, username.clone(), password)
            .err()
            .map(|error| Failure {
                username,
                refusal: AccountChangeRefusal::from(&error),
            });
        if failed.is_none() {
            self.account_work.say(
                account,
                crate::AccountChangeNotice::SignedInSeparately { uses: moved },
            );
        }
        self.account_work.separate.tried(account, failed);
    }

    /// Stops using `account` for what needs a sign-in of its own: setup's Skip.
    pub(crate) fn stop_using_separate_server(&self, account: &str) {
        let Some(need) = self.separate_need(account) else {
            return;
        };
        for capability in need.uses {
            if let Err(error) = self.set_account_capability(account.to_owned(), capability, false) {
                log::warn!("accounts: a use could not be switched off ({error})");
                let failure = Failure {
                    username: need.username,
                    refusal: AccountChangeRefusal::from(&error),
                };
                return self.account_work.separate.tried(account, Some(failure));
            }
        }
        self.account_work.separate.tried(account, None);
    }
}

#[cfg(test)]
#[path = "app_account_page_separate_tests.rs"]
mod tests;
