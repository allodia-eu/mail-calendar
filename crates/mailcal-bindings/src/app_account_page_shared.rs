//! An account's shared mailboxes on its page (`docs/shared-mailboxes.md`): the server asked in the
//! background, never on the thread that reads the page, and switching or adding one as a change
//! on the page.

use std::sync::Arc;

use engine_api::AccountId;
use mailcal_app::SharedMailboxOffer;

use crate::{
    AccountChangeNotice, AccountChangeRefusal, AccountPageIntent, AccountSharedMailboxes,
    MailcalApp, account_page::shared::Asked, account_work::Hold, app_shared_mailboxes::view,
};

#[uniffi::export]
impl MailcalApp {
    /// The account's page opened: its server is asked again which mailboxes the account's sign-in
    /// opens besides its own (rule 3), and the page is signalled once it answers. What it answered
    /// before is drawn meanwhile.
    pub fn account_page_opened(&self, account_id: String) {
        self.ask_shared(&account_id, true);
    }
}

impl MailcalApp {
    /// `account_id`'s shared mailboxes as its page draws them, asking its server first when it
    /// has not answered or the account changed since it was asked.
    pub(crate) fn shared_section(
        &self,
        account_id: &str,
        hold: &Hold,
    ) -> Option<AccountSharedMailboxes> {
        let shared = &self.account_work.shared;
        // One that cannot open any is answered without asking its server.
        let offer = if self.opens_shared_mailboxes(account_id) {
            self.ask_shared(account_id, false);
            shared.offer(account_id)
        } else {
            Some(Ok(SharedMailboxOffer::Unsupported))
        };
        shared.section(
            account_id,
            offer,
            self.chosen_shared_mailboxes(account_id),
            hold,
        )
    }

    /// Asks `account_id`'s server on a thread of its own, when [`SharedAnswers::ask`] says so.
    ///
    /// [`SharedAnswers::ask`]: crate::account_page::shared::SharedAnswers::ask
    fn ask_shared(&self, account_id: &str, again: bool) {
        if !self.opens_shared_mailboxes(account_id) {
            return;
        }
        let Some(ticket) =
            self.account_work
                .shared
                .ask(account_id, &self.asked_as(account_id), again)
        else {
            return;
        };
        let Some(app) = self.rediscovery.app() else {
            return;
        };
        let account = account_id.to_owned();
        std::thread::spawn(move || {
            let offer = app.shared_offer(&account);
            app.account_work.shared.answered(&account, ticket, offer);
            app.app.accounts_changed();
        });
    }

    /// The account as its server would be asked now.
    fn asked_as(&self, account_id: &str) -> Asked {
        let live = AccountId::try_from(account_id).is_ok_and(|account| {
            let app = Arc::clone(&self.app);
            self.runtime
                .block_on(|| async move { app.has_live_mailbox(&account).await })
        });
        Asked {
            uses: self
                .registry
                .dial(account_id)
                .map(|dial| format!("{:?}", dial.capabilities()))
                .unwrap_or_default(),
            live,
        }
    }

    /// Switches a shared mailbox, or adds one by its address, and answers what needs saying.
    pub(crate) fn run_shared_change(
        &self,
        intent: AccountPageIntent,
    ) -> Option<AccountChangeNotice> {
        let failed = |error: &crate::MailcalError| AccountChangeNotice::SharedMailboxFailed {
            refusal: AccountChangeRefusal::from(error),
        };
        match intent {
            AccountPageIntent::SetSharedMailbox {
                account,
                handle,
                on,
            } => {
                let offer = self
                    .account_work
                    .shared
                    .offer(&account)
                    .unwrap_or(Ok(SharedMailboxOffer::NotConnected));
                let Some(mailbox) = view(offer, self.chosen_shared_mailboxes(&account))
                    .mailboxes
                    .into_iter()
                    .find(|mailbox| mailbox.handle == handle)
                else {
                    return Some(AccountChangeNotice::SharedMailboxFailed {
                        refusal: AccountChangeRefusal::Other {
                            detail: "the server no longer offers that mailbox".to_owned(),
                        },
                    });
                };
                self.set_shared_mailbox(account, mailbox, on)
                    .err()
                    .map(|error| failed(&error))
            }
            AccountPageIntent::AddSharedMailbox { account, address } => {
                match self.resolve_shared_mailbox(account.clone(), address.clone()) {
                    Ok(Some(mailbox)) => {
                        self.account_work.shared.missed(&account, None);
                        self.set_shared_mailbox(account, mailbox, true)
                            .err()
                            .map(|error| failed(&error))
                    }
                    Ok(None) => {
                        let address = address.trim().to_owned();
                        self.account_work
                            .shared
                            .missed(&account, Some(address.clone()));
                        Some(AccountChangeNotice::SharedMailboxNotFound { address })
                    }
                    Err(error) => {
                        self.account_work.shared.missed(&account, Some(address));
                        Some(failed(&error))
                    }
                }
            }
            _ => None,
        }
    }
}
