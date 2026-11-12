//! Reading the row an account's mail half is drawn from, never on the thread that draws the page
//! (`docs/waiting.md` rule 4): it waits on the store and on the live account list.

use crate::{MailcalApp, account_page_mail::going_strategy};

impl MailcalApp {
    /// Reads `account`'s row now and keeps it, as a running change would store it. Answers whether
    /// the page draws something else now. **Blocking** on the store; never called from a UI
    /// thread.
    pub(crate) fn read_mail_row(&self, account: &str) -> bool {
        let rows = &self.account_work.mail_rows;
        let read = rows.begin();
        let going = going_strategy(&self.account_work.of(account).0);
        let row = self
            .runtime
            .block_on(|| self.app.sync_settings_row(account, going));
        rows.found(account, read, row)
    }

    /// Reads `account`'s row again on a thread of its own, and signals the page when it changed.
    pub(crate) fn ask_mail_row(&self, account: &str) {
        // The app first: an ask taken with no thread to answer it would hold every later one.
        let Some(app) = self.rediscovery.app() else {
            return;
        };
        if !self.account_work.mail_rows.ask(account) {
            return;
        }
        let account = account.to_owned();
        std::thread::spawn(move || {
            loop {
                if app.read_mail_row(&account) {
                    app.app.account_page_changed();
                }
                if !app.account_work.mail_rows.landed(&account) {
                    break;
                }
            }
        });
    }
}
