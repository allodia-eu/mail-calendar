//! Per-account configuration storage in Secret Service (or oo7's Flatpak backend).

use std::{collections::HashMap, future::Future, sync::Arc};

use mailcal_bindings::{AccountCredentialStore, CredentialStoreError};
use oo7::{Item, Keyring};
use tokio::runtime::Runtime;

use crate::{host_runtime, secret_encoding};

/// The `application` attribute every stored item is tagged with, so this app's secrets are
/// distinguishable from another's in the same keyring. It is the application id, which a re-branded
/// build changes; and must, or two builds would claim each other's items (docs/branding.md).
const APPLICATION: &str = crate::l10n::APP_ID;
const INDEX_KIND: &str = "account-index";
const ACCOUNT_KIND: &str = "account";

/// What [`SecretStore::load`] read.
#[derive(Debug, Default)]
pub(crate) struct StoredAccounts {
    /// Every stored config, for the core to open or leave out.
    pub(crate) configs: Vec<String>,
    /// The accounts whose config a plain-text keyring cut short, beyond recovery.
    pub(crate) cut_short: Vec<String>,
}

/// One secure item per account plus an ordered account-id index.
pub(crate) struct SecretStore {
    /// Borrowed, never owned: inside a sandbox `oo7` fetches the keyring key through the portal,
    /// and that connection is process-global, so a store that owned its runtime would kill every
    /// later portal call by failing to open ([`crate::host_runtime`]).
    runtime: &'static Runtime,
    keyring: Keyring,
    /// Which set of items this store may see, or `None` for the real one.
    ///
    /// A debug launch that connects a canned account gets its own namespace, so a sign-in made
    /// while testing is kept without ever landing among the developer's real accounts; the shape
    /// the Windows client uses. `None` adds **no** attribute at all rather than a default one:
    /// every item already in a keyring was written without it, and searching for one that carries
    /// it would match none of them, which reads as every account having vanished.
    namespace: Option<String>,
    /// See [`Self::lock_index`].
    index_lock: std::sync::Mutex<()>,
}

/// Writes an account's credential through the same encrypted per-account item the store reads
/// from, for every family; the core keys on the account id and never branches on the provider.
#[derive(Debug)]
pub(crate) struct SecretSink {
    store: Arc<SecretStore>,
}

impl SecretSink {
    pub(crate) fn new(store: Arc<SecretStore>) -> Self {
        Self { store }
    }
}

impl AccountCredentialStore for SecretSink {
    fn persist(&self, account_id: String, config_toml: String) -> Result<(), CredentialStoreError> {
        // The message carries the store's reason, never the config or the token itself.
        self.store
            .save(&account_id, &config_toml)
            .map_err(CredentialStoreError::Store)
    }

    fn delete(&self, account_id: String) -> Result<(), CredentialStoreError> {
        self.store
            .remove(&account_id)
            .map_err(CredentialStoreError::Store)
    }
}

impl std::fmt::Debug for SecretStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SecretStore")
            .finish_non_exhaustive()
    }
}

impl SecretStore {
    /// Opens the native keyring on the real accounts.
    ///
    /// A failure is surfaced because falling back to plaintext would violate the account-storage
    /// contract.
    pub(crate) fn open() -> Result<Self, String> {
        Self::open_namespace(None)
    }

    /// Opens the keyring on a **debug** namespace, keyed by the dev account's own name.
    ///
    /// Items written here carry a `namespace` attribute no real item has, so the two sets cannot
    /// see each other in either direction: a harness sign-in is kept across launches, and the
    /// developer's own accounts are neither read nor written by a run that connects a fixture.
    #[cfg(debug_assertions)]
    pub(crate) fn open_dev(namespace: &str) -> Result<Self, String> {
        Self::open_namespace(Some(namespace.to_owned()))
    }

    fn open_namespace(namespace: Option<String>) -> Result<Self, String> {
        let runtime = host_runtime::shared()
            .ok_or_else(|| "no host runtime to open the keyring on".to_owned())?;
        let keyring =
            block_on_runtime(runtime, Keyring::new())?.map_err(|error| error.to_string())?;
        // `oo7` never unlocks on its own. A locked collection still answers `search_items`, but
        // every `GetSecret` on the items it returns fails: so an app started into a session
        // whose login keyring is locked (autologin, a separate keyring password, an idle lock)
        // would fail to read its own accounts and land on an opaque boot error with no way
        // forward. Unlocking here hands the ask to the desktop's keyring agent instead.
        block_on_runtime(runtime, keyring.unlock())?.map_err(|error| error.to_string())?;
        Ok(Self {
            runtime,
            keyring,
            namespace,
            index_lock: std::sync::Mutex::default(),
        })
    }

    /// This store's attributes for one account, and for the index.
    ///
    /// Built here rather than free-standing so the namespace cannot be forgotten at a call site:
    /// a read that dropped it would answer with the real accounts.
    fn account_attributes<'a>(&'a self, account_id: &'a str) -> HashMap<&'a str, &'a str> {
        let mut attributes = HashMap::from([
            ("application", APPLICATION),
            ("kind", ACCOUNT_KIND),
            ("account", account_id),
        ]);
        self.tag(&mut attributes);
        attributes
    }

    fn index_attributes(&self) -> HashMap<&str, &str> {
        let mut attributes = HashMap::from([("application", APPLICATION), ("kind", INDEX_KIND)]);
        self.tag(&mut attributes);
        attributes
    }

    fn tag<'a>(&'a self, attributes: &mut HashMap<&'a str, &'a str>) {
        if let Some(namespace) = &self.namespace {
            attributes.insert("namespace", namespace);
        }
    }

    /// What a person reading their keyring sees, so a debug item is recognisable as one.
    fn label(&self, suffix: &str) -> String {
        let app = crate::l10n::app_title();
        match &self.namespace {
            Some(namespace) => format!("{app} ({namespace}): {suffix}"),
            None => format!("{app}: {suffix}"),
        }
    }

    #[cfg(any(test, debug_assertions))]
    pub(crate) fn configs(&self) -> Result<Vec<String>, String> {
        self.load().map(|stored| stored.configs)
    }

    /// Every stored config in index order, and the ids of those a plain-text keyring cut short.
    ///
    /// An item that cannot be read or decoded is handed on as an empty config rather than failing
    /// the whole read: the core leaves out and logs that one account, and the rest still load.
    pub(crate) fn load(&self) -> Result<StoredAccounts, String> {
        let mut stored = StoredAccounts::default();
        for id in self.read_index()? {
            let raw = match self.read_raw(&self.account_attributes(&id)) {
                Ok(Some(raw)) => raw,
                Ok(None) => continue,
                Err(_) => Vec::new(),
            };
            let cut_short = secret_encoding::is_cut_short(&raw);
            let config = secret_encoding::decode_config(raw);
            // Removed only when the core itself would leave it out, not on the bytes alone.
            if cut_short && !mailcal_bindings::stored_account_is_readable(config.clone()) {
                stored.cut_short.push(id);
            }
            stored.configs.push(config);
        }
        Ok(stored)
    }

    /// Removes the accounts [`Self::load`] found cut short, after the launch that reported them.
    ///
    /// Their rest is gone for good, so keeping them would only report the same loss at every
    /// launch.
    pub(crate) fn forget_cut_short(&self, ids: &[String]) -> Result<(), String> {
        let _index = self.lock_index();
        for id in ids {
            block_on_runtime(
                self.runtime,
                self.keyring.delete(&self.account_attributes(id)),
            )?
            .map_err(|error| error.to_string())?;
        }
        let kept = self
            .read_index()?
            .into_iter()
            .filter(|id| !ids.contains(id))
            .collect::<Vec<_>>();
        self.write_index(&kept)
    }

    /// Held across every read-modify-write of the index: the core saves and removes accounts from
    /// its own threads while boot may be removing others.
    fn lock_index(&self) -> std::sync::MutexGuard<'_, ()> {
        self.index_lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub(crate) fn save(&self, account_id: &str, config: &str) -> Result<(), String> {
        let _index = self.lock_index();
        self.write_secret(
            &self.label(account_id),
            &self.account_attributes(account_id),
            config.as_bytes(),
        )?;
        let mut ids = self.read_index()?;
        if !ids.iter().any(|id| id == account_id) {
            ids.push(account_id.to_owned());
            self.write_index(&ids)?;
        }
        Ok(())
    }

    pub(crate) fn remove(&self, account_id: &str) -> Result<(), String> {
        let _index = self.lock_index();
        block_on_runtime(
            self.runtime,
            self.keyring.delete(&self.account_attributes(account_id)),
        )?
        .map_err(|error| error.to_string())?;
        let ids = self
            .read_index()?
            .into_iter()
            .filter(|id| id != account_id)
            .collect::<Vec<_>>();
        self.write_index(&ids)
    }

    fn read_index(&self) -> Result<Vec<String>, String> {
        let Some(raw) = self.read_secret(&self.index_attributes())? else {
            return Ok(Vec::new());
        };
        serde_json::from_str(&raw).map_err(|error| error.to_string())
    }

    fn write_index(&self, ids: &[String]) -> Result<(), String> {
        let raw = serde_json::to_vec(ids).map_err(|error| error.to_string())?;
        self.write_secret(&self.label("account index"), &self.index_attributes(), &raw)
    }

    fn read_secret(&self, attributes: &HashMap<&str, &str>) -> Result<Option<String>, String> {
        let Some(secret) = self.read_raw(attributes)? else {
            return Ok(None);
        };
        String::from_utf8(secret_encoding::decode(secret)?)
            .map(Some)
            .map_err(|error| error.to_string())
    }

    /// The item's value exactly as the keyring holds it.
    fn read_raw(&self, attributes: &HashMap<&str, &str>) -> Result<Option<Vec<u8>>, String> {
        let items = block_on_runtime(self.runtime, self.keyring.search_items(attributes))?
            .map_err(|error| error.to_string())?;
        let Some(item) = items.first() else {
            return Ok(None);
        };
        block_on_runtime(self.runtime, item_secret(item))?
            .map(Some)
            .map_err(|error| error.to_string())
    }

    fn write_secret(
        &self,
        label: &str,
        attributes: &HashMap<&str, &str>,
        secret: &[u8],
    ) -> Result<(), String> {
        block_on_runtime(
            self.runtime,
            self.keyring
                .create_item(label, attributes, secret_encoding::encode(secret), true),
        )?
        .map_err(|error| error.to_string())
    }
}

/// Runs a Secret Service future from both ordinary host threads and the core's Tokio workers.
/// Tokio rejects nesting one runtime's `block_on` inside another, so the rare rotation callback
/// crosses a scoped OS thread and remains synchronous before reporting persistence complete.
fn block_on_runtime<F>(runtime: &Runtime, future: F) -> Result<F::Output, String>
where
    F: Future + Send,
    F::Output: Send,
{
    if tokio::runtime::Handle::try_current().is_err() {
        return Ok(runtime.block_on(future));
    }
    std::thread::scope(|scope| {
        scope
            .spawn(move || runtime.block_on(future))
            .join()
            .map_err(|_| "secure-store runtime worker failed".to_owned())
    })
}

async fn item_secret(item: &Item) -> oo7::Result<Vec<u8>> {
    Ok(item.secret().await?.as_bytes().to_vec())
}

#[cfg(test)]
#[path = "secrets_tests.rs"]
mod tests;
