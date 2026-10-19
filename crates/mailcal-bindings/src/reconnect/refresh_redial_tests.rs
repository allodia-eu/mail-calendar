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
        wait_for(what, || connections.load(Ordering::SeqCst) > before && settled());
    }
}
