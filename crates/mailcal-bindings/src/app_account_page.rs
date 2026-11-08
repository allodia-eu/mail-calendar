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
    account_work::{PageWork, Start},
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
        let pushing = hold.going(|work| match work {
            PageWork::Push(on) => Some(if *on {
                mailcal_viewmodel::SyncStrategyKind::Push
            } else {
                mailcal_viewmodel::SyncStrategyKind::Poll
            }),
            _ => None,
        });
        let mail = self
            .runtime
            .block_on(|| self.app.sync_settings_row(&account_id, pushing));
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

    /// The notice `account_id`'s page shows, without reading the rest of the page, for a client
    /// that says it apart from drawing the page.
    #[must_use]
    pub fn account_page_notice(&self, account_id: String) -> Option<AccountChangeNoticeRaised> {
        self.account_work.of(&account_id).1
    }

    /// What the last removal came to, when it needs saying: on the accounts list, since the
    /// account's page is gone with it. Its `id` grows with each, so a client says each once.
    #[must_use]
    pub fn accounts_notice(&self) -> Option<AccountChangeNoticeRaised> {
        self.account_work.list_notice()
    }
}

impl MailcalApp {
    /// Starts `intent`'s change, or queues it behind the one running; every page's controls are
    /// held from now until the last of them ends.
    pub(crate) fn dispatch_account(&self, intent: AccountPageIntent) {
        let Some(app) = self.rediscovery.app() else {
            log::warn!("accounts: a change to an account was asked for while the app shuts down");
            return;
        };
        match self.account_work.start(intent) {
            Start::Run(intent) => {
                self.app.accounts_changed();
                Self::run_from(app, intent);
            }
            // Drawn running from now, at the value it is moving to.
            Start::Waits => self.app.accounts_changed(),
            Start::Dropped => {}
        }
    }

    /// Runs `intent`, then each change that waited behind it, on a thread of their own: each
    /// change blocks on the runtime itself.
    fn run_from(app: Arc<Self>, intent: AccountPageIntent) {
        std::thread::spawn(move || {
            let mut next = Some(intent);
            while let Some(intent) = next {
                let account = intent.account().to_owned();
                // Freed even when the change panics, or every page would stay held until a
                // restart; the panic still goes on to the crash hook, after the next change is
                // handed a thread of its own.
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    app.run_account_change(intent)
                }));
                next = app
                    .account_work
                    .finish(&account, outcome.as_ref().ok().cloned().flatten());
                app.app.accounts_changed();
                if let Err(panic) = outcome {
                    if let Some(next) = next {
                        Self::run_from(Arc::clone(&app), next);
                    }
                    std::panic::resume_unwind(panic);
                }
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
            // A change to how the account's mail arrives, how much of it is kept, or the name it
            // is sent under. Each is stored before this returns and none can be refused, so none
            // has anything to say; what follows from it (telling the provider, reconciling the
            // mail) runs on afterwards as work the page does not wait on.
            AccountPageIntent::SetSenderName { account, name } => {
                self.set_account_sender_name(account, name);
                None
            }
            AccountPageIntent::SetSyncDepth { account, months } => {
                let app = Arc::clone(&self.app);
                let stored = self
                    .runtime
                    .block_on(|| app.store_account_sync_depth(&account, months));
                if let Some(change) = stored {
                    self.runtime
                        .spawn(async move { app.reconcile_sync_depth(change).await });
                }
                None
            }
            AccountPageIntent::SetMessageSizeLimit { account, megabytes } => {
                let app = Arc::clone(&self.app);
                let stored = self
                    .runtime
                    .block_on(|| app.store_account_message_size_limit(&account, megabytes));
                if let Some(change) = stored {
                    self.runtime
                        .spawn(async move { app.reconcile_message_size_limit(change).await });
                }
                None
            }
            AccountPageIntent::SetPush { account, on } => {
                let strategy = if on {
                    SyncStrategyKind::Push
                } else {
                    SyncStrategyKind::Poll
                };
                self.set_sync_strategy(account, strategy);
                None
            }
            AccountPageIntent::SetPollInterval { account, minutes } => {
                self.set_poll_interval(account, minutes);
                None
            }
            AccountPageIntent::SetWatchedFolder { account, key, on } => {
                self.set_push_folder(account, key, on);
                None
            }
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
