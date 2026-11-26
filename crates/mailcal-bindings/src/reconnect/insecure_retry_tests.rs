//! What a retry does with a server reached only over plaintext: it dials it again, once, badging
//! nothing, whether the whole account or only its calendar and address book are.
//!
//! Driven through the real constructor; every dial is counted at the server it reaches.

use std::{
    sync::{Arc, atomic::Ordering, mpsc},
    time::Duration,
};

use mailcal_account::InsecureCause;

use super::{
    leading_server::leading_server,
    signin_refused_tests::{account, boot, wait_for},
};
use crate::{
    LogLevel, MailcalApp, UseProblem,
    tests::{ChannelObserver, NullLogger, RecordingCredentialStore, RecordingStoreHandle},
};

#[test]
fn one_retry_dials_an_account_led_to_plaintext_exactly_once() {
    let server = leading_server(false);
    let (config, id) = account("led-once", &server.base);
    let app = boot!(new_accounts, "insecure-retry-once", config);
    let id = id.as_str().to_owned();
    let settled = || {
        app.redialling.lock().unwrap().is_empty() && app.disconnected.lock().unwrap().contains(&id)
    };
    wait_for("the launch's verdict", || {
        !app.app.insecure_server_accounts().is_empty() && settled()
    });
    let before = server.sessions.load(Ordering::SeqCst);

    app.retry_connections();
    wait_for("the retry's dial", || {
        server.sessions.load(Ordering::SeqCst) > before && settled()
    });
    std::thread::sleep(Duration::from_millis(500));

    assert_eq!(server.sessions.load(Ordering::SeqCst), before + 1);
    assert!(
        !app.connectivity().unreachable_accounts.contains(&id),
        "nothing badged"
    );
}

/// A calendar and an address book whose server led to plaintext, beside a mailbox that
/// connected: each retry dials them again, the account's badge stays its mail's, and they connect
/// once the server stops leading there.
#[test]
fn uses_led_to_plaintext_beside_working_mail_are_dialled_again_and_cleared_once_fixed() {
    let server = leading_server(true);
    // The launch's dial: its mail's session read is answered, its calendar's and contacts' led.
    server.answer_first.store(1, Ordering::SeqCst);
    let (config, id) = account("led-uses", &server.base);
    let app = boot!(new_accounts, "insecure-retry-uses", config);
    let id = id.as_str().to_owned();
    let led = |problem: &Option<mailcal_app::UseProblem>| {
        *problem
            == Some(mailcal_app::UseProblem::Insecure(
                InsecureCause::LedToPlaintext,
            ))
    };
    let both_led = || {
        let problems = app
            .app
            .use_problems(&engine_api::AccountId::try_from(id.as_str()).unwrap());
        led(&problems.calendar) && led(&problems.contacts)
    };
    wait_for("the launch's dial", || {
        both_led() && app.redialling.lock().unwrap().is_empty()
    });
    let connectivity = app.connectivity();
    assert!(
        !connectivity.unreachable_accounts.contains(&id),
        "nothing badged"
    );
    assert!(
        connectivity
            .calendar_outages
            .iter()
            .any(|outage| outage.account_id == id && outage.problem == UseProblem::LedToPlaintext)
    );
    assert!(
        !app.disconnected.lock().unwrap().contains(&id),
        "the mailbox connected"
    );

    // A retry dials them again: the session read for them, and each of the two.
    let before = server.sessions.load(Ordering::SeqCst);
    server.answer_first.store(1, Ordering::SeqCst);
    app.retry_connections();
    wait_for("the retry's dial", || {
        server.sessions.load(Ordering::SeqCst) >= before + 3
            && app.redialling.lock().unwrap().is_empty()
    });
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(
        server.sessions.load(Ordering::SeqCst),
        before + 3,
        "dialled once, the mailbox left alone"
    );
    assert!(both_led(), "still led there, still said");
    assert!(!app.connectivity().unreachable_accounts.contains(&id));

    server.led.store(false, Ordering::SeqCst);
    app.retry_connections();
    wait_for("the dial once fixed", || {
        let problems = app
            .app
            .use_problems(&engine_api::AccountId::try_from(id.as_str()).unwrap());
        problems.calendar.is_none()
            && problems.contacts.is_none()
            && app.redialling.lock().unwrap().is_empty()
    });
}
