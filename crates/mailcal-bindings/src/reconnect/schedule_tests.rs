//! The launch order, on a paused clock: a dial and a catch-up are timers, and the log says what
//! happened in which order.

use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use super::dial_then_catch_up;

/// One account: how long its dial and its catch-up take, and whether the dial connects.
#[derive(Clone, Copy)]
struct Account {
    name: &'static str,
    dial_ms: u64,
    catch_up_ms: u64,
    connects: bool,
}

const fn account(name: &'static str, dial_ms: u64, catch_up_ms: u64) -> Account {
    Account {
        name,
        dial_ms,
        catch_up_ms,
        connects: true,
    }
}

/// Runs `accounts` through the schedule, returning the events in order and the most slots busy
/// at once.
async fn run(accounts: Vec<Account>) -> (Vec<String>, usize) {
    let events = Arc::new(Mutex::new(Vec::new()));
    let busy = Arc::new(AtomicUsize::new(0));
    let most = Arc::new(AtomicUsize::new(0));
    let log = |event: String| events.lock().unwrap().push(event);
    dial_then_catch_up(
        accounts,
        |_, account: Account| {
            let (busy, most, events) = (Arc::clone(&busy), Arc::clone(&most), Arc::clone(&events));
            async move {
                let now = busy.fetch_add(1, Ordering::SeqCst) + 1;
                most.fetch_max(now, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(account.dial_ms)).await;
                events
                    .lock()
                    .unwrap()
                    .push(format!("dialled {}", account.name));
                if account.connects {
                    Some(account)
                } else {
                    busy.fetch_sub(1, Ordering::SeqCst);
                    None
                }
            }
        },
        |_, account: Account| {
            let (busy, events) = (Arc::clone(&busy), Arc::clone(&events));
            async move {
                events
                    .lock()
                    .unwrap()
                    .push(format!("catching up {}", account.name));
                tokio::time::sleep(Duration::from_millis(account.catch_up_ms)).await;
                events
                    .lock()
                    .unwrap()
                    .push(format!("caught up {}", account.name));
                busy.fetch_sub(1, Ordering::SeqCst);
            }
        },
        async { log("calendar".to_owned()) },
    )
    .await;
    let events = events.lock().unwrap().clone();
    (events, most.load(Ordering::SeqCst))
}

fn at(events: &[String], event: &str) -> usize {
    events
        .iter()
        .position(|seen| seen == event)
        .unwrap_or_else(|| panic!("{event} never happened: {events:?}"))
}

#[tokio::test(start_paused = true)]
async fn the_calendar_runs_once_every_dial_has_landed_and_not_after_the_catch_ups() {
    // `fast` connects at once and has a long catch-up (a body prefetch); `slow` takes a while to
    // dial. The calendar waits for `slow`'s dial, and not for `fast`'s prefetch.
    let (events, _) = run(vec![account("fast", 10, 500), account("slow", 100, 10)]).await;
    assert!(
        at(&events, "calendar") > at(&events, "dialled slow"),
        "{events:?}"
    );
    assert!(
        at(&events, "calendar") < at(&events, "caught up fast"),
        "{events:?}"
    );
}

#[tokio::test(start_paused = true)]
async fn an_account_catches_up_as_soon_as_its_own_dial_lands() {
    // A server that takes long to answer holds back no other account's mail.
    let (events, _) = run(vec![account("fast", 10, 10), account("slow", 1000, 10)]).await;
    assert!(
        at(&events, "caught up fast") < at(&events, "dialled slow"),
        "{events:?}"
    );
}

#[tokio::test(start_paused = true)]
async fn the_bound_covers_the_catch_ups_as_well_as_the_dials() {
    let accounts = (0..6).map(|_| account("any", 10, 100)).collect::<Vec<_>>();
    let (_, most) = run(accounts).await;
    assert_eq!(most, crate::account_registry::MAX_CONCURRENT_DIALS);
}

#[tokio::test(start_paused = true)]
async fn an_account_that_does_not_connect_has_no_catch_up_and_still_counts_as_dialled() {
    let refused = Account {
        connects: false,
        ..account("refused", 50, 10)
    };
    let (events, _) = run(vec![account("ok", 10, 500), refused]).await;
    assert!(!events.iter().any(|event| event == "catching up refused"));
    assert!(at(&events, "calendar") > at(&events, "dialled refused"));
    assert!(at(&events, "calendar") < at(&events, "caught up ok"));
}

#[tokio::test(start_paused = true)]
async fn with_nothing_to_dial_the_calendar_still_runs() {
    let (events, _) = run(Vec::new()).await;
    assert_eq!(events, ["calendar"]);
}
