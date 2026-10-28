//! Settings → Accounts → an account's shared mailboxes: which mailboxes its sign-in opens
//! besides its own, and which of them it is used for (`docs/shared-mailboxes.md`).

use std::sync::Arc;

use engine_api::{AccountId, SharedMailboxId};
use mailcal_account::{Capability, SharedMailboxChoice};
use mailcal_app::{SharedMailboxLookup, SharedMailboxOffer};

use crate::{MailcalApp, MailcalError};

/// Whether an account's sign-in can open mailboxes besides its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum SharedMailboxSupport {
    /// The server lists the mailboxes it opens; they are in
    /// [`SharedMailboxesView::mailboxes`], none when nothing is shared with the account.
    Listed,
    /// The server lists none, but opens one at an address the person types
    /// ([`MailcalApp::resolve_shared_mailbox`]); the chosen ones are in
    /// [`SharedMailboxesView::mailboxes`].
    ByAddress,
    /// The server lists none this app can open.
    Unsupported,
    /// The account's mailbox is not connected, so there is nothing to ask yet.
    NotConnected,
    /// The server was asked and did not answer.
    Unreachable,
}

/// One mailbox an account's sign-in opens besides its own.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SharedMailboxEntry {
    /// The server's handle for it, handed back to [`MailcalApp::set_shared_mailbox`].
    pub handle: String,
    /// The server's label for it.
    pub name: String,
    /// The address it sends as, when the server gives one.
    pub address: Option<String>,
    /// Whether the account is used for it: it is then a mail account of its own.
    pub selected: bool,
    /// Whether the server still lists it. A chosen mailbox the server stopped listing is kept
    /// until it is switched off, so access withdrawn for a moment does not delete its mail.
    pub listed: bool,
}

/// An account's shared mailboxes, as its server answered and as the account is used.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SharedMailboxesView {
    /// Whether the sign-in can open any.
    pub support: SharedMailboxSupport,
    /// The chosen ones first, in the order they were chosen, then the rest the server listed.
    pub mailboxes: Vec<SharedMailboxEntry>,
}

#[uniffi::export]
impl MailcalApp {
    /// Asks `account_id`'s server which mailboxes its sign-in opens besides its own. Blocks on
    /// the network, so a client calls it off its UI thread, when the account's page opens.
    ///
    /// The mailboxes the account is used for are in the answer whatever the server said, so
    /// one can be switched off while the server is away.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Config`] for an account that is not set up.
    pub fn shared_mailboxes(
        &self,
        account_id: String,
    ) -> Result<SharedMailboxesView, MailcalError> {
        if !self.registry.contains(&account_id)
            || self.registry.shared_parent(&account_id).is_some()
        {
            return Err(MailcalError::Config("no such account".to_owned()));
        }
        let chosen = self
            .registry
            .shared_mailboxes(&account_id)
            .into_iter()
            .map(|(_, choice)| choice)
            .collect();
        if !self.opens_shared_mailboxes(&account_id) {
            return Ok(view(Ok::<_, ()>(SharedMailboxOffer::Unsupported), chosen));
        }
        let account = AccountId::try_from(account_id.as_str())
            .map_err(|err| MailcalError::Config(err.to_string()))?;
        let app = Arc::clone(&self.app);
        let offer = self
            .runtime
            .block_on(|| async move { app.shared_mailbox_offer(&account).await })
            .inspect_err(|err| {
                log::info!("shared mailboxes: the server did not list them ({err})");
            });
        Ok(view(offer, chosen))
    }

    /// The mailbox at `address`, if `account_id`'s sign-in opens it, ready for
    /// [`MailcalApp::set_shared_mailbox`]; `None` when nothing it can open answers to `address`.
    /// The server will not say whether no such mailbox exists or it was not shared with the
    /// account, so neither does this. Blocks on the network.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Config`] for an account that is not set up or cannot open a shared
    /// mailbox, and [`MailcalError::Connect`] when the account is not connected or the server did
    /// not answer.
    pub fn resolve_shared_mailbox(
        &self,
        account_id: String,
        address: String,
    ) -> Result<Option<SharedMailboxEntry>, MailcalError> {
        if !self.opens_shared_mailboxes(&account_id) {
            return Err(MailcalError::Config(
                "this account opens no shared mailbox".to_owned(),
            ));
        }
        let account = AccountId::try_from(account_id.as_str())
            .map_err(|err| MailcalError::Config(err.to_string()))?;
        let app = Arc::clone(&self.app);
        let address = address.trim().to_owned();
        let lookup = self
            .runtime
            .block_on(|| async move { app.resolve_shared_mailbox(&account, &address).await })
            .map_err(|err| MailcalError::Connect(err.to_string()))?;
        let chosen = self.registry.shared_mailboxes(&account_id);
        match lookup {
            SharedMailboxLookup::Found(mailbox) => Ok(Some(SharedMailboxEntry {
                selected: chosen
                    .iter()
                    .any(|(_, choice)| choice.handle == mailbox.handle),
                handle: mailbox.handle.as_str().to_owned(),
                name: mailbox.name,
                address: mailbox.address,
                listed: true,
            })),
            SharedMailboxLookup::NotOpenable => Ok(None),
            SharedMailboxLookup::NotConnected => Err(MailcalError::Connect(
                "the account is not connected".to_owned(),
            )),
        }
    }

    /// Uses `account_id` for the shared mailbox `mailbox`, or stops using it, and stores the
    /// account. Switched on, the mailbox is listed as a mail account of its own and connected in
    /// the background; switched off, it leaves every mail surface and what the device holds of
    /// it is deleted. Nothing changes on the server.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Config`] for an account that is not set up or a handle that is no
    /// handle, and [`MailcalError::Connect`] when the host's store refused the write.
    pub fn set_shared_mailbox(
        &self,
        account_id: String,
        mailbox: SharedMailboxEntry,
        on: bool,
    ) -> Result<(), MailcalError> {
        let parent = AccountId::try_from(account_id.as_str())
            .map_err(|err| MailcalError::Config(err.to_string()))?;
        let choice = SharedMailboxChoice {
            handle: SharedMailboxId::try_from(mailbox.handle.as_str())
                .map_err(|err| MailcalError::Config(err.to_string()))?,
            name: mailbox.name,
            address: mailbox.address,
        };
        let child = choice
            .account_id(&parent)
            .map_err(|err| MailcalError::Config(err.to_string()))?;
        if self.registry.shared_parent(&account_id).is_some() {
            return Err(MailcalError::Config("no such account".to_owned()));
        }
        // A choice the account cannot open would be stored and listed nowhere.
        if on && !self.opens_shared_mailboxes(&account_id) {
            return Err(MailcalError::Config(
                "this account opens no shared mailbox".to_owned(),
            ));
        }
        if !self
            .registry
            .set_shared_mailbox(&account_id, choice.clone(), on)
        {
            return if self.registry.contains(&account_id) {
                Ok(())
            } else {
                Err(MailcalError::Config("no such account".to_owned()))
            };
        }
        // Stored before anything is opened or deleted, so a refused write changes nothing.
        let stored = match self.registry.config_toml(&account_id) {
            Some(config) => config
                .map_err(MailcalError::Config)
                .and_then(|config| self.persist_config(&account_id, config)),
            None => Ok(()),
        };
        if let Err(err) = stored {
            self.registry.set_shared_mailbox(&account_id, choice, !on);
            return Err(err);
        }
        if on {
            self.open_shared_mailbox(&parent, &child);
        } else {
            self.drop_shared_mailbox(&child);
        }
        self.app.accounts_changed();
        Ok(())
    }
}

impl MailcalApp {
    /// Whether `account_id` is an account whose sign-in may open a shared mailbox: an IMAP or
    /// JMAP account, whose server says, or a Microsoft account used for shared mailboxes with a
    /// grant that allows them. Never a shared mailbox itself.
    fn opens_shared_mailboxes(&self, account_id: &str) -> bool {
        if self.registry.shared_parent(account_id).is_some() {
            return false;
        }
        match self.registry.account_type(account_id) {
            "imap" | "jmap" => true,
            "graph" => self
                .registry
                .dial(account_id)
                .is_some_and(|dial| dial.capabilities().contains(Capability::SharedMailboxes)),
            _ => false,
        }
    }

    /// Lists the shared mailbox `child` of `parent` and dials it in the background.
    fn open_shared_mailbox(&self, parent: &AccountId, child: &AccountId) {
        let Some(placeholder) = self
            .registry
            .shared_placeholders(parent.as_str())
            .into_iter()
            .find(|placeholder| placeholder.id == *child)
        else {
            return;
        };
        let app = Arc::clone(&self.app);
        self.runtime
            .block_on(|| async move { app.add_new_account_deferred(placeholder).await });
        self.disconnected
            .lock()
            .expect("disconnected mutex poisoned")
            .insert(child.as_str().to_owned());
        self.retry_connections();
    }

    /// Dials `parent`'s shared mailboxes again, after its credential was replaced: each was
    /// opened with the one it had before.
    pub(crate) fn redial_shared_mailboxes(&self, parent: &str) {
        let shared = self.registry.shared_mailboxes(parent);
        if shared.is_empty() {
            return;
        }
        self.disconnected
            .lock()
            .expect("disconnected mutex poisoned")
            .extend(
                shared
                    .into_iter()
                    .map(|(child, _)| child.as_str().to_owned()),
            );
        self.retry_connections();
    }

    /// Stops `parent` being used for any shared mailbox, as when the use that opens them is
    /// switched off, and stores it.
    pub(crate) fn drop_shared_mailboxes(&self, parent: &str) -> Result<(), MailcalError> {
        let chosen = self.registry.shared_mailboxes(parent);
        if chosen.is_empty() {
            return Ok(());
        }
        for (child, choice) in chosen {
            self.registry.set_shared_mailbox(parent, choice, false);
            self.drop_shared_mailbox(&child);
        }
        match self.registry.config_toml(parent) {
            Some(config) => self.persist_config(parent, config.map_err(MailcalError::Config)?),
            None => Ok(()),
        }
    }

    /// Takes the shared mailbox `child` out of every surface and deletes what the device holds
    /// of it, as removing an account does.
    pub(crate) fn drop_shared_mailbox(&self, child: &AccountId) {
        self.disconnected
            .lock()
            .expect("disconnected mutex poisoned")
            .remove(child.as_str());
        self.background.apply(child.as_str(), None);
        let (app, child) = (Arc::clone(&self.app), child.clone());
        self.runtime
            .block_on(|| async move { app.remove_account(&child).await });
        self.refresh_mcp();
    }
}

/// The view Settings draws: the chosen mailboxes first, then the rest the server listed.
fn view<E>(
    offer: Result<SharedMailboxOffer, E>,
    chosen: Vec<SharedMailboxChoice>,
) -> SharedMailboxesView {
    let (support, listed) = match offer {
        Ok(SharedMailboxOffer::Listed(listed)) => (SharedMailboxSupport::Listed, listed),
        Ok(SharedMailboxOffer::ByAddress) => (SharedMailboxSupport::ByAddress, Vec::new()),
        Ok(SharedMailboxOffer::Unsupported) => (SharedMailboxSupport::Unsupported, Vec::new()),
        Ok(SharedMailboxOffer::NotConnected) => (SharedMailboxSupport::NotConnected, Vec::new()),
        Err(_) => (SharedMailboxSupport::Unreachable, Vec::new()),
    };
    let is_listed =
        |handle: &SharedMailboxId| listed.iter().any(|mailbox| mailbox.handle == *handle);
    let mut mailboxes: Vec<SharedMailboxEntry> = chosen
        .iter()
        .map(|choice| SharedMailboxEntry {
            handle: choice.handle.as_str().to_owned(),
            name: choice.name.clone(),
            address: choice.address.clone(),
            selected: true,
            // Not connected is not the same as withdrawn: nothing was asked.
            listed: support != SharedMailboxSupport::Listed || is_listed(&choice.handle),
        })
        .collect();
    mailboxes.extend(
        listed
            .iter()
            .filter(|mailbox| !chosen.iter().any(|choice| choice.handle == mailbox.handle))
            .map(|mailbox| SharedMailboxEntry {
                handle: mailbox.handle.as_str().to_owned(),
                name: mailbox.name.clone(),
                address: mailbox.address.clone(),
                selected: false,
                listed: true,
            }),
    );
    SharedMailboxesView { support, mailboxes }
}

#[cfg(test)]
#[path = "app_shared_mailboxes_tests.rs"]
mod tests;
