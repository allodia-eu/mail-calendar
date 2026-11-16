//! Settings → Accounts → an account's shared stores: which stores its sign-in opens besides its
//! own, what each holds, and which of them it is used for (`docs/shared-mailboxes.md`).

use std::sync::Arc;

use engine_api::{AccountId, SharedAccount, SharedAccountId};
use mailcal_account::{SharedStoreChoice, SharedUse, SharedUses};
use mailcal_app::{SharedStoreLookup, SharedStoreOffer};

use crate::{MailcalApp, MailcalError};

/// Whether an account's sign-in can open stores besides its own.
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
    /// What it holds of what the account opens: what switching it on opens.
    pub holds: SharedUses,
    /// Whether the account is used for it: it is then an account of its own.
    pub selected: bool,
    /// Whether the server still lists it. A chosen store the server stopped listing is kept
    /// until it is switched off, so access withdrawn for a moment does not delete it.
    pub listed: bool,
}

/// An account's shared stores, as its server answered and as the account is used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedStoresView {
    /// Whether the sign-in can open any.
    pub support: SharedStoreSupport,
    /// The chosen ones first, in the order they were chosen, then the rest the server listed.
    pub stores: Vec<SharedStoreEntry>,
}

/// What the account's page asks through its own changes (`AccountPage::shared`); kept public for
/// the live suites, which drive a real server through them.
impl MailcalApp {
    /// Asks `account_id`'s server which stores its sign-in opens besides its own. Blocks on the
    /// network.
    ///
    /// The stores the account is used for are in the answer whatever the server said, so one
    /// can be switched off while the server is away.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Config`] for an account that is not set up.
    pub fn shared_stores(&self, account_id: String) -> Result<SharedStoresView, MailcalError> {
        if !self.registry.contains(&account_id)
            || self.registry.shared_parent(&account_id).is_some()
        {
            return Err(MailcalError::Config("no such account".to_owned()));
        }
        Ok(view(
            self.shared_offer(&account_id),
            self.shown_shared(&account_id),
        ))
    }

    /// The store at `address`, if `account_id`'s sign-in opens it, ready for
    /// [`MailcalApp::set_shared_store`]; `None` when nothing it can open answers to `address`.
    /// The server will not say whether no such store exists or it was not shared with the
    /// account, so neither does this. Blocks on the network.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Config`] for an account that is not set up or opens no shared
    /// store, and [`MailcalError::Connect`] when the account is not connected or the server did
    /// not answer.
    pub fn resolve_shared_store(
        &self,
        account_id: String,
        address: String,
    ) -> Result<Option<SharedStoreEntry>, MailcalError> {
        let uses = self.shared_uses(&account_id);
        if uses.is_empty() {
            return Err(MailcalError::Config(
                "this account opens no shared store".to_owned(),
            ));
        }
        let account = AccountId::try_from(account_id.as_str())
            .map_err(|err| MailcalError::Config(err.to_string()))?;
        let app = Arc::clone(&self.app);
        let address = address.trim().to_owned();
        let lookup = self
            .runtime
            .block_on(|| async move { app.resolve_shared_store(&account, uses, &address).await })
            .map_err(|err| MailcalError::Connect(err.to_string()))?;
        match lookup {
            SharedStoreLookup::Found(store) => {
                let selected = self
                    .chosen_shared(&account_id)
                    .iter()
                    .any(|choice| choice.handle == store.handle);
                Ok(Some(entry(&store, selected)))
            }
            SharedStoreLookup::NotOpenable => Ok(None),
            SharedStoreLookup::NotConnected => Err(MailcalError::Connect(
                "the account is not connected".to_owned(),
            )),
        }
    }

    /// Uses `account_id` for the store `store`, or stops using it, and stores the account.
    /// Switched on, the store is listed as an account of its own holding what `store` holds and
    /// connected in the background; switched on again, it gains what it holds now; switched off,
    /// it leaves every surface and what the device holds of it is deleted. Nothing changes on the
    /// server.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Config`] for an account that is not set up or a handle that is no
    /// handle, and [`MailcalError::Connect`] when the host's store refused the write.
    pub fn set_shared_store(
        &self,
        account_id: String,
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
            holds: store.holds,
        };
        let child = choice
            .account_id(&parent)
            .map_err(|err| MailcalError::Config(err.to_string()))?;
        if self.registry.shared_parent(&account_id).is_some() {
            return Err(MailcalError::Config("no such account".to_owned()));
        }
        // A choice the account cannot open would be stored and listed nowhere.
        if on
            && choice
                .holds
                .intersect(self.shared_uses(&account_id))
                .is_empty()
        {
            return Err(MailcalError::Config(
                "this account opens nothing this store holds".to_owned(),
            ));
        }
        let was_chosen = self.registry.shared_parent(child.as_str()).is_some();
        let before = self.chosen_shared(&account_id);
        if !self.registry.set_shared_store(&account_id, choice, on) {
            return if self.registry.contains(&account_id) {
                Ok(())
            } else {
                Err(MailcalError::Config("no such account".to_owned()))
            };
        }
        // Stored before anything is opened or deleted, so a refused write changes nothing.
        if let Err(err) = self.store_shared_choices(&account_id) {
            self.registry.restore_shared_stores(&account_id, before);
            return Err(err);
        }
        match (on, was_chosen) {
            (true, false) => self.open_shared_store(&child),
            // It holds more than it did: dialled again to open it.
            (true, true) => self.redial(child.as_str()),
            (false, _) => self.drop_shared_store(&child),
        }
        self.app.accounts_changed();
        Ok(())
    }
}

impl MailcalApp {
    /// The stores `account_id` is used for, in the order they were chosen.
    pub(crate) fn chosen_shared(&self, account_id: &str) -> Vec<SharedStoreChoice> {
        self.registry
            .shared_children(account_id)
            .into_iter()
            .map(|child| child.choice)
            .collect()
    }

    /// The stores `account_id` is used for, each holding what the account opens of it now, as
    /// its row says: a store holding mail and calendars holds only mail once the account's
    /// calendar is switched off. One the account opens nothing of keeps what it was stored with,
    /// so its row is still drawn and can be switched off.
    pub(crate) fn shown_shared(&self, account_id: &str) -> Vec<SharedStoreChoice> {
        shown(self.chosen_shared(account_id), self.shared_uses(account_id))
    }

    /// What `account_id`'s server says of the stores its sign-in opens: asked of the server only
    /// for an account that may open one. A store it is used for that the server now lists as
    /// holding more than it did gains it here, as a store found by its address does when added
    /// again. **Blocking** on the network.
    pub(crate) fn shared_offer(&self, account_id: &str) -> Result<SharedStoreOffer, ()> {
        let uses = self.shared_uses(account_id);
        if uses.is_empty() {
            return Ok(SharedStoreOffer::Unsupported);
        }
        let Ok(account) = AccountId::try_from(account_id) else {
            return Ok(SharedStoreOffer::Unsupported);
        };
        let app = Arc::clone(&self.app);
        let offer = self
            .runtime
            .block_on(|| async move { app.shared_store_offer(&account, uses).await })
            .map_err(|err| {
                log::info!("shared stores: the server did not list them ({err})");
            })?;
        if let SharedStoreOffer::Listed { stores, unanswered } = &offer {
            if !unanswered.is_empty() {
                log::info!("shared stores: a server did not list them ({unanswered:?})");
            }
            self.gain_listed(account_id, stores);
        }
        Ok(offer)
    }

    /// Switches each store `account_id` is used for that `listed` says holds more on again, so it
    /// opens what it holds now. A store stored as a shared mailbox gains its calendars this way.
    fn gain_listed(&self, account_id: &str, listed: &[SharedAccount]) {
        for choice in self.chosen_shared(account_id) {
            let Some(store) = listed.iter().find(|store| store.handle == choice.handle) else {
                continue;
            };
            if SharedUses::held_by(store).union(choice.holds) != choice.holds {
                let gained = entry(store, true);
                if let Err(err) = self.set_shared_store(account_id.to_owned(), gained, true) {
                    log::info!("shared stores: what a store holds now was not stored ({err})");
                }
            }
        }
    }

    /// What the stores `account_id`'s sign-in opens besides its own are opened for: nothing
    /// for a shared store itself, or an account that is not set up.
    pub(crate) fn shared_uses(&self, account_id: &str) -> SharedUses {
        if self.registry.shared_parent(account_id).is_some() {
            return SharedUses::default();
        }
        self.registry
            .dial(account_id)
            .map(|dial| dial.shared_uses())
            .unwrap_or_default()
    }

    /// Whether `account_id`'s sign-in may open any store besides its own.
    pub(crate) fn opens_shared(&self, account_id: &str) -> bool {
        !self.shared_uses(account_id).is_empty()
    }

    /// Writes `account_id`'s stored config, with the shared stores as the registry holds them.
    fn store_shared_choices(&self, account_id: &str) -> Result<(), MailcalError> {
        match self.registry.config_toml(account_id) {
            Some(config) => self.persist_config(account_id, config.map_err(MailcalError::Config)?),
            None => Ok(()),
        }
    }

    /// Dials the shared store `child` again, as it is stored now.
    fn redial(&self, child: &str) {
        self.disconnected
            .lock()
            .expect("disconnected mutex poisoned")
            .insert(child.to_owned());
        self.retry_connections();
    }

    /// Lists the shared store `child` and dials it in the background.
    fn open_shared_store(&self, child: &AccountId) {
        let Some(placeholder) = self.registry.shared_placeholder(child.clone()) else {
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

    /// `parent` stopped opening `kind`, as when the use that opens it is switched off: each store
    /// holding `kind` keeps the rest of what it holds, with what the device holds of `kind`
    /// forgotten, and one left opening nothing is no longer used. Stores the account. A store
    /// that stays is queued to be dialled, which the caller's dial of `parent` does.
    pub(crate) fn drop_shared_use(
        &self,
        parent: &str,
        kind: SharedUse,
    ) -> Result<(), MailcalError> {
        let mut dropped = false;
        for child in self.registry.shared_children(parent) {
            if !child.choice.holds.contains(kind) {
                continue;
            }
            if self.registry.dial(child.id.as_str()).is_some() {
                self.list_without_providers(&child.id);
                let (app, id) = (Arc::clone(&self.app), child.id.clone());
                self.runtime.block_on(|| async move {
                    app.forget_account_domain(&id, kind.domain()).await;
                });
                // Queued, not dialled: the parent is dialled again once its use changed, and its
                // dial takes this store with it, on the connections it makes.
                self.disconnected
                    .lock()
                    .expect("disconnected mutex poisoned")
                    .insert(child.id.as_str().to_owned());
            } else {
                self.registry.set_shared_store(parent, child.choice, false);
                self.drop_shared_store(&child.id);
                dropped = true;
            }
        }
        if dropped {
            self.store_shared_choices(parent)
        } else {
            Ok(())
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

/// `store` as Settings draws it.
fn entry(store: &SharedAccount, selected: bool) -> SharedStoreEntry {
    SharedStoreEntry {
        handle: store.handle.as_str().to_owned(),
        name: store.name.clone(),
        address: store.address.clone(),
        holds: SharedUses::held_by(store),
        selected,
        listed: true,
    }
}

/// `chosen`, each holding what `opens` opens of it, or what it was stored with when that is
/// nothing.
pub(crate) fn shown(chosen: Vec<SharedStoreChoice>, opens: SharedUses) -> Vec<SharedStoreChoice> {
    chosen
        .into_iter()
        .map(|mut choice| {
            let now = choice.holds.intersect(opens);
            if !now.is_empty() {
                choice.holds = now;
            }
            choice
        })
        .collect()
}

/// The view Settings draws: the chosen stores first, then the rest the server listed. A chosen
/// store the server stopped listing is drawn as withdrawn, unless the server for one of the
/// uses it holds did not answer, which says nothing of it.
pub(crate) fn view<E>(
    offer: Result<SharedStoreOffer, E>,
    chosen: Vec<SharedStoreChoice>,
) -> SharedStoresView {
    let none = SharedUses::default();
    let (support, listed, unanswered) = match offer {
        Ok(SharedStoreOffer::Listed { stores, unanswered }) => {
            (SharedStoreSupport::Listed, stores, unanswered)
        }
        Ok(SharedStoreOffer::ByAddress) => (SharedStoreSupport::ByAddress, Vec::new(), none),
        Ok(SharedStoreOffer::Unsupported) => (SharedStoreSupport::Unsupported, Vec::new(), none),
        Ok(SharedStoreOffer::NotConnected) => (SharedStoreSupport::NotConnected, Vec::new(), none),
        Err(_) => (SharedStoreSupport::Unreachable, Vec::new(), none),
    };
    let is_listed = |handle: &SharedAccountId| listed.iter().any(|store| store.handle == *handle);
    let mut stores: Vec<SharedStoreEntry> = chosen
        .iter()
        .map(|choice| SharedStoreEntry {
            handle: choice.handle.as_str().to_owned(),
            name: choice.name.clone(),
            address: choice.address.clone(),
            holds: choice.holds,
            selected: true,
            // Not connected is not the same as withdrawn: nothing was asked.
            listed: support != SharedStoreSupport::Listed
                || is_listed(&choice.handle)
                || !choice.holds.intersect(unanswered).is_empty(),
        })
        .collect();
    stores.extend(
        listed
            .iter()
            .filter(|store| !chosen.iter().any(|choice| choice.handle == store.handle))
            .map(|store| entry(store, false)),
    );
    SharedStoresView { support, stores }
}

#[cfg(test)]
#[path = "app_shared_stores_tests.rs"]
mod tests;
