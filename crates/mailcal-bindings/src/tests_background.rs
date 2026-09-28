//! When an added account's push watches start, against the in-memory engine.
//!
//! The account here is a provider that offers `IDLE`, so its settings default to pushing the
//! Inbox. What the watches name comes from the folders the engine has stored, which is why the
//! order of the first sync and the start matters.

use std::sync::{Arc, mpsc};

use engine_api::{AccountId, EmailAddress, Engine, Provider};
use mailcal_app::{Account, App, Telemetry, TimeZoneInit};
use mailcal_viewmodel::SyncStrategyKind;

use crate::{
    account_registry::AccountRegistry,
    background::{self, BackgroundManager},
    demo::DemoProvider,
    observer::{DebouncedObserver, ObserverBridge},
    runtime::runtime,
    tests::ChannelObserver,
};

type SharedApp = Arc<App<Box<dyn Provider>>>;

fn pushing_account() -> Account<Box<dyn Provider>> {
    Account {
        id: AccountId::try_from("pushing").expect("valid account id"),
        providers: vec![Box::new(DemoProvider::with_idle()) as Box<dyn Provider>],
        calendar_providers: Vec::new(),
        contact_providers: Vec::new(),
        identity: EmailAddress::new("push@allodia.local"),
    }
}

fn empty_app() -> SharedApp {
    let (tx, _rx) = mpsc::channel();
    Arc::new(App::new(
        Engine::open_in_memory().expect("in-memory engine opens"),
        Vec::new(),
        TimeZoneInit {
            device_zone: crate::device_zone("UTC".to_owned()),
            prefs_path: None,
        },
        None,
        Arc::new(DebouncedObserver::new(ObserverBridge {
            foreign: Box::new(ChannelObserver { tx }),
        })),
        Telemetry::off(None),
    ))
}

/// A newly added account has no stored folders until its first sync, so its settings name no
/// folder to watch before it. Starting the watches there started none, and nothing started them
/// again until the next launch: new mail waited for a manual refresh.
#[test]
fn an_added_account_watches_its_inbox_once_the_first_sync_has_run() {
    let runtime = runtime();
    let app = empty_app();
    let background = BackgroundManager::new(
        Arc::clone(&app),
        AccountRegistry::new(),
        runtime.handle().clone(),
    );
    let account = pushing_account();
    let id = account.id.clone();
    runtime.block_on(app.add_new_account_deferred(account));

    runtime.block_on(background.apply_current(id.as_str()));
    assert_eq!(
        background.task_count(id.as_str()),
        0,
        "before the first sync there is no folder to watch",
    );

    runtime.block_on(background::sync_added_account(&app, &background, &id));

    let settings = runtime.block_on(app.sync_settings());
    let row = settings
        .accounts
        .iter()
        .find(|row| row.account_id == id.as_str())
        .expect("the added account has a settings row");
    assert_eq!(row.strategy, SyncStrategyKind::Push);
    let watched: Vec<&str> = row
        .folders
        .iter()
        .filter(|folder| folder.subscribed)
        .map(|folder| folder.key.as_str())
        .collect();
    assert_eq!(watched, ["inbox"]);
    assert_eq!(
        background.task_count(id.as_str()),
        1,
        "the first sync ends with one watch, on the Inbox",
    );
}
