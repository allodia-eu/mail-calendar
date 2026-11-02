//! An account that could not be reached is dialled again by a refresh of any surface that says
//! so, the calendar's and contacts' included (`docs/accounts.md` rule 16).
//!
//! Driven through the real constructor over a loopback server that accepts each connection and
//! closes it unanswered, counting them, so the assertion is on what the server sees.

use std::{
    net::TcpListener,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    time::Duration,
};

use super::signin_refused_tests::{account, boot, wait_for};
use crate::{
    Intent, LogLevel, MailcalApp,
    tests::{ChannelObserver, NullLogger, RecordingCredentialStore, RecordingStoreHandle},
};

/// A server that hangs up on every connection, and how many it has taken.
fn silent_server() -> (String, Arc<AtomicUsize>) {
    let connections = Arc::new(AtomicUsize::new(0));
    let seen = Arc::clone(&connections);
    let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
    let addr = listener.local_addr().expect("the bound address");
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            if stream.is_ok() {
                seen.fetch_add(1, Ordering::SeqCst);
            }
        }
    });
    (format!("http://{addr}"), connections)
}

#[test]
fn a_refresh_of_the_calendar_or_contacts_dials_an_unreachable_account_again() {
    let (base_url, connections) = silent_server();
    let (config, id) = account("silent", &base_url);
    let app = boot!(new_accounts, "refresh-redial", config);
    let id = id.as_str().to_owned();
    // A dial has ended when nothing is being dialled and the account waits for the next retry.
    let settled = || {
        app.redialling.lock().unwrap().is_empty() && app.disconnected.lock().unwrap().contains(&id)
    };
    wait_for("the launch's outage", || {
        app.connectivity().unreachable_accounts.contains(&id) && settled()
    });

    for (what, intent) in [
        ("the calendar's refresh to dial", Intent::RefreshCalendar),
        ("the contacts' refresh to dial", Intent::RefreshContacts),
    ] {
        let before = connections.load(Ordering::SeqCst);
        app.dispatch(intent);
        wait_for(what, || {
            connections.load(Ordering::SeqCst) > before && settled()
        });
    }
}

/// A host reports the network whenever the OS says anything about it, and macOS's path monitor
/// says "satisfied" every few seconds with nothing changed. Only a report that ends an offline
/// spell dials again: re-dialling on each of the others is a storm of sign-ins.
#[test]
fn only_a_report_that_ends_an_offline_spell_dials_an_unreachable_account_again() {
    let (base_url, connections) = silent_server();
    let (config, id) = account("silent", &base_url);
    let app = boot!(new_accounts, "network-report-redial", config);
    let id = id.as_str().to_owned();
    let settled = || {
        app.redialling.lock().unwrap().is_empty() && app.disconnected.lock().unwrap().contains(&id)
    };
    wait_for("the launch's outage", || {
        app.connectivity().unreachable_accounts.contains(&id) && settled()
    });

    let before = connections.load(Ordering::SeqCst);
    for _ in 0..3 {
        app.dispatch(Intent::ReportNetworkReachable { reachable: true });
    }
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(
        connections.load(Ordering::SeqCst),
        before,
        "a report that the network is still there dialled the account again",
    );

    app.dispatch(Intent::ReportNetworkReachable { reachable: false });
    app.dispatch(Intent::ReportNetworkReachable { reachable: true });
    wait_for("coming back online to dial", || {
        connections.load(Ordering::SeqCst) > before && settled()
    });
}

/// A calendar that could not be reached beside a working mailbox is dialled again on its own:
/// the mailbox is connected, and signing in to it on every retry is what makes a provider limit
/// the account's sign-ins.
#[test]
fn a_retry_of_an_unreachable_calendar_leaves_the_mailbox_alone() {
    let (imap, mailbox_connections) = silent_server();
    let (dav, calendar_connections) = silent_server();
    let config = format!(
        "[imap]\naddr = \"{}\"\nserver_name = \"localhost\"\nusername = \"alice@example.org\"\n\
         password = \"pw\"\n\n[caldav]\nbase_url = \"{dav}/dav\"\nusername = \"alice@example.org\"\n\
         password = \"pw\"\n",
        imap.trim_start_matches("http://"),
    );
    let id = mailcal_account::load_str(&config)
        .expect("a valid config")
        .account_id()
        .expect("a valid account id")
        .as_str()
        .to_owned();
    let app = boot!(new_accounts, "refresh-redial-uses", config);
    let idle = || app.redialling.lock().unwrap().is_empty();
    wait_for("the launch's dial", || {
        app.disconnected.lock().unwrap().contains(&id) && idle()
    });
    // What a launch leaves when the mailbox connected and the calendar did not.
    app.disconnected.lock().unwrap().clear();
    app.app.note_use_problems(
        &engine_api::AccountId::try_from(id.as_str()).unwrap(),
        mailcal_app::UseProblems {
            calendar: Some(mailcal_app::UseProblem::Unreachable),
            contacts: None,
        },
    );
    let mailbox_before = mailbox_connections.load(Ordering::SeqCst);
    let calendar_before = calendar_connections.load(Ordering::SeqCst);

    app.dispatch(Intent::RefreshMail);
    wait_for("the calendar to be dialled again", || {
        calendar_connections.load(Ordering::SeqCst) > calendar_before && idle()
    });

    assert_eq!(
        mailbox_connections.load(Ordering::SeqCst),
        mailbox_before,
        "the retry signed in to a mailbox that was already connected"
    );
}
