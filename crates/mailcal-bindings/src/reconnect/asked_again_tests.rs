//! A dial asked for while one of the same account is under way, as switching a use asks for one,
//! follows that dial at once rather than waiting for the next refresh: the dial under way was
//! planned from what the account was before.
//!
//! Driven through the real constructor over a loopback server that holds each connection open on
//! request, counting them, so the assertion is on what the server sees.

use std::{
    net::{TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc,
    },
    time::Duration,
};

use super::signin_refused_tests::{account, boot, wait_for};
use crate::{
    Intent, LogLevel, MailcalApp,
    tests::{ChannelObserver, NullLogger, RecordingCredentialStore, RecordingStoreHandle},
};

/// A server that hangs up on every connection, or holds each open while told to.
struct HoldingServer {
    url: String,
    connections: Arc<AtomicUsize>,
    holding: Arc<AtomicBool>,
    held: Arc<Mutex<Vec<TcpStream>>>,
}

impl HoldingServer {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
        let addr = listener.local_addr().expect("the bound address");
        let server = Self {
            url: format!("http://{addr}"),
            connections: Arc::default(),
            holding: Arc::default(),
            held: Arc::default(),
        };
        let (connections, holding, held) = (
            Arc::clone(&server.connections),
            Arc::clone(&server.holding),
            Arc::clone(&server.held),
        );
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                connections.fetch_add(1, Ordering::SeqCst);
                if holding.load(Ordering::SeqCst) {
                    held.lock().unwrap().push(stream);
                }
            }
        });
        server
    }

    fn connections(&self) -> usize {
        self.connections.load(Ordering::SeqCst)
    }

    fn hold(&self) {
        self.holding.store(true, Ordering::SeqCst);
    }

    fn release(&self) {
        self.holding.store(false, Ordering::SeqCst);
        self.held.lock().unwrap().clear();
    }
}

#[test]
fn a_dial_asked_for_while_one_is_under_way_follows_it_at_once() {
    let server = HoldingServer::start();
    let (config, id) = account("held", &server.url);
    let app = boot!(new_accounts, "asked-again-redial", config);
    let id = id.as_str().to_owned();
    let settled = || {
        app.redialling.lock().unwrap().is_empty() && app.disconnected.lock().unwrap().contains(&id)
    };
    wait_for("the launch's outage", || {
        app.connectivity().unreachable_accounts.contains(&id) && settled()
    });
    // How many connections one dial makes, so the second dial below is told from the first.
    let before = server.connections();
    app.dispatch(Intent::RefreshMail);
    wait_for("a dial", || server.connections() > before && settled());
    std::thread::sleep(Duration::from_millis(300));
    let one_dial = server.connections() - before;

    server.hold();
    let before = server.connections();
    app.dispatch(Intent::RefreshMail);
    wait_for("a dial to be under way", || {
        server.connections() > before && app.redialling.lock().unwrap().contains_key(&id)
    });
    // What switching a use on or off does: queue the account and ask for a retry.
    app.disconnected.lock().unwrap().insert(id.clone());
    app.retry_connections();
    assert!(
        crate::retry::asked_again(&app.redialling, &id),
        "the dial under way was not told it is out of date",
    );

    server.release();
    wait_for("the dial asked for meanwhile", || {
        server.connections() >= before + 2 * one_dial && settled()
    });
}
