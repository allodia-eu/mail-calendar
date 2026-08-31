//! Binding-level boot recovery tests. These lock the host-facing guarantee that every native
//! client constructor using `new_accounts` runs the shared engine startup lease recovery.

use std::{
    fs,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use engine_api::{AccountId, Engine};
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
