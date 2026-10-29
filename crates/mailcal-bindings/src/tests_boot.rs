//! Binding-level boot recovery tests. These lock the host-facing guarantee that every native
//! client constructor using `new_accounts` runs the shared engine startup lease recovery.

use std::{
    fs,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use engine_api::{AccountId, CalendarWrites, Engine};
use engine_core::{mail::Mailbox, sync::SyncState};
use engine_provider::{Capabilities, ConnectionInfo, Provider, ProviderResult, ScopeSync};
use tokio::sync::oneshot;

use crate::{LogLevel, Logger, MailcalApp, Observer, Surface};

struct NoopObserver;

impl Observer for NoopObserver {
    fn surface_changed(&self, _surface: Surface) {}
}

struct NullLogger;

impl Logger for NullLogger {
    fn log(&self, _level: LogLevel, _target: String, _message: String) {}
}

/// This boot has no accounts, so nothing can rotate; the store is here because the constructor
/// requires one, which is the property `credential_store.rs` exists to hold.
struct NullCredentialStore;

impl crate::AccountCredentialStore for NullCredentialStore {
    fn persist(
        &self,
        _account_id: String,
        _config_toml: String,
    ) -> Result<(), crate::CredentialStoreError> {
        Ok(())
    }

    fn delete(&self, _account_id: String) -> Result<(), crate::CredentialStoreError> {
        Ok(())
    }
}

struct BlockingMailboxProvider {
    caps: Capabilities,
    claimed: Mutex<Option<oneshot::Sender<()>>>,
}

impl BlockingMailboxProvider {
    fn new(claimed: oneshot::Sender<()>) -> Self {
        Self {
            caps: Capabilities::none().with_mail(),
            claimed: Mutex::new(Some(claimed)),
        }
    }
}

#[async_trait::async_trait]
impl Provider for BlockingMailboxProvider {
    fn connection_info(&self) -> ConnectionInfo {
        ConnectionInfo::new(self.caps)
    }

    async fn sync_mailboxes(
        &self,
        _account: &AccountId,
        _cursor: Option<&SyncState>,
    ) -> ProviderResult<ScopeSync<Mailbox>> {
        if let Some(claimed) = self.claimed.lock().expect("claimed mutex poisoned").take() {
            let _ = claimed.send(());
        }
        std::future::pending::<ProviderResult<ScopeSync<Mailbox>>>().await
    }
}

impl engine_api::MailboxWrites for BlockingMailboxProvider {}
impl CalendarWrites for BlockingMailboxProvider {}

fn temp_data_dir(name: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is after the Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "mailcal-bindings-{name}-{}-{nanos}",
        std::process::id(),
    ))
}

#[test]
fn new_accounts_abandons_interrupted_sync_leases_on_boot() {
    let data_dir = temp_data_dir("boot-recovery");
    fs::create_dir_all(&data_dir).expect("create temp data dir");
    let db = data_dir.join("mailcal.sqlite");
    let account = AccountId::try_from("boot-recovery@example.com@imap.example.com")
        .expect("valid account id");

    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let engine = std::sync::Arc::new(Engine::open(&db).expect("open engine"));
    let (claimed_tx, claimed_rx) = oneshot::channel();
    let provider = std::sync::Arc::new(BlockingMailboxProvider::new(claimed_tx));
    let sync = {
        let engine = std::sync::Arc::clone(&engine);
        let provider = std::sync::Arc::clone(&provider);
        let account = account.clone();
        runtime.spawn(async move {
            engine
                .sync_mail(
                    core::slice::from_ref(&*provider),
                    &account,
                    engine_api::StreamTuning::default(),
                    &engine_api::IgnoreCommits,
                )
                .await
        })
    };
    runtime
        .block_on(claimed_rx)
        .expect("sync claimed the mailbox scope");
    sync.abort();
    let _ = runtime.block_on(sync);
    drop(engine);
    drop(runtime);

    let app = MailcalApp::new_accounts(
        Box::new(NoopObserver),
        Box::new(NullLogger),
        LogLevel::Info,
        Vec::new(),
        data_dir.to_string_lossy().into_owned(),
        "Etc/UTC".to_owned(),
        crate::analytics::test_device(),
        Box::new(NullCredentialStore),
    )
    .expect("boot over interrupted store");
    drop(app);

    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let engine = Engine::open(&db).expect("reopen engine");
    assert_eq!(
        runtime
            .block_on(engine.abandon_sync_leases())
            .expect("query abandoned leases after boot"),
        0,
        "new_accounts should have abandoned the interrupted lease during boot",
    );

    let _ = fs::remove_dir_all(data_dir);
}

/// Keeps every config the core stores, as a host's keychain would.
struct RecordingCredentialStore(std::sync::Arc<Mutex<Vec<String>>>);

impl crate::AccountCredentialStore for RecordingCredentialStore {
    fn persist(
        &self,
        _account_id: String,
        config_toml: String,
    ) -> Result<(), crate::CredentialStoreError> {
        self.0
            .lock()
            .expect("stored mutex poisoned")
            .push(config_toml);
        Ok(())
    }

    fn delete(&self, _account_id: String) -> Result<(), crate::CredentialStoreError> {
        Ok(())
    }
}

fn boot(
    data_dir: &std::path::Path,
    configs: Vec<String>,
    stored: &std::sync::Arc<Mutex<Vec<String>>>,
) -> std::sync::Arc<MailcalApp> {
    MailcalApp::new_accounts(
        Box::new(NoopObserver),
        Box::new(NullLogger),
        LogLevel::Info,
        configs,
        data_dir.to_string_lossy().into_owned(),
        "Etc/UTC".to_owned(),
        crate::analytics::test_device(),
        Box::new(RecordingCredentialStore(std::sync::Arc::clone(stored))),
    )
    .expect("boot")
}

/// Nothing listens on the discard port, so every dial fails and nothing leaves the machine.
const ALICE: &str =
    "[jmap]\nemail = \"alice@example.org\"\nbase_url = \"http://127.0.0.1:9\"\npassword = \"pw\"\n";

#[test]
fn a_stored_shared_mailbox_is_listed_once_under_its_own_id_after_a_relaunch() {
    let data_dir = temp_data_dir("shared-relaunch");
    fs::create_dir_all(&data_dir).expect("create temp data dir");
    let stored = std::sync::Arc::default();
    let app = boot(&data_dir, vec![ALICE.to_owned()], &stored);
    let parent = app.accounts_snapshot().accounts[0].id.clone();
    app.set_shared_mailbox(
        parent.clone(),
        crate::SharedMailboxEntry {
            handle: "support".to_owned(),
            name: "Support".to_owned(),
            address: Some("support@example.org".to_owned()),
            selected: false,
            listed: true,
        },
        true,
    )
    .expect("chosen");
    let persisted = stored
        .lock()
        .expect("stored mutex poisoned")
        .last()
        .cloned()
        .expect("the choice is stored with the account");
    drop(app);

    let expected = [
        (parent.clone(), "alice@example.org".to_owned()),
        (
            format!("{parent}#shared:support"),
            "support@example.org".to_owned(),
        ),
    ];
    // As stored, and as a dev launch hands it in: its canned account beside the copy stored for it.
    for configs in [vec![persisted.clone()], vec![ALICE.to_owned(), persisted]] {
        let app = boot(&data_dir, configs, &stored);
        let rows: Vec<(String, String)> = app
            .sync_settings()
            .accounts
            .into_iter()
            .map(|row| (row.account_id, row.email))
            .collect();
        assert_eq!(rows, expected);
        assert_eq!(
            app.accounts_snapshot().accounts.len(),
            1,
            "alice, listed once"
        );
    }
    let _ = fs::remove_dir_all(data_dir);
}

#[path = "tests_boot_outbox.rs"]
mod outbox;
