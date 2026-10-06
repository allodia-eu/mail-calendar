//! Settings → Accounts: the snapshot every client draws it from, and what removing an account does
//! to the links other accounts hold to it.

use crate::{AccountsSnapshot, LinkSlot, MailcalApp, MailcalError, accounts_view};

impl MailcalApp {
    /// Asks `id`'s calendar server, once per registration and off this thread, which addresses it
    /// schedules as, and signals Settings when it named some, so the next snapshot suggests
    /// links from them.
    fn ask_calendar_users(&self, id: &str) {
        if !self.registry.begin_calendar_ask(id) {
            return;
        }
        let Ok(account) = engine_api::AccountId::try_from(id) else {
            return;
        };
        let (app, registry, id) = (
            std::sync::Arc::clone(&self.app),
            std::sync::Arc::clone(&self.registry),
            id.to_owned(),
        );
        self.runtime.spawn(async move {
            let answer = app
                .calendar_user_addresses(&account)
                .await
                .map(|addresses| addresses.addresses().to_vec());
            let named = answer
                .as_ref()
                .is_some_and(|addresses| !addresses.is_empty());
            registry.calendar_answered(&id, answer);
            if named {
                app.accounts_changed();
            }
        });
    }
}

#[uniffi::export]
impl MailcalApp {
    /// Every account on this device, mail or not, in the order the host stored them: what each
    /// is used for, in what state, and which accounts it relies on. Pulled after a
    /// [`Surface::Settings`](crate::Surface::Settings) signal, which adding, removing and signing
    /// an account in again all raise, and after a
    /// [`Surface::Connectivity`](crate::Surface::Connectivity) one, which a provider refusing a
    /// calendar raises.
    ///
    /// An empty list means the last account is gone, and the client returns to first-run setup.
    pub fn accounts_snapshot(&self) -> AccountsSnapshot {
        let app = std::sync::Arc::clone(&self.app);
        let order = self
            .runtime
            .block_on(|| async move { app.account_ids().await });
        let mut facts = self.registry.facts();
        let position = |id: &str| {
            order
                .iter()
                .position(|listed| listed.as_str() == id)
                .unwrap_or(usize::MAX)
        };
        facts.sort_by(|a, b| position(&a.id).cmp(&position(&b.id)).then(a.id.cmp(&b.id)));
        for account in &mut facts {
            if account
                .chosen
                .contains(mailcal_account::Capability::Calendar)
            {
                account.calendar_addresses = self.registry.calendar_addresses(&account.id);
                self.ask_calendar_users(&account.id);
            }
        }
        let calendar_refused = self
            .app
            .connectivity()
            .calendar_reauth_accounts
            .into_iter()
            .collect();
        AccountsSnapshot {
            accounts: accounts_view::entries(&facts, &calendar_refused),
        }
    }

    /// Links `account_id` in `slot` to `target`, one of the entry's `link_candidates`, or clears
    /// that link when `target` is `None`, and stores every account it changed. Naming a mail
    /// account from a calendar links that mail account to the calendar too.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Config`] for an unknown account or a target the slot may not name,
    /// and [`MailcalError::Connect`] when the host's store refused a write; the link then holds
    /// until the app is restarted.
    pub fn set_account_link(
        &self,
        account_id: String,
        slot: LinkSlot,
        target: Option<String>,
    ) -> Result<(), MailcalError> {
        let changed = self
            .registry
            .set_link(&account_id, slot, target.as_deref())?;
        self.app.accounts_changed();
        for (id, config) in changed {
            self.persist_config(&id, config.map_err(MailcalError::Config)?)?;
        }
        Ok(())
    }
}

impl MailcalApp {
    /// Clears every link the other accounts hold to `removed`, and stores each account it
    /// changed.
    ///
    /// A store that refuses is logged and left: the link it kept names an account that no longer
    /// exists, which reads as no link until the account is changed again.
    pub(crate) fn clear_links_to(&self, removed: &str) {
        for (id, config) in self.registry.clear_links_to(removed) {
            let stored = config
                .map_err(MailcalError::Config)
                .and_then(|config| self.persist_config(&id, config));
            if let Err(err) = stored {
                log::warn!(
                    "credentials: [{}] a link to a removed account could not be cleared from \
                     this account's stored config ({err})",
                    mailcal_account::account_log_handle(&id),
                );
            }
        }
    }

    /// Stores `config`, a serialization of `id`'s registry entry taken under its lock, and stores
    /// the entry again if it has changed since.
    ///
    /// The write happens after the lock is released, so a token rotation can land in between: its
    /// sink stores the new refresh token first, and `config` would then put the superseded one
    /// back. Storing what the registry holds afterwards ends the store on the newer token whichever
    /// of the two writes lands last.
    pub(crate) fn persist_config(&self, id: &str, config: String) -> Result<(), MailcalError> {
        self.persist_credential(id, config.clone())?;
        match self.registry.config_toml(id) {
            Some(Ok(current)) if current != config => self.persist_credential(id, current),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
#[path = "app_accounts_view_tests.rs"]
mod tests;
