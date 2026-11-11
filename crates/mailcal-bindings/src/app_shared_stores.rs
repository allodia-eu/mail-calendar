//! Settings → Accounts → an account's shared mailboxes and shared calendars: which stores its
//! sign-in opens besides its own, and which of them it is used for, for each use
//! (`docs/shared-mailboxes.md`).

use std::sync::Arc;

use engine_api::{AccountId, SearchDomain, SharedAccountId};
use mailcal_account::{Capability, SharedStoreChoice, SharedUse};
use mailcal_app::{SharedStoreLookup, SharedStoreOffer};

use crate::{MailcalApp, MailcalError};

/// Whether an account's sign-in can open stores besides its own for a use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SharedStoreSupport {
    /// The server lists the stores it opens; they are in [`SharedStoresView::stores`], none when
    /// nothing is shared with the account.
    Listed,
    /// The server lists none, but opens one at an address the person types
    /// ([`MailcalApp::resolve_shared_store`]); the chosen ones are in
    /// [`SharedStoresView::stores`].
    ByAddress,
    /// The server lists none this app can open.
    Unsupported,
    /// The account's mailbox or calendar is not connected, so there is nothing to ask yet.
    NotConnected,
    /// The server was asked and did not answer.
    Unreachable,
}

/// One store an account's sign-in opens besides its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedStoreEntry {
    /// The server's handle for it, handed back to [`MailcalApp::set_shared_store`].
    pub handle: String,
    /// The server's label for it.
    pub name: String,
    /// The address it is known by, when the server gives one. A shared mailbox sends as it.
    pub address: Option<String>,
    /// Whether the account is used for it: it is then an account of its own.
    pub selected: bool,
    /// Whether the server still lists it. A chosen store the server stopped listing is kept
    /// until it is switched off, so access withdrawn for a moment does not delete it.
    pub listed: bool,
}

/// An account's shared stores for one use, as its server answered and as the account is used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedStoresView {
    /// Whether the sign-in can open any.
    pub support: SharedStoreSupport,
    /// The chosen ones first, in the order they were chosen, then the rest the server listed.
    pub stores: Vec<SharedStoreEntry>,
}

/// The domain a store chosen for `kind` is asked of and opened for.
pub(crate) const fn domain(kind: SharedUse) -> SearchDomain {
    match kind {
        SharedUse::Mail => SearchDomain::Mail,
        SharedUse::Calendar => SearchDomain::Calendar,
    }
}

/// What the account's page asks through its own changes (`AccountPage::shared`); kept public for
/// the live suites, which drive a real server through them.
impl MailcalApp {
    /// Asks `account_id`'s server which stores its sign-in opens besides its own for `kind`.
    /// Blocks on the network.
    ///
    /// The stores the account is used for are in the answer whatever the server said, so one
    /// can be switched off while the server is away.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Config`] for an account that is not set up.
    pub fn shared_stores(
        &self,
        account_id: String,
        kind: SharedUse,
    ) -> Result<SharedStoresView, MailcalError> {
        if !self.registry.contains(&account_id)
            || self.registry.shared_parent(&account_id).is_some()
        {
            return Err(MailcalError::Config("no such account".to_owned()));
        }
        Ok(view(
            self.shared_offer(&account_id, kind),
            self.chosen_shared(&account_id, kind),
        ))
    }

    /// The store at `address` for `kind`, if `account_id`'s sign-in opens it, ready for
    /// [`MailcalApp::set_shared_store`]; `None` when nothing it can open answers to `address`.
    /// The server will not say whether no such store exists or it was not shared with the
    /// account, so neither does this. Blocks on the network.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Config`] for an account that is not set up or cannot open a shared
    /// store for `kind`, and [`MailcalError::Connect`] when the account is not connected or the
    /// server did not answer.
    pub fn resolve_shared_store(
        &self,
        account_id: String,
        kind: SharedUse,
        address: String,
    ) -> Result<Option<SharedStoreEntry>, MailcalError> {
        if !self.opens_shared(&account_id, kind) {
            return Err(MailcalError::Config(
                "this account opens no shared store for this use".to_owned(),
            ));
        }
        let account = AccountId::try_from(account_id.as_str())
            .map_err(|err| MailcalError::Config(err.to_string()))?;
        let app = Arc::clone(&self.app);
        let address = address.trim().to_owned();
        let lookup = self
            .runtime
            .block_on(|| async move {
                app.resolve_shared_store(&account, domain(kind), &address)
                    .await
            })
            .map_err(|err| MailcalError::Connect(err.to_string()))?;
        let chosen = self.registry.shared_stores(&account_id, kind);
        match lookup {
            SharedStoreLookup::Found(store) => Ok(Some(SharedStoreEntry {
                selected: chosen
                    .iter()
                    .any(|(_, choice)| choice.handle == store.handle),
                handle: store.handle.as_str().to_owned(),
                name: store.name,
                address: store.address,
                listed: true,
            })),
            SharedStoreLookup::NotOpenable => Ok(None),
            SharedStoreLookup::NotConnected => Err(MailcalError::Connect(
                "the account is not connected".to_owned(),
            )),
        }
    }

    /// Uses `account_id` for the store `store` for `kind`, or stops using it, and stores the
    /// account. Switched on, the store is listed as an account of its own for that use and
    /// connected in the background; switched off, it leaves every surface and what the device
    /// holds of it is deleted. Nothing changes on the server.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Config`] for an account that is not set up or a handle that is no
    /// handle, and [`MailcalError::Connect`] when the host's store refused the write.
    pub fn set_shared_store(
        &self,
        account_id: String,
        kind: SharedUse,
        store: SharedStoreEntry,
        on: bool,
    ) -> Result<(), MailcalError> {
        let parent = AccountId::try_from(account_id.as_str())
            .map_err(|err| MailcalError::Config(err.to_string()))?;
        let choice = SharedStoreChoice {
            handle: SharedAccountId::try_from(store.handle.as_str())
                .map_err(|err| MailcalError::Config(err.to_string()))?,
            name: store.name,
            address: store.address,
        };
        let child = choice
            .account_id(&parent, kind)
            .map_err(|err| MailcalError::Config(err.to_string()))?;
        if self.registry.shared_parent(&account_id).is_some() {
            return Err(MailcalError::Config("no such account".to_owned()));
        }
        // A choice the account cannot open would be stored and listed nowhere.
        if on && !self.opens_shared(&account_id, kind) {
            return Err(MailcalError::Config(
                "this account opens no shared store for this use".to_owned(),
            ));
        }
        if !self
            .registry
            .set_shared_store(&account_id, kind, choice.clone(), on)
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
            self.registry
                .set_shared_store(&account_id, kind, choice, !on);
            return Err(err);
        }
        if on {
            self.open_shared_store(&parent, &child);
        } else {
            self.drop_shared_store(&child);
        }
        self.app.accounts_changed();
        Ok(())
    }
}

impl MailcalApp {
    /// The stores `account_id` is used for for `kind`, in the order they were chosen.
    pub(crate) fn chosen_shared(
        &self,
        account_id: &str,
        kind: SharedUse,
    ) -> Vec<SharedStoreChoice> {
        self.registry
            .shared_stores(account_id, kind)
            .into_iter()
            .map(|(_, choice)| choice)
            .collect()
    }

    /// What `account_id`'s server says of the stores its sign-in opens for `kind`: asked of the
    /// server only for an account that may open one. **Blocking** on the network.
    pub(crate) fn shared_offer(
        &self,
        account_id: &str,
        kind: SharedUse,
    ) -> Result<SharedStoreOffer, ()> {
        if !self.opens_shared(account_id, kind) {
            return Ok(SharedStoreOffer::Unsupported);
        }
        let Ok(account) = AccountId::try_from(account_id) else {
            return Ok(SharedStoreOffer::Unsupported);
        };
        let app = Arc::clone(&self.app);
        self.runtime
            .block_on(|| async move { app.shared_store_offer(&account, domain(kind)).await })
            .map_err(|err| {
                log::info!("shared stores: the server did not list them ({err})");
            })
    }

    /// Whether `account_id`'s sign-in may open a store besides its own for `kind`. For mail: an
    /// IMAP or JMAP account, whose server says, or a Microsoft account used for shared mailboxes
    /// with a grant that allows them. For calendars: a JMAP account, or an IMAP account with a
    /// CalDAV server, whose server says. Never a shared store itself, and never for a use the
    /// account is not used for: a store is shared with a mailbox or a calendar, and an account
    /// without one would wait forever for it to connect.
    pub(crate) fn opens_shared(&self, account_id: &str, kind: SharedUse) -> bool {
        if self.registry.shared_parent(account_id).is_some() {
            return false;
        }
        let Some(dial) = self.registry.dial(account_id) else {
            return false;
        };
        let uses = dial.capabilities();
        match kind {
            SharedUse::Mail if !uses.contains(Capability::Mail) => false,
            SharedUse::Calendar if !uses.contains(Capability::Calendar) => false,
            SharedUse::Mail => match &dial {
                crate::account_registry::AccountDial::Imap { .. }
                | crate::account_registry::AccountDial::Jmap { .. } => true,
                crate::account_registry::AccountDial::Microsoft { .. } => {
                    uses.contains(Capability::SharedMailboxes)
                }
                crate::account_registry::AccountDial::Google { .. } => false,
            },
            SharedUse::Calendar => match &dial {
                crate::account_registry::AccountDial::Imap { config, .. } => {
                    config.caldav.is_some()
                }
                crate::account_registry::AccountDial::Jmap { .. } => true,
                crate::account_registry::AccountDial::Microsoft { .. }
                | crate::account_registry::AccountDial::Google { .. } => false,
            },
        }
    }

    /// Lists the shared store `child` of `parent` and dials it in the background.
    fn open_shared_store(&self, parent: &AccountId, child: &AccountId) {
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

    /// Dials `parent`'s shared stores again, after its credential was replaced: each was opened
    /// with the one it had before.
    pub(crate) fn redial_shared_stores(&self, parent: &str) {
        let shared = self.registry.shared_children(parent);
        if shared.is_empty() {
            return;
        }
        self.disconnected
            .lock()
            .expect("disconnected mutex poisoned")
            .extend(shared.into_iter().map(|child| child.id.as_str().to_owned()));
        self.retry_connections();
    }

    /// Stops `parent` being used for any store for `kind`, as when the use that opens them is
    /// switched off, and stores it.
    pub(crate) fn drop_shared_stores(
        &self,
        parent: &str,
        kind: SharedUse,
    ) -> Result<(), MailcalError> {
        let chosen = self.registry.shared_stores(parent, kind);
        if chosen.is_empty() {
            return Ok(());
        }
        for (child, choice) in chosen {
            self.registry.set_shared_store(parent, kind, choice, false);
            self.drop_shared_store(&child);
        }
        match self.registry.config_toml(parent) {
            Some(config) => self.persist_config(parent, config.map_err(MailcalError::Config)?),
            None => Ok(()),
        }
    }

    /// Takes the shared store `child` out of every surface and deletes what the device holds of
    /// it, as removing an account does.
    pub(crate) fn drop_shared_store(&self, child: &AccountId) {
        self.disconnected
            .lock()
            .expect("disconnected mutex poisoned")
            .remove(child.as_str());
        self.background.apply(child.as_str(), None);
        let (app, child) = (Arc::clone(&self.app), child.clone());
        self.runtime
            .block_on(|| async move { app.remove_account(&child).await });
        self.refresh_mcp();
        let app = Arc::clone(&self.app);
        self.runtime
            .spawn(async move { app.reclaim_removed_account_space().await });
    }
}

/// The view Settings draws: the chosen stores first, then the rest the server listed.
pub(crate) fn view<E>(
    offer: Result<SharedStoreOffer, E>,
    chosen: Vec<SharedStoreChoice>,
) -> SharedStoresView {
    let (support, listed) = match offer {
        Ok(SharedStoreOffer::Listed(listed)) => (SharedStoreSupport::Listed, listed),
        Ok(SharedStoreOffer::ByAddress) => (SharedStoreSupport::ByAddress, Vec::new()),
        Ok(SharedStoreOffer::Unsupported) => (SharedStoreSupport::Unsupported, Vec::new()),
        Ok(SharedStoreOffer::NotConnected) => (SharedStoreSupport::NotConnected, Vec::new()),
        Err(_) => (SharedStoreSupport::Unreachable, Vec::new()),
    };
    let is_listed = |handle: &SharedAccountId| listed.iter().any(|store| store.handle == *handle);
    let mut stores: Vec<SharedStoreEntry> = chosen
        .iter()
        .map(|choice| SharedStoreEntry {
            handle: choice.handle.as_str().to_owned(),
            name: choice.name.clone(),
            address: choice.address.clone(),
            selected: true,
            // Not connected is not the same as withdrawn: nothing was asked.
            listed: support != SharedStoreSupport::Listed || is_listed(&choice.handle),
        })
        .collect();
    stores.extend(
        listed
            .iter()
            .filter(|store| !chosen.iter().any(|choice| choice.handle == store.handle))
            .map(|store| SharedStoreEntry {
                handle: store.handle.as_str().to_owned(),
                name: store.name.clone(),
                address: store.address.clone(),
                selected: false,
                listed: true,
            }),
    );
    SharedStoresView { support, stores }
}

#[cfg(test)]
#[path = "app_shared_stores_tests.rs"]
mod tests;
