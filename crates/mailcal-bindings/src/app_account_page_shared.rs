//! An account's shared mailboxes on its page (`docs/shared-mailboxes.md`): the server asked in the
//! background, never on the thread that reads the page, and switching or adding one as a change
//! on the page.

use std::sync::Arc;

use engine_api::AccountId;
use mailcal_account::SharedUses;
use mailcal_app::SharedStoreOffer;

use crate::{
    AccountChangeNotice, AccountChangeRefusal, AccountPageIntent, AccountSharedStores, MailcalApp,
    account_page::shared::Asked, account_work::Hold, app_shared_stores::view,
};

#[uniffi::export]
impl MailcalApp {
    /// The account's page opened: its server is asked again which ways in it takes
    /// (`docs/accounts.md` rule 17), which mailboxes the account's sign-in opens besides its own
    /// (rule 3) and, while its calendar or contacts need a sign-in of their own, which ways in
    /// their server takes (rule 18); the page is signalled once each answers. What it answered
    /// before is drawn meanwhile.
    pub fn account_page_opened(&self, account_id: String) {
        self.ask_sign_in_options(&account_id, true);
        self.ask_shared(&account_id, true);
        if let Some(need) = self.separate_need(&account_id) {
            self.ask_separate(&account_id, &need.uses, true);
        }
    }
}

impl MailcalApp {
    /// `account_id`'s shared mailboxes as its page draws them, asking its server first when it
    /// has not answered or the account changed since it was asked.
    pub(crate) fn shared_section(
        &self,
        account_id: &str,
        hold: &Hold,
    ) -> Option<AccountSharedStores> {
        let shared = &self.account_work.shared;
        // One that cannot open any is answered without asking its server.
        let offer = if self.opens_shared(account_id) {
            self.ask_shared(account_id, false);
            shared.offer(account_id)
        } else {
            Some(Ok(SharedStoreOffer::Unsupported))
        };
        shared.section(account_id, offer, self.chosen_shared(account_id), hold)
    }

    /// Asks `account_id`'s server on a thread of its own, when [`SharedAnswers::ask`] says so.
    ///
    /// [`SharedAnswers::ask`]: crate::account_page::shared::SharedAnswers::ask
    fn ask_shared(&self, account_id: &str, again: bool) {
        if !self.opens_shared(account_id) {
            return;
        }
        // The app first: a ticket taken with no thread to answer it would hold every later ask.
        let Some(app) = self.rediscovery.app() else {
            return;
        };
        let Some(ticket) =
            self.account_work
                .shared
                .ask(account_id, &self.asked_as(account_id), again)
        else {
            return;
        };
        let account = account_id.to_owned();
        std::thread::spawn(move || {
            let offer = app.shared_offer(&account);
            app.account_work.shared.answered(&account, ticket, offer);
            app.app.account_page_changed();
        });
    }

    /// The account as its server would be asked now.
    fn asked_as(&self, account_id: &str) -> Asked {
        let uses = self.shared_uses(account_id);
        let live = AccountId::try_from(account_id).map_or_else(
            |_| SharedUses::default(),
            |account| {
                let app = Arc::clone(&self.app);
                self.runtime
                    .block_on(|| async move { app.live_shared_uses(&account, uses).await })
            },
        );
        Asked {
            uses: self
                .registry
                .dial(account_id)
                .map(|dial| dial.capabilities().clone())
                .unwrap_or_default(),
            live,
        }
    }

    /// Switches a shared mailbox, or adds one by its address, and answers what needs saying.
    pub(crate) fn run_shared_change(
        &self,
        intent: AccountPageIntent,
    ) -> Option<AccountChangeNotice> {
        let failed = |error: &crate::MailcalError| AccountChangeNotice::SharedStoreFailed {
            refusal: AccountChangeRefusal::from(error),
        };
        match intent {
            AccountPageIntent::SetSharedStore {
                account,
                handle,
                on,
            } => {
                let offer = self
                    .account_work
                    .shared
                    .offer(&account)
                    .unwrap_or(Ok(SharedStoreOffer::NotConnected));
                let Some(mailbox) = view(offer, self.chosen_shared(&account))
                    .stores
                    .into_iter()
                    .find(|mailbox| mailbox.handle == handle)
                else {
                    return Some(AccountChangeNotice::SharedStoreFailed {
                        refusal: AccountChangeRefusal::NoLongerShared,
                    });
                };
                self.set_shared_store(account, mailbox, on)
                    .err()
                    .map(|error| failed(&error))
            }
            AccountPageIntent::AddSharedStore { account, address } => {
                let address = address.trim().to_owned();
                // The field keeps the address until it is added, so a refusal is corrected
                // rather than retyped.
                let refused = match self.resolve_shared_store(account.clone(), address.clone()) {
                    Ok(Some(mailbox)) => {
                        match self.set_shared_store(account.clone(), mailbox, true) {
                            Ok(()) => {
                                self.account_work.shared.missed(&account, None);
                                return None;
                            }
                            Err(error) => failed(&error),
                        }
                    }
                    Ok(None) => AccountChangeNotice::SharedStoreNotFound {
                        address: address.clone(),
                    },
                    Err(error) => failed(&error),
                };
                self.account_work.shared.missed(&account, Some(address));
                Some(refused)
            }
            _ => None,
        }
    }
}
