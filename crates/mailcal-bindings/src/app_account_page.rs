//! An account's page as a client draws it, and the changes it dispatches (`docs/waiting.md`).
//!
//! A change returns at once and runs on a thread of its own, since every one of them can wait on
//! the host's store or a server; the page says it runs until it ends, and says what it came to
//! when that needs saying. A client keeps no state of its own about any of it.

use std::sync::Arc;

use mailcal_account::Capability;

use crate::{
    AccountCapability, AccountChangeNotice, AccountPage, AccountPageIntent, CapabilityChange,
    LinkSlot, MailcalApp, MailcalError, account_page, account_work::PageWork,
};

#[uniffi::export]
impl MailcalApp {
    /// The page for `account_id`, or `None` once it is gone. Read again after every
    /// `Surface::Settings` signal, which a change on the page raises when it starts and when it
    /// ends.
    #[must_use]
    pub fn account_page(&self, account_id: String) -> Option<AccountPage> {
        let entry = self
            .accounts_snapshot()
            .accounts
            .into_iter()
            .find(|entry| entry.id == account_id)?;
        let (running, notice) = self.account_work.of(&account_id);
        Some(account_page::page(&entry, running, notice, |capability| {
            self.registry
                .switch_on(&account_id, Capability::from(capability))
        }))
    }
}

impl MailcalApp {
    /// Starts `intent`'s change, unless the page already has one running; its control is held
    /// from now until the change ends.
    pub(crate) fn dispatch_account(&self, intent: AccountPageIntent) {
        let (account, work) = match &intent {
            AccountPageIntent::SetUse {
                account,
                capability,
                ..
            } => (account.clone(), PageWork::Use(*capability)),
            AccountPageIntent::SetLink { account, slot, .. } => {
                (account.clone(), PageWork::Link(*slot))
            }
            AccountPageIntent::Remove { account } => (account.clone(), PageWork::Remove),
        };
        let Some(app) = self.rediscovery.app() else {
            return;
        };
        if !self.account_work.start(&account, work) {
            return;
        }
        self.app.accounts_changed();
        // A thread rather than the runtime: each change blocks on the runtime itself.
        std::thread::spawn(move || {
            let notice = app.run_account_change(intent);
            app.account_work.finish(&account, notice);
            app.app.accounts_changed();
        });
    }

    /// Makes the change, and answers what needs saying about it.
    fn run_account_change(
        self: &Arc<Self>,
        intent: AccountPageIntent,
    ) -> Option<AccountChangeNotice> {
        match intent {
            AccountPageIntent::SetUse {
                account,
                capability,
                on,
            } => use_failed(
                capability,
                on,
                self.set_account_capability(account, capability, on),
            ),
            AccountPageIntent::SetLink {
                account,
                slot,
                target,
            } => link_failed(slot, self.set_account_link(account, slot, target)),
            AccountPageIntent::Remove { account } => {
                let removed = self.remove_account(account.clone());
                if removed.is_ok() {
                    self.account_work.forget(&account);
                }
                removed
                    .err()
                    .map(|error| AccountChangeNotice::RemoveFailed {
                        detail: error.to_string(),
                    })
            }
        }
    }
}

/// What a switch's change needs saying: only a failure. The page already asks the provider, or
/// says a server is missing, before the switch is pressed, so the core's answer for either is a
/// race the person sees as the switch not taking.
fn use_failed(
    capability: AccountCapability,
    on: bool,
    answer: Result<CapabilityChange, MailcalError>,
) -> Option<AccountChangeNotice> {
    let detail = match answer {
        Ok(CapabilityChange::Applied) => return None,
        Ok(CapabilityChange::NeedsConsent | CapabilityChange::NeedsEndpoint) => String::new(),
        Err(error) => error.to_string(),
    };
    Some(AccountChangeNotice::UseFailed {
        capability,
        on,
        detail,
    })
}

fn link_failed(slot: LinkSlot, answer: Result<(), MailcalError>) -> Option<AccountChangeNotice> {
    answer.err().map(|error| AccountChangeNotice::LinkFailed {
        slot,
        detail: error.to_string(),
    })
}

#[cfg(test)]
#[path = "app_account_page_tests.rs"]
mod tests;
