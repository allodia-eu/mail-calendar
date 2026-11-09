//! An account's Sign-in section on its page (`docs/accounts.md` rule 17): its server asked in the
//! background which ways in it takes, never on the thread that reads the page, and a typed
//! password tried as a change on the page.

use crate::{
    AccountChangeNotice, AccountChangeRefusal, AccountEntry, AccountProvider, AccountSignIn,
    MailcalApp, account_page::sign_in::section, account_work::Hold,
};

impl MailcalApp {
    /// `entry`'s Sign-in section as its page draws it, its server having `refused` the stored
    /// sign-in or not, asking its server first when it has not answered.
    pub(crate) fn sign_in_section(
        &self,
        entry: &AccountEntry,
        refused: bool,
        hold: &Hold,
    ) -> Option<AccountSignIn> {
        self.ask_sign_in_options(&entry.id, false);
        section(
            entry,
            self.account_work.sign_in.options(&entry.id).as_ref(),
            refused,
            hold,
        )
    }

    /// Whether `account_id`'s server refused its stored sign-in.
    pub(crate) fn sign_in_refused(&self, account_id: &str) -> bool {
        self.app
            .connectivity()
            .signin_expired_accounts
            .iter()
            .any(|id| id == account_id)
    }

    /// Asks `account_id`'s server which ways in it takes, on a thread of its own, unless it is
    /// being asked, or has answered and is not asked `again`. An account whose sign-in Settings
    /// cannot switch is asked nothing: its section needs no answer.
    pub(crate) fn ask_sign_in_options(&self, account_id: &str, again: bool) {
        if matches!(
            self.registry.provider(account_id),
            None | Some(AccountProvider::Microsoft | AccountProvider::Google)
        ) {
            return;
        }
        // The app first: a ticket taken with no thread to answer it would hold every later ask.
        let Some(app) = self.rediscovery.app() else {
            return;
        };
        let Some(ticket) = self.account_work.sign_in.ask(account_id, again) else {
            return;
        };
        let account = account_id.to_owned();
        std::thread::spawn(move || {
            let options = app.account_sign_in_options(account.clone());
            // An answer the page already draws changes nothing on it.
            if app.account_work.sign_in.answered(&account, ticket, options) {
                app.app.accounts_changed();
            }
        });
    }

    /// Tries `password` on `account`: in place of its grant when it signs in through its provider,
    /// and in place of the password or token its server refused otherwise. Answers what needs
    /// saying, which is only a failure: the section says how the account signs in now.
    pub(crate) fn use_password(
        &self,
        account: String,
        password: String,
    ) -> Option<AccountChangeNotice> {
        let answer = match self.registry.provider(&account) {
            Some(AccountProvider::ImapOauth | AccountProvider::JmapOauth) => {
                self.switch_account_to_password(account, password)
            }
            _ => self.replace_account_secret(account, password),
        };
        answer
            .err()
            .map(|error| AccountChangeNotice::PasswordFailed {
                refusal: AccountChangeRefusal::from(&error),
            })
    }
}

#[cfg(test)]
#[path = "app_account_page_sign_in_tests.rs"]
mod tests;
