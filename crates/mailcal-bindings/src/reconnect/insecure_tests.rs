//! An account whose server is reached only over plaintext: no sign-in is sent to it, it is
//! neither badged unreachable nor asked to sign in again, and the next retry dials it again, once,
//! in case its server changed.
//!
//! Driven through the real constructors. A JMAP account at a plaintext address off this device is
//! refused by the engine before anything is sent, so nothing leaves the machine; a loopback server
//! that sends the session request on to such an address is one that led there, and one that later
//! answers is a server that was fixed.

use std::sync::{Arc, atomic::Ordering, mpsc};

use super::{
    leading_server::{PLAINTEXT, leading_server},
    signin_refused_tests::{account, boot, wait_for},
};
use crate::{
    Intent, LogLevel, MailcalApp, NoticeKind,
    tests::{ChannelObserver, NullLogger, RecordingCredentialStore, RecordingStoreHandle},
};

#[test]
fn a_plaintext_address_is_said_badges_nothing_and_is_queued_for_the_next_retry() {
    let (config, id) = account("plaintext", PLAINTEXT);
    let app = boot!(new_accounts, "insecure-server", config);
    let id = id.as_str().to_owned();

    wait_for("the launch's dial", || {
        app.app
            .insecure_server_accounts()
            .iter()
            .any(|(account, cause)| {
                *account == id && *cause == mailcal_account::InsecureCause::PlaintextAddress
            })
            && app.disconnected.lock().unwrap().contains(&id)
    });

    let connectivity = app.connectivity();
    assert!(
        !connectivity.unreachable_accounts.contains(&id),
        "no outage"
    );
    assert!(!connectivity.signin_expired_accounts.contains(&id));
    assert!(
        app.notices()
            .notices
            .iter()
            .any(|notice| matches!(notice.kind, NoticeKind::InsecureServer { .. })),
        "the account says why"
    );
}

/// The headless worker dials synchronously, records the same answer and queues it the same way.
#[test]
fn a_headless_boot_records_a_plaintext_server_and_queues_it() {
    let (config, id) = account("plaintext-headless", PLAINTEXT);

    let app = boot!(new_background_worker, "insecure-server-headless", config);

    let id = id.as_str().to_owned();
    assert!(
        app.app
            .insecure_server_accounts()
            .iter()
            .any(|(account, _)| *account == id)
    );
    assert!(app.disconnected.lock().unwrap().contains(&id));
}

/// A server that led to plaintext and then serves its address securely is connected by the next
/// refresh, and what was said about it goes.
#[test]
fn a_server_that_no_longer_leads_to_plaintext_connects_on_the_next_refresh() {
    let server = leading_server(false);
    let (config, id) = account("fixed", &server.base);
    let app = boot!(new_accounts, "insecure-server-fixed", config);
    let id = id.as_str().to_owned();
    wait_for("the launch's dial", || {
        app.app
            .insecure_server_accounts()
            .iter()
            .any(|(account, cause)| {
                *account == id && *cause == mailcal_account::InsecureCause::LedToPlaintext
            })
            && app.redialling.lock().unwrap().is_empty()
    });

    server.led.store(false, Ordering::SeqCst);
    app.dispatch(Intent::RefreshMail);

    wait_for("the refresh's dial", || {
        app.app.insecure_server_accounts().is_empty()
            && app.redialling.lock().unwrap().is_empty()
            && !app.disconnected.lock().unwrap().contains(&id)
    });
}

/// A repair that connects, a new password or servers saved on the account's page, takes away
/// what was said about a server reached only over plaintext.
#[test]
fn a_repair_that_connects_takes_the_verdict_away() {
    let server = leading_server(false);
    let (config, id) = account("repaired", &server.base);
    let app = boot!(new_accounts, "insecure-server-repaired", config);
    let id = id.as_str().to_owned();
    wait_for("the launch's dial", || {
        app.app
            .insecure_server_accounts()
            .iter()
            .any(|(account, _)| *account == id)
    });

    server.led.store(false, Ordering::SeqCst);
    app.replace_account_secret(id.clone(), "new-pw".to_owned())
        .expect("the server answers now");

    assert!(app.app.insecure_server_accounts().is_empty());
}
