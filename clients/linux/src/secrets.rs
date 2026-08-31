//! Per-account configuration storage in Secret Service (or oo7's Flatpak backend).

use std::{collections::HashMap, future::Future, sync::Arc};

use mailcal_bindings::{AccountCredentialStore, CredentialStoreError};
use oo7::{Item, Keyring};
use tokio::runtime::{Builder, Runtime};

/// The `application` attribute every stored item is tagged with, so this app's secrets are
/// distinguishable from another's in the same keyring. It is the application id, which a re-branded
/// build changes; and must, or two builds would claim each other's items (docs/branding.md).
const APPLICATION: &str = crate::l10n::APP_ID;
const INDEX_KIND: &str = "account-index";
const ACCOUNT_KIND: &str = "account";

/// One secure item per account plus an ordered account-id index.
pub(crate) struct SecretStore {
    runtime: Runtime,
    keyring: Keyring,
    /// Which set of items this store may see, or `None` for the real one.
    ///
    /// A debug launch that connects a canned account gets its own namespace, so a sign-in made
    /// while testing is kept without ever landing among the developer's real accounts; the shape
    /// the Windows client uses. `None` adds **no** attribute at all rather than a default one:
    /// every item already in a keyring was written without it, and searching for one that carries
    /// it would match none of them, which reads as every account having vanished.
    namespace: Option<String>,
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
        let runtime = Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .map_err(|error| error.to_string())?;
        let keyring =
            block_on_runtime(&runtime, Keyring::new())?.map_err(|error| error.to_string())?;
        // `oo7` never unlocks on its own. A locked collection still answers `search_items`, but
        // every `GetSecret` on the items it returns fails: so an app started into a session
        // whose login keyring is locked (autologin, a separate keyring password, an idle lock)
        // would fail to read its own accounts and land on an opaque boot error with no way
        // forward. Unlocking here hands the ask to the desktop's keyring agent instead.
        block_on_runtime(&runtime, keyring.unlock())?.map_err(|error| error.to_string())?;
        Ok(Self {
            runtime,
            keyring,
            namespace,
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
        match &self.namespace {
            Some(namespace) => format!("Allodia Mail & Calendar ({namespace}): {suffix}"),
            None => format!("Allodia Mail & Calendar: {suffix}"),
        }
    }

    pub(crate) fn configs(&self) -> Result<Vec<String>, String> {
        let ids = self.read_index()?;
        ids.into_iter()
            .filter_map(|id| self.read_secret(&self.account_attributes(&id)).transpose())
            .collect()
    }

    pub(crate) fn save(&self, account_id: &str, config: &str) -> Result<(), String> {
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
        block_on_runtime(
            &self.runtime,
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
        let items = block_on_runtime(&self.runtime, self.keyring.search_items(attributes))?
            .map_err(|error| error.to_string())?;
        let Some(item) = items.first() else {
            return Ok(None);
        };
        let secret = block_on_runtime(&self.runtime, item_secret(item))?
            .map_err(|error| error.to_string())?;
        String::from_utf8(secret)
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
            &self.runtime,
            self.keyring.create_item(label, attributes, secret, true),
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
mod tests {
    use std::{
        fs,
        sync::Arc,
        time::{SystemTime, UNIX_EPOCH},
    };

    use mailcal_bindings::AccountCredentialStore;
    use oo7::{Keyring, Secret, file};
    use tokio::{runtime::Builder, sync::RwLock};

    use super::{ACCOUNT_KIND, APPLICATION, SecretSink, SecretStore, block_on_runtime};

    fn scratch_keyring() -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("mailcal-linux-secrets-{nonce}"));
        fs::create_dir_all(&directory).expect("create secure-store scratch directory");
        directory.join("accounts.keyring")
    }

    fn file_store(path: &std::path::Path, key: &[u8]) -> SecretStore {
        namespaced_file_store(path, key, None)
    }

    fn namespaced_file_store(
        path: &std::path::Path,
        key: &[u8],
        namespace: Option<&str>,
    ) -> SecretStore {
        let runtime = Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("secure-store runtime");
        let unlocked = runtime
            .block_on(file::UnlockedKeyring::load(
                path,
                Secret::from(key.to_vec()),
            ))
            .expect("open encrypted file backend");
        let keyring = Keyring::File(Arc::new(RwLock::new(Some(file::Keyring::Unlocked(
            unlocked,
        )))));
        SecretStore {
            runtime,
            keyring,
            namespace: namespace.map(str::to_owned),
        }
    }

    #[test]
    fn account_items_are_scoped_by_application_kind_and_id() {
        let path = scratch_keyring();
        let store = file_store(&path, b"attribute-key");
        let attributes = store.account_attributes("alice@example.test@imap.example.test");

        assert_eq!(attributes["application"], APPLICATION);
        assert_eq!(attributes["kind"], ACCOUNT_KIND);
        assert_eq!(
            attributes["account"],
            "alice@example.test@imap.example.test"
        );
        assert!(!store.index_attributes().contains_key("account"));
    }

    /// The real store must keep writing and reading exactly the attributes it always has.
    ///
    /// Every item already in a developer's or a user's keyring was written without a `namespace`,
    /// so tagging the real store with one: even a default like `"default"`: would match none of
    /// them on the next launch, and every account would appear to have vanished.
    #[test]
    fn the_real_store_tags_nothing_so_it_still_finds_what_is_already_stored() {
        let path = scratch_keyring();
        let store = file_store(&path, b"real-key");

        assert!(
            !store
                .account_attributes("someone")
                .contains_key("namespace")
        );
        assert!(!store.index_attributes().contains_key("namespace"));
    }

    /// A debug namespace and the real accounts cannot see each other, in either direction.
    ///
    /// This is what lets a harness sign-in survive a relaunch: before it, those launches were
    /// handed a store that refused every write, so signing in while testing never stuck.
    ///
    /// Each step reopens the keyring rather than holding two stores at once; the file backend
    /// refuses a write when the file changed under a handle it already had, which is an artefact
    /// of testing without a Secret Service daemon rather than anything the app can hit.
    #[test]
    fn a_dev_namespace_and_the_real_accounts_never_see_each_other() {
        let path = scratch_keyring();
        let key = b"shared-keyring-key";
        let real = "[allodia]\nemail = \"real@example.test\"\nrefresh_token = \"real\"\n";
        let harness = "[allodia]\nemail = \"harness@example.test\"\nrefresh_token = \"dev\"\n";

        file_store(&path, key)
            .save("allodia-account", real)
            .expect("write the real entry");
        namespaced_file_store(&path, key, Some("dev"))
            .save("allodia-account", harness)
            .expect("write the dev entry");

        // One account id, two items, each store seeing only its own.
        assert_eq!(
            file_store(&path, key).configs().expect("real configs"),
            vec![real.to_owned()]
        );
        assert_eq!(
            namespaced_file_store(&path, key, Some("dev"))
                .configs()
                .expect("dev configs"),
            vec![harness.to_owned()]
        );

        // And erasing one leaves the other alone, which a shared index would not.
        namespaced_file_store(&path, key, Some("dev"))
            .remove("allodia-account")
            .expect("remove the dev entry");
        assert!(
            namespaced_file_store(&path, key, Some("dev"))
                .configs()
                .expect("dev configs")
                .is_empty()
        );
        assert_eq!(
            file_store(&path, key).configs().expect("real configs"),
            vec![real.to_owned()],
            "signing out of the harness must not touch the developer's own accounts"
        );
    }

    #[test]
    fn credential_rotation_can_persist_from_inside_the_core_runtime() {
        let secret_runtime = Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("secret runtime");
        let core_runtime = Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .expect("core runtime");

        let persisted = core_runtime.block_on(async {
            block_on_runtime(&secret_runtime, async { "persisted" }).expect("nested block")
        });

        assert_eq!(persisted, "persisted");
    }

    #[test]
    fn encrypted_store_replaces_then_deletes_the_account_credential_and_index() {
        let path = scratch_keyring();
        let key = [0x5a; 64];
        let store = Arc::new(file_store(&path, &key));
        let sink = SecretSink::new(Arc::clone(&store));

        sink.persist("first".to_owned(), "secret-v1".to_owned())
            .expect("save first credential");
        sink.persist("second".to_owned(), "secret-two".to_owned())
            .expect("save second credential");
        sink.persist("first".to_owned(), "secret-v2".to_owned())
            .expect("replace first credential");
        assert_eq!(
            store.configs().expect("read account index"),
            ["secret-v2", "secret-two"],
            "replacement keeps one ordered index entry and exposes only the new secret",
        );
        let encrypted = fs::read(&path).expect("read encrypted keyring file");
        assert!(
            !encrypted
                .windows("secret-v2".len())
                .any(|bytes| bytes == b"secret-v2")
        );

        sink.delete("first".to_owned())
            .expect("delete first credential");
        assert_eq!(
            store.configs().expect("read index after delete"),
            ["secret-two"]
        );
        drop(sink);
        drop(store);

        let reopened = file_store(&path, &key);
        assert_eq!(
            reopened.configs().expect("reopen persisted keyring"),
            ["secret-two"],
            "the removed credential does not return on a cold open",
        );
        let directory = path.parent().expect("keyring parent");
        let _ = fs::remove_dir_all(directory);
    }
}
