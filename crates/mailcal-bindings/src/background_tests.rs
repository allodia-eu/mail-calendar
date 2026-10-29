//! The watch backoff, its log scope and logging rule, and the refresher that keeps a watch
//! from waiting on its own refreshes.

use super::*;

#[test]
fn a_watch_log_scope_does_not_expose_the_account_address() {
    let scope = watch_log_scope("alice@example.test@imap.example.test", "folder-opaque-key");

    assert!(scope.starts_with("acct:"));
    assert!(scope.ends_with("/folder-opaque-key"));
    assert!(!scope.contains("alice"));
    assert!(!scope.contains('@'));
}

#[tokio::test]
async fn changes_during_a_refresh_cost_one_more_refresh_and_never_wait_for_it() {
    let runs = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let release = Arc::new(tokio::sync::Notify::new());
    let refresher = Refresher::spawn({
        let (runs, release) = (Arc::clone(&runs), Arc::clone(&release));
        move || {
            let (runs, release) = (Arc::clone(&runs), Arc::clone(&release));
            async move {
                runs.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                release.notified().await;
            }
        }
    });
    let count = || runs.load(std::sync::atomic::Ordering::SeqCst);
    let settle = || tokio::time::sleep(Duration::from_millis(20));

    refresher.request();
    settle().await;
    assert_eq!(count(), 1, "the first request starts a refresh");
    // Three changes while it runs: each request returns at once, and they become one.
    for _ in 0..3 {
        refresher.request();
    }
    release.notify_one();
    settle().await;
    assert_eq!(count(), 2, "the changes during it cost exactly one more");
    release.notify_one();
    settle().await;
    assert_eq!(count(), 2, "and nothing after that");

    drop(refresher);
    refresher_stops_with_its_owner(&runs).await;
}

async fn refresher_stops_with_its_owner(runs: &Arc<std::sync::atomic::AtomicUsize>) {
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert_eq!(
        Arc::strong_count(runs),
        1,
        "dropping the refresher ends its task and frees what it held"
    );
}

#[test]
fn a_long_watch_that_closed_is_routine_and_anything_else_a_warning() {
    use engine_api::FailureClass;
    let long = HEALTHY_WATCH_UPTIME + Duration::from_secs(270);
    assert!(ended_routinely(long, FailureClass::Retryable));
    assert!(!ended_routinely(
        Duration::from_secs(2),
        FailureClass::Retryable
    ));
    assert!(!ended_routinely(long, FailureClass::Authentication));
    assert!(!ended_routinely(long, FailureClass::Permanent));
}

#[test]
fn backoff_escalates_then_settles_at_the_cap() {
    let mut backoff = Backoff::new("INBOX");
    let offset = backoff.offset;
    // base·2^0, ·2^1, ·2^2 …
    assert_eq!(backoff.next_delay(), RECONNECT_BACKOFF_BASE + offset);
    assert_eq!(backoff.next_delay(), RECONNECT_BACKOFF_BASE * 2 + offset);
    assert_eq!(backoff.next_delay(), RECONNECT_BACKOFF_BASE * 4 + offset);
    // …then every later delay is pinned to the cap (never the old busy-loop, never
    // unbounded growth).
    for _ in 0..12 {
        assert!(backoff.next_delay() <= RECONNECT_BACKOFF_MAX + offset);
    }
    assert_eq!(backoff.next_delay(), RECONNECT_BACKOFF_MAX + offset);
}

#[test]
fn reset_returns_to_the_base_delay() {
    let mut backoff = Backoff::new("Sent");
    let offset = backoff.offset;
    for _ in 0..8 {
        backoff.next_delay();
    }
    backoff.reset();
    assert_eq!(backoff.next_delay(), RECONNECT_BACKOFF_BASE + offset);
}

#[test]
fn the_per_watch_offset_is_bounded_and_stable() {
    // Under a second, and identical across restarts for the same folder (so staggering is
    // deterministic and logs stay reproducible).
    assert!(Backoff::new("INBOX").offset < Duration::from_secs(1));
    assert_eq!(Backoff::new("INBOX").offset, Backoff::new("INBOX").offset);
}
