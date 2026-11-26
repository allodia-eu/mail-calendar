use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use engine_core::error::FailureClass;
use engine_provider::ProviderError;

use super::{Delegate, SharedClient, should_reconnect};

/// A client that is the number of the connect that made it.
fn counting() -> (Arc<SharedClient<usize>>, Arc<AtomicUsize>) {
    let connects = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&connects);
    let client = SharedClient::new(move || Ok(counter.fetch_add(1, Ordering::SeqCst) + 1));
    (Arc::new(client), connects)
}

#[test]
fn one_client_serves_every_delegate_until_it_is_dropped() {
    let (client, connects) = counting();
    let inbox = Delegate::new(Arc::clone(&client), |client| ("inbox", client)).unwrap();
    let sent = Delegate::new(Arc::clone(&client), |client| ("sent", client)).unwrap();
    assert_eq!(*inbox.get().unwrap().1, ("inbox", 1));
    assert_eq!(*sent.get().unwrap().1, ("sent", 1));
    assert_eq!(connects.load(Ordering::SeqCst), 1);
}

/// The pool behind a dead socket may hold more like it, so one folder meeting it replaces the
/// client of every folder.
#[test]
fn a_dropped_client_is_replaced_for_every_delegate_on_it() {
    let (client, connects) = counting();
    let inbox = Delegate::new(Arc::clone(&client), |client| client).unwrap();
    let sent = Delegate::new(Arc::clone(&client), |client| client).unwrap();

    let (generation, _) = inbox.get().unwrap();
    inbox.drop_client(generation);

    assert_eq!(*inbox.get().unwrap().1, 2);
    assert_eq!(*sent.get().unwrap().1, 2);
    assert_eq!(connects.load(Ordering::SeqCst), 2);
}

/// Two folders meeting the same dead client both drop it, and the second must not drop the one
/// the first has just connected.
#[test]
fn a_client_already_replaced_is_not_dropped_again() {
    let (client, connects) = counting();
    let inbox = Delegate::new(Arc::clone(&client), |client| client).unwrap();
    let sent = Delegate::new(Arc::clone(&client), |client| client).unwrap();
    let (on_inbox, _) = inbox.get().unwrap();
    let (on_sent, _) = sent.get().unwrap();

    inbox.drop_client(on_inbox);
    assert_eq!(*inbox.get().unwrap().1, 2);
    sent.drop_client(on_sent);

    assert_eq!(*sent.get().unwrap().1, 2);
    assert_eq!(connects.load(Ordering::SeqCst), 2);
}

/// Two requests of one wrapper fail on the same dead client. The first replaces it; the second,
/// failing after that, names the client it used, so the new one is kept.
#[test]
fn two_failures_in_flight_on_one_delegate_replace_the_client_once() {
    let (client, connects) = counting();
    let inbox = Delegate::new(client, |client| client).unwrap();
    let (first, _) = inbox.get().unwrap();
    let (second, _) = inbox.get().unwrap();

    inbox.drop_client(first);
    assert_eq!(*inbox.get().unwrap().1, 2);
    inbox.drop_client(second);

    assert_eq!(*inbox.get().unwrap().1, 2);
    assert_eq!(connects.load(Ordering::SeqCst), 2);
}

#[test]
fn the_last_delegate_answers_until_a_request_builds_the_next() {
    let (client, _) = counting();
    let inbox = Delegate::new(client, |client| client).unwrap();
    let (generation, _) = inbox.get().unwrap();
    inbox.drop_client(generation);
    assert_eq!(*inbox.last(), 1);
    assert_eq!(*inbox.get().unwrap().1, 2);
    assert_eq!(*inbox.last(), 2);
}

#[test]
fn a_retryable_transport_error_reconnects_but_a_rate_limit_or_auth_does_not() {
    // A retryable transport failure (a stale keep-alive socket after sleep) replaces the client
    // and its connection pool…
    assert!(should_reconnect(&ProviderError::retryable("broken pipe")));
    // …while a 429 is waited out on the *same* connection one layer down, and a refused
    // credential has already been replaced once by the engine.
    assert!(!should_reconnect(&ProviderError::new(
        FailureClass::RateLimited,
        "429",
    )));
    assert!(!should_reconnect(&ProviderError::authentication("401")));
}
