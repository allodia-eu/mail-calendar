//! The per-account **message-size** setting: what an account keeps offline, and what changing
//! it does to the mail already cached.
//!
//! The warm's own behaviour under a cap is `tests_warm.rs`; this is about the setting, that it
//! survives its neighbours, that it falls back to the device's default, and that the two
//! directions of a change do opposite things (lowering forgets, raising fetches).

use std::sync::{Arc, Mutex, atomic::Ordering};

use fakes::{FakeProvider, account, app, message, msg};

use super::Intent;

#[allow(clippy::duplicate_mod)]
#[path = "tests_fakes.rs"]
mod fakes;

#[tokio::test]
async fn an_account_with_no_choice_of_its_own_uses_the_devices_default() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", FakeProvider::new())], &surfaces);

    assert_eq!(
        app.effective_message_size_limit("acct-1"),
        crate::default_prefetch_size_limit(),
        "an untouched account resolves to the form factor's answer, not a hardcoded number",
    );
}

#[tokio::test]
async fn a_stored_choice_wins_over_the_devices_default() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", FakeProvider::new())], &surfaces);

    app.set_account_message_size_limit("acct-1", 5).await;
    assert_eq!(
        app.effective_message_size_limit("acct-1"),
        Some(5 * 1024 * 1024),
    );

    // `0` is the no-limit sentinel, and it must beat a device default that *has* a cap;
    // "unlimited" is a choice, not the absence of one.
    app.set_account_message_size_limit("acct-1", 0).await;
    assert_eq!(app.effective_message_size_limit("acct-1"), None);
}

#[tokio::test]
async fn changing_a_neighbouring_setting_keeps_the_size_choice() {
    // Every setter rebuilds the whole stored record, so a field one of them forgets is silently
    // reset by an unrelated change: the user picks 5 MB, later changes their poll interval, and
    // their cap is back to the default with nothing said.
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", FakeProvider::new())], &surfaces);
    app.set_account_message_size_limit("acct-1", 5).await;

    app.set_poll_interval("acct-1", 60).await;
    assert_eq!(
        app.effective_message_size_limit("acct-1"),
        Some(5 * 1024 * 1024),
        "changing the poll interval must not reset the size cap",
    );

    app.set_account_sync_depth("acct-1", 12).await;
    assert_eq!(
        app.effective_message_size_limit("acct-1"),
        Some(5 * 1024 * 1024),
        "changing the sync depth must not reset the size cap",
    );
}

#[tokio::test]
async fn the_snapshot_offers_the_options_and_marks_the_one_in_effect() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", FakeProvider::new())], &surfaces);
    app.set_account_message_size_limit("acct-1", 10).await;

    let snapshot = app.sync_settings().await;
    assert_eq!(
        snapshot.message_size_limits_mb,
        vec![2, 5, 10, 0],
        "a client builds its picker from the core's list rather than hardcoding it",
    );
    assert_eq!(snapshot.accounts[0].message_size_limit_mb, 10);
}

#[tokio::test]
async fn lowering_the_cap_keeps_the_mail_readable() {
    // The deliberate half of the trade, and the one most likely to be "helpfully" undone: a
    // lowered cap forgets the *bytes*, never the mail. The extracted text stays, so the message
    // is still listed and still reads offline; dropping that too would shrink body search to
    // reclaim space that is not in it. That the bytes really go is asserted in the engine's own
    // suite, which can see the blob area; here the point is that this did not take the mail.
    let mut huge = message("huge", "a", "Holiday photos");
    huge.size = Some(8 * 1024 * 1024);
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let provider = FakeProvider::with(vec![huge]);
    let offline = provider.failure_switch();
    let app = app(vec![account("acct-1", provider)], &surfaces);

    app.set_account_message_size_limit("acct-1", 0).await;
    app.dispatch(Intent::RefreshMail).await;
    app.update_account_message_size_limit("acct-1", 2).await;

    assert!(
        !app.mailbox_list().rows.is_empty(),
        "the message is still listed",
    );
    offline.store(true, Ordering::SeqCst);
    app.dispatch(Intent::OpenMessage {
        message: msg("acct-1", "huge"),
    })
    .await;
    assert!(
        !app.reading_view().load_error,
        "and still reads offline: only its offline copy of the raw bytes went",
    );
}

#[tokio::test]
async fn raising_the_cap_fetches_what_the_lower_one_skipped() {
    let mut huge = message("huge", "a", "Holiday photos");
    huge.size = Some(8 * 1024 * 1024);
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let provider = FakeProvider::with(vec![huge]);
    let offline = provider.failure_switch();
    let app = app(vec![account("acct-1", provider)], &surfaces);

    app.set_account_message_size_limit("acct-1", 2).await;
    app.dispatch(Intent::RefreshMail).await;

    // Skipped by the warm; with the provider down it cannot be read.
    offline.store(true, Ordering::SeqCst);
    app.dispatch(Intent::OpenMessage {
        message: msg("acct-1", "huge"),
    })
    .await;
    assert!(app.reading_view().load_error, "skipped under the low cap");

    // Raise it with the provider reachable again: the change itself fetches, with no sync in
    // between, because a user who just asked for more offline mail should not have to wait for
    // the next refresh to get it.
    offline.store(false, Ordering::SeqCst);
    app.update_account_message_size_limit("acct-1", 0).await;
    offline.store(true, Ordering::SeqCst);
    app.dispatch(Intent::OpenMessage {
        message: msg("acct-1", "huge"),
    })
    .await;
    assert!(
        !app.reading_view().load_error,
        "raising the cap downloads what it now admits",
    );
}

#[tokio::test]
async fn raising_the_cap_puts_back_the_bytes_a_lowering_dropped() {
    // The cycle a size picker makes easy: keep everything, keep less, keep everything again.
    // A body read is text-first; once the extracted text is cached it returns without touching
    // the bytes: so nothing in an open can notice the raw source went, and only a warm asking
    // for it explicitly brings it back. Without that the message stays on the work list for
    // ever with its attachments and inline images no longer local.
    //
    // The fixture's source is genuinely over the cap, because the drop measures the bytes on
    // disk rather than the size the server reported: a message that only *claims* to be large
    // is never dropped, and this would then pass over a warm that puts nothing back.
    let mut raw = b"Content-Type: text/plain; charset=utf-8\r\n\r\n".to_vec();
    raw.resize(raw.len() + 3 * 1024 * 1024, b'x');
    let mut huge = message("huge", "a", "Holiday photos");
    huge.size = Some(8 * 1024 * 1024);
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let provider = FakeProvider::with(vec![huge]).with_source(&raw);
    let fetches = provider.source_fetches();
    let app = app(vec![account("acct-1", provider)], &surfaces);

    app.set_account_message_size_limit("acct-1", 0).await;
    app.dispatch(Intent::RefreshMail).await;
    let warmed = fetches.load(Ordering::SeqCst);
    assert!(warmed > 0, "the warm cached the bytes once");

    app.update_account_message_size_limit("acct-1", 2).await;
    app.update_account_message_size_limit("acct-1", 0).await;

    assert!(
        fetches.load(Ordering::SeqCst) > warmed,
        "raising it again fetched the dropped bytes back",
    );
}

#[test]
fn no_limit_counts_as_larger_than_any_cap() {
    // Which direction a change went decides whether it forgets or fetches, and "unlimited" is
    // the value that makes an `Option` comparison lie if it is treated as absent.
    use crate::message_size::is_smaller;
    let two = Some(2 * 1024 * 1024);
    let ten = Some(10 * 1024 * 1024);

    assert!(is_smaller(two, ten), "10 MB -> 2 MB keeps less");
    assert!(!is_smaller(ten, two), "2 MB -> 10 MB keeps more");
    assert!(is_smaller(two, None), "unlimited -> 2 MB keeps less");
    assert!(!is_smaller(None, two), "2 MB -> unlimited keeps more");
    assert!(!is_smaller(None, None), "unchanged is not smaller");
}
