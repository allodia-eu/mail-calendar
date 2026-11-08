//! An account's page as a client draws it, and the changes it dispatches (`docs/waiting.md`).
//!
//! A change returns at once and runs on a thread of its own, since every one of them can wait on
//! the host's store or a server; the page says it runs until it ends, and says what it came to
//! when that needs saying. A client keeps no state of its own about any of it.

use std::sync::Arc;

use mailcal_account::Capability;

use crate::{
    AccountCapability, AccountChangeNotice, AccountChangeNoticeRaised, AccountPage,
    AccountPageIntent, AccountUseBlock, CapabilityChange, LinkSlot, MailcalApp, MailcalError,
    SyncStrategyKind, account_page, account_page_mail, account_registry::Toggle,
    account_work::PageWork,
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
        let (hold, notice) = self.account_work.of(&account_id);
        let refused = self.refused_uses().remove(&account_id).unwrap_or_default();
        let mail = self
            .runtime
            .block_on(|| self.app.sync_settings_row(&account_id));
        let mut page = account_page::page(&entry, hold.clone(), notice, |capability, on| {
            self.registry
                .toggle(&account_id, Capability::from(capability), on, &refused)
                .unwrap_or(Toggle::Unchanged)
        });
        if let Some(row) = mail {
            let (sender_name, mail) = account_page_mail::mail_half(row.into(), &hold);
            page.sender_name = Some(sender_name);
            page.mail = Some(mail);
        }
        Some(page)
    }

    /// What the last removal came to, when it needs saying: on the accounts list, since the
    /// account's page is gone with it. Its `id` grows with each, so a client says each once.
    #[must_use]
    pub fn accounts_notice(&self) -> Option<AccountChangeNoticeRaised> {
        self.account_work.list_notice()
    }
}

impl MailcalApp {
    /// Starts `intent`'s change, unless another change is running; every page's controls are held
    /// from now until it ends.
    pub(crate) fn dispatch_account(&self, intent: AccountPageIntent) {
        let (account, work) = (intent.account().to_owned(), PageWork::of(&intent));
        let Some(app) = self.rediscovery.app() else {
            log::warn!("accounts: a change to an account was asked for while the app shuts down");
            return;
        };
        if !self.account_work.start(&account, work) {
            return;
        }
        self.app.accounts_changed();
        // A thread rather than the runtime: each change blocks on the runtime itself.
        std::thread::spawn(move || {
            // Freed even when the change panics, or every page would stay held until a restart;
            // the panic still goes on to the crash hook.
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                app.run_account_change(intent)
            }));
            app.account_work
                .finish(&account, outcome.as_ref().ok().cloned().flatten());
            app.app.accounts_changed();
            if let Err(panic) = outcome {
                std::panic::resume_unwind(panic);
            }
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
            // The account is gone whatever this answers: an error says its stored credential
            // could not be erased, which the accounts list says.
            AccountPageIntent::Remove { account } => {
                self.remove_account(account)
                    .err()
                    .map(|error| AccountChangeNotice::RemoveFailed {
                        detail: error.to_string(),
                    })
            }
            mail => {
                self.run_mail_change(mail);
                None
            }
        }
    }

    /// A change to how the account's mail arrives, how much of it is kept, or the name it is sent
    /// under. Each is stored before this returns and none can be refused, so none has anything to
    /// say; what follows from it (telling the provider, reconciling the mail) runs on afterwards
    /// as work the page does not wait on.
    fn run_mail_change(&self, intent: AccountPageIntent) {
        let app = Arc::clone(&self.app);
        match intent {
            AccountPageIntent::SetSenderName { account, name } => {
                self.set_account_sender_name(account, name);
            }
            AccountPageIntent::SetSyncDepth { account, months } => {
                let stored = self
                    .runtime
                    .block_on(|| app.store_account_sync_depth(&account, months));
                if let Some(change) = stored {
                    self.runtime
                        .spawn(async move { app.reconcile_sync_depth(change).await });
                }
            }
            AccountPageIntent::SetMessageSizeLimit { account, megabytes } => {
                let stored = self
                    .runtime
                    .block_on(|| app.store_account_message_size_limit(&account, megabytes));
                if let Some(change) = stored {
                    self.runtime
                        .spawn(async move { app.reconcile_message_size_limit(change).await });
                }
            }
            AccountPageIntent::SetPush { account, on } => {
                let strategy = if on {
                    SyncStrategyKind::Push
                } else {
                    SyncStrategyKind::Poll
                };
                self.set_sync_strategy(account, strategy);
            }
            AccountPageIntent::SetPollInterval { account, minutes } => {
                self.set_poll_interval(account, minutes);
            }
            AccountPageIntent::SetWatchedFolder { account, key, on } => {
                self.set_push_folder(account, key, on);
            }
            AccountPageIntent::SetUse { .. }
            | AccountPageIntent::SetLink { .. }
            | AccountPageIntent::Remove { .. } => {}
        }
    }
}

/// What a switch's change needs saying: only a failure. The page already asks the provider, or
/// says a server is missing, before the switch is pressed, so the core's answer for either is a
/// race the person sees as the switch not taking, and is said with its reason all the same.
fn use_failed(
    capability: AccountCapability,
    on: bool,
    answer: Result<CapabilityChange, MailcalError>,
) -> Option<AccountChangeNotice> {
    let detail = match answer {
        Ok(CapabilityChange::Applied) => return None,
        Ok(CapabilityChange::NeedsConsent) => "the provider has not granted that".to_owned(),
        Ok(CapabilityChange::NeedsEndpoint) => AccountUseBlock::NeedsServer.reason().to_owned(),
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
