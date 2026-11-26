//! A shared mailbox is dialled through the account whose sign-in opens it, after that account's
//! own dial. One retry dials a parent that keeps failing once, and its shared store not at all:
//! the store waits in the queue for the next retry, which dials the parent first again.
//!
//! Driven through the real constructor; every dial is counted at the server it reaches.

use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    time::Duration,
};

use engine_api::SharedAccountId;
use mailcal_account::{SharedStoreChoice, SharedUse, SharedUses};

use super::{
    leading_server::leading_server,
    refresh_redial_tests::silent_server,
    signin_refused_tests::{account, boot, refusing_server, wait_for},
};
use crate::{
    LogLevel, MailcalApp,
    tests::{ChannelObserver, NullLogger, RecordingCredentialStore, RecordingStoreHandle},
};

/// The shared mailbox the parent's sign-in opens, and its id.
fn share(app: &MailcalApp, parent: &str) -> String {
    let choice = SharedStoreChoice {
        handle: SharedAccountId::try_from("f").expect("a handle"),
        name: "Support".to_owned(),
        address: Some("support@example.org".to_owned()),
        holds: SharedUses::of(SharedUse::Mail),
    };
    assert!(app.registry.set_shared_store(parent, choice, true));
    let [child] = app
        .registry
        .shared_children(parent)
        .try_into()
        .expect("one");
    child.id.as_str().to_owned()
}

/// Nothing is being dialled, and both `ids` wait for the next retry.
fn settled_with(app: &MailcalApp, ids: &[&str]) -> bool {
    app.redialling.lock().unwrap().is_empty() && {
        let queued = app.disconnected.lock().unwrap();
        ids.iter().all(|id| queued.contains(*id))
    }
}

/// One retry of `parent` and its shared store, with every dial counted in `dials`: the parent is
/// dialled once and the store not at all, and both wait for the next retry.
fn one_retry_dials_the_failing_parent_once(app: &MailcalApp, parent: &str, dials: &AtomicUsize) {
    let child = share(app, parent);
    let before = dials.load(Ordering::SeqCst);

    app.retry_connections();
    wait_for("the parent's dial", || {
        dials.load(Ordering::SeqCst) > before
    });
    // Long enough for the dial to end, and for a follow-up that dials again to have dialled.
    std::thread::sleep(Duration::from_secs(1));

    assert_eq!(
        dials.load(Ordering::SeqCst),
        before + 1,
        "the parent once and its shared store not at all"
    );
    assert!(
        settled_with(app, &[parent, &child]),
        "both wait for the next retry"
    );
}

#[test]
fn a_retry_dials_an_unreachable_parent_once_and_its_shared_store_not_at_all() {
    let (base_url, connections) = silent_server();
    let (config, id) = account("shared-unreachable", &base_url);
    let app = boot!(new_accounts, "shared-retry-unreachable", config);
    let id = id.as_str().to_owned();
    wait_for("the launch's outage", || {
        app.connectivity().unreachable_accounts.contains(&id) && settled_with(&app, &[&id])
    });

    one_retry_dials_the_failing_parent_once(&app, &id, &connections);
}

#[test]
fn a_retry_dials_a_parent_led_to_plaintext_once_and_its_shared_store_not_at_all() {
    let server = leading_server(false);
    let (config, id) = account("shared-insecure", &server.base);
    let app = boot!(new_accounts, "shared-retry-insecure", config);
    let id = id.as_str().to_owned();
    wait_for("the launch's verdict", || {
        !app.app.insecure_server_accounts().is_empty() && settled_with(&app, &[&id])
    });

    one_retry_dials_the_failing_parent_once(&app, &id, &server.sessions);
}

/// A parent that connects opens its shared store: the store is dialled once the parent's dial
/// is over.
#[test]
fn a_parent_that_connects_has_its_shared_store_dialled_after_it() {
    let server = leading_server(false);
    let (config, id) = account("shared-fixed", &server.base);
    let app = boot!(new_accounts, "shared-retry-fixed", config);
    let id = id.as_str().to_owned();
    wait_for("the launch's verdict", || {
        !app.app.insecure_server_accounts().is_empty() && settled_with(&app, &[&id])
    });
    share(&app, &id);
    let before = server.sessions.load(Ordering::SeqCst);

    server.led.store(false, Ordering::SeqCst);
    app.retry_connections();

    wait_for("the parent's dial, then its shared store's", || {
        server.sessions.load(Ordering::SeqCst) >= before + 2
    });
    assert!(
        !app.disconnected.lock().unwrap().contains(&id),
        "the parent connected"
    );
}

/// A parent whose sign-in the server refuses waits for a new credential, and so does its shared
/// store, which signs in with the same one: one retry presents it once, for the parent alone.
#[test]
fn a_retry_presents_a_refused_parents_credential_once_and_not_for_its_shared_store() {
    let (base_url, requests) = refusing_server();
    let (config, id) = account("shared-refused", &base_url);
    let app = boot!(new_accounts, "shared-retry-refused", config);
    let id = id.as_str().to_owned();
    wait_for("the launch's refusal", || {
        app.connectivity().signin_expired_accounts.contains(&id)
            && app.redialling.lock().unwrap().is_empty()
    });
    let child = share(&app, &id);
    // Queued, as an account is that a refusal found unreachable before.
    app.disconnected.lock().unwrap().insert(id.clone());
    let before = requests.load(Ordering::SeqCst);

    app.retry_connections();
    wait_for("the parent's dial", || {
        requests.load(Ordering::SeqCst) > before
    });
    std::thread::sleep(Duration::from_secs(1));

    assert_eq!(
        requests.load(Ordering::SeqCst),
        before + 1,
        "the credential once, for the parent"
    );
    assert!(
        !app.disconnected.lock().unwrap().contains(&child),
        "the shared store waits for a new credential with its parent, not for the next retry"
    );

    app.retry_connections();
    std::thread::sleep(Duration::from_secs(1));
    assert_eq!(
        requests.load(Ordering::SeqCst),
        before + 1,
        "the next retry presents the refused credential for nothing"
    );
}

/// A new credential for a parent whose sign-in was refused dials its shared stores with it.
#[test]
fn a_parents_new_credential_dials_its_shared_store() {
    let server = leading_server(false);
    server.led.store(false, Ordering::SeqCst);
    server.refuse.store(true, Ordering::SeqCst);
    let (config, id) = account("shared-replaced", &server.base);
    let app = boot!(new_accounts, "shared-retry-replaced", config);
    let id = id.as_str().to_owned();
    wait_for("the launch's refusal", || {
        app.connectivity().signin_expired_accounts.contains(&id)
            && app.redialling.lock().unwrap().is_empty()
    });
    share(&app, &id);
    server.refuse.store(false, Ordering::SeqCst);
    let before = server.sessions.load(Ordering::SeqCst);

    app.replace_account_secret(id.clone(), "new-pw".to_owned())
        .expect("the server takes the new password");

    wait_for("the parent's connect, then its shared store's", || {
        server.sessions.load(Ordering::SeqCst) >= before + 2
    });
}

/// Boots one app over `configs`.
fn boot_all(name: &str, configs: Vec<String>) -> Arc<MailcalApp> {
    let (tx, _rx) = mpsc::channel();
    MailcalApp::new_accounts(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        configs,
        crate::tests::temp_data_dir(name)
            .to_string_lossy()
            .into_owned(),
        "Etc/UTC".to_owned(),
        crate::analytics::test_device(),
        Box::new(RecordingStoreHandle(Arc::new(
            RecordingCredentialStore::default(),
        ))),
    )
    .expect("the app boots")
}

/// A parent that connects has its shared store dialled after it, and only that: an account
/// beside it that failed again waits for the next retry, so one retry dials it once.
#[test]
fn a_shared_store_dialled_after_its_parent_dials_nothing_else_again() {
    let server = leading_server(false);
    let (parent_config, parent) = account("shared-sibling-parent", &server.base);
    let (silent_url, connections) = silent_server();
    let (sibling_config, sibling) = account("shared-sibling", &silent_url);
    let app = boot_all("shared-retry-sibling", vec![parent_config, sibling_config]);
    let (parent, sibling) = (parent.as_str().to_owned(), sibling.as_str().to_owned());
    wait_for("the launch's dials", || {
        app.connectivity().unreachable_accounts.contains(&sibling)
            && !app.app.insecure_server_accounts().is_empty()
            && settled_with(&app, &[&parent, &sibling])
    });
    let child = share(&app, &parent);
    server.led.store(false, Ordering::SeqCst);
    let (sessions, dials) = (
        server.sessions.load(Ordering::SeqCst),
        connections.load(Ordering::SeqCst),
    );

    app.retry_connections();
    wait_for("the parent's dial, then its shared store's", || {
        server.sessions.load(Ordering::SeqCst) >= sessions + 2
    });
    std::thread::sleep(Duration::from_secs(1));

    assert_eq!(
        connections.load(Ordering::SeqCst),
        dials + 1,
        "the account beside it once"
    );
    assert!(
        settled_with(&app, &[&sibling]),
        "it waits for the next retry"
    );
    assert!(!app.disconnected.lock().unwrap().contains(&parent));
    let _ = child;
}

/// A Refresh that finds the parent being dialled by another pass, its sign-in about to be
/// refused, as the other pass is modelled here: `store_queued` says whether the shared store was
/// queued on its own, as opening it queues it, or only derived from its parent.
///
/// The Refresh dials nothing and marks the parent as asked for again. The other pass then ends
/// as a claim does: it queues the parent and its follow-up dials the parent alone, which is
/// refused and derives the store again, dropping it with the parent. The next Refresh must
/// present the refused credential for nothing.
fn a_retry_racing_the_parents_refused_dial(name: &'static str, store_queued: bool) {
    let (base_url, requests) = refusing_server();
    let (config, id) = account(name, &base_url);
    let app = boot!(new_accounts, "shared-retry-racing", config);
    let id = id.as_str().to_owned();
    wait_for("the launch's refusal", || {
        app.connectivity().signin_expired_accounts.contains(&id)
            && app.redialling.lock().unwrap().is_empty()
    });
    let child = share(&app, &id);
    if store_queued {
        app.disconnected.lock().unwrap().insert(child.clone());
    }
    // Another pass is dialling the parent, and it is queued again meanwhile.
    app.redialling.lock().unwrap().insert(id.clone(), false);
    app.disconnected.lock().unwrap().insert(id.clone());

    app.retry_connections();
    assert_eq!(
        app.redialling.lock().unwrap().get(&id),
        Some(&true),
        "the parent is asked for again"
    );
    assert!(
        !app.disconnected.lock().unwrap().contains(&child),
        "the shared store is the parent's next dial's to derive"
    );

    // The other pass ends as its claim does: the parent, asked for again, is queued and dialled
    // alone by the follow-up, which is refused.
    let before = requests.load(Ordering::SeqCst);
    app.redialling.lock().unwrap().remove(&id);
    app.disconnected.lock().unwrap().insert(id.clone());
    app.retry_only(std::slice::from_ref(&id));
    wait_for("the follow-up's refusal", || {
        requests.load(Ordering::SeqCst) > before && app.redialling.lock().unwrap().is_empty()
    });
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(
        requests.load(Ordering::SeqCst),
        before + 1,
        "the parent once"
    );

    app.retry_connections();
    std::thread::sleep(Duration::from_secs(1));
    assert_eq!(
        requests.load(Ordering::SeqCst),
        before + 1,
        "the refused credential is not presented for the shared store"
    );
}

#[test]
fn a_retry_that_finds_the_parent_being_dialled_does_not_queue_its_derived_shared_store() {
    a_retry_racing_the_parents_refused_dial("shared-racing-derived", false);
}

#[test]
fn a_retry_that_finds_the_parent_being_dialled_does_not_put_back_its_queued_shared_store() {
    a_retry_racing_the_parents_refused_dial("shared-racing-queued", true);
}
