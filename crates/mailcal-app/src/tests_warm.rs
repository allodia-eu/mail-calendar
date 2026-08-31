//! Body-warm tests for [`super::App`]: the background pass that fills the store's body cache
//! after a sync, so opens are instant and the synced window reads offline.
//!
//! Three properties, each of which has been wrong at some point: it covers the **whole**
//! window rather than a newest-N slice; it **overlaps** its fetches to whatever width the
//! transport reports, because one round trip per message is the whole cost of a first sync on
//! an HTTP provider, and it leaves a message **too large to be worth pre-fetching** to the open
//! that asks for it. The shared fixtures live in `tests_fakes.rs`.

use std::sync::{Arc, Mutex, atomic::Ordering};

use fakes::{FakeProvider, account, app, message, msg};

use super::Intent;

#[allow(clippy::duplicate_mod)]
#[path = "tests_fakes.rs"]
mod fakes;

#[tokio::test]
async fn a_refresh_warms_every_body_so_opens_work_offline_with_no_prior_open() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let provider = FakeProvider::new(); // its inbox holds m1, m2
    let offline = provider.failure_switch();
    let app = app(vec![account("acct-1", provider)], &surfaces);

    // A routine refresh both syncs the metadata AND warms every body in the window; no
    // explicit prefetch call, no prior open. (The old behaviour warmed only on account-add,
    // so mail synced by a later refresh was unreadable offline until first opened.)
    app.dispatch(Intent::RefreshMail).await;
    offline.store(true, Ordering::SeqCst);

    for key in ["m1", "m2"] {
        app.dispatch(Intent::OpenMessage {
            message: msg("acct-1", key),
        })
        .await;
        let reading = app.reading_view();
        assert!(
            !reading.load_error,
            "{key}: the refresh warmed the body, so it opens with the provider down",
        );
        assert!(
            reading.plain.is_some(),
            "{key}: the cached body is readable offline"
        );
    }
}

#[tokio::test]
async fn the_warm_overlaps_its_fetches_when_the_transport_says_it_can() {
    // Each body is one round trip, and a serial warm pays that latency once per message;
    // on a real Gmail account that was 827 bodies at ~400ms each, minutes behind a list the
    // user was already looking at. The transport is asked how wide it can go; here it says 8.
    let messages: Vec<_> = (0..40)
        .map(|i| message(&format!("m{i}"), "a", &format!("Subject {i}")))
        .collect();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let provider = FakeProvider::with(messages).with_concurrent_fetches(8);
    let peak = provider.in_flight_peak();
    let app = app(vec![account("acct-1", provider)], &surfaces);

    app.dispatch(Intent::RefreshMail).await;

    // Both halves of the contract, and neither is an exact count: how many of a wave are
    // in flight at the instant the counter is read depends on the executor's polling order,
    // so pinning it to exactly the width would be a flake rather than a stricter test.
    let observed = peak.lock().unwrap().1;
    assert!(
        observed > 1,
        "the warm must overlap its fetches; a serial drain peaks at 1, this peaked at {observed}",
    );
    assert!(
        observed <= 8,
        "the transport's width is a ceiling, not a suggestion; this peaked at {observed}",
    );
}

#[tokio::test]
async fn the_warm_stays_serial_on_a_transport_that_shares_one_socket() {
    // The other half of the same rule: IMAP's commands share a connection, so overlapping
    // fetches would only queue behind themselves. A width of 1 must stay a width of 1.
    let messages: Vec<_> = (0..12)
        .map(|i| message(&format!("m{i}"), "a", &format!("Subject {i}")))
        .collect();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let provider = FakeProvider::with(messages).with_concurrent_fetches(1);
    let peak = provider.in_flight_peak();
    let app = app(vec![account("acct-1", provider)], &surfaces);

    app.dispatch(Intent::RefreshMail).await;

    assert_eq!(
        peak.lock().unwrap().1,
        1,
        "a single-socket transport is not fanned out"
    );
}

#[tokio::test]
async fn the_warm_leaves_an_oversized_message_for_the_open_that_asks_for_it() {
    // A warm pulls the whole raw source, attachments included, so one heavy message can cost
    // more than the rest of the mailbox. Past the cap it is skipped, and still opens, because
    // the on-demand read fetches and caches it then.
    let mut small = message("small", "a", "A reply");
    small.size = Some(4 * 1024);
    let mut huge = message("huge", "a", "Holiday photos");
    huge.size = Some(64 * 1024 * 1024);
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let provider = FakeProvider::with(vec![small, huge]);
    let offline = provider.failure_switch();
    let app = app(vec![account("acct-1", provider)], &surfaces);

    // Set the cap rather than lean on the default: what the default *is* differs by form
    // factor, and this is about what a cap does once there is one.
    let shared: &crate::App<_> = &app;
    shared.set_account_message_size_limit("acct-1", 2).await;

    app.dispatch(Intent::RefreshMail).await;
    offline.store(true, Ordering::SeqCst);

    // The small one warmed, so it reads with the provider down.
    app.dispatch(Intent::OpenMessage {
        message: msg("acct-1", "small"),
    })
    .await;
    assert!(!app.reading_view().load_error, "the small body was warmed");

    // The oversized one was left alone; offline it cannot be read, which is the trade.
    app.dispatch(Intent::OpenMessage {
        message: msg("acct-1", "huge"),
    })
    .await;
    assert!(
        app.reading_view().load_error,
        "an oversized body is not warmed; it is fetched when someone opens it",
    );
}

#[tokio::test]
async fn raising_the_cap_warms_what_the_default_would_have_skipped() {
    // The knob a Settings control would drive. Asserted through the same door a host has;
    // a shared `&App`, never a `&mut`; because a setter a host cannot reach is not a knob.
    let mut huge = message("huge", "a", "Holiday photos");
    huge.size = Some(64 * 1024 * 1024);
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let provider = FakeProvider::with(vec![huge]);
    let offline = provider.failure_switch();
    let app = app(vec![account("acct-1", provider)], &surfaces);

    // `None` is "warm every size, whatever it costs": the metered-link escape hatch.
    // A shared borrow is all an `Arc<App>` can ever hand out, so this is the reachable shape.
    let shared: &crate::App<_> = &app;
    shared.set_account_message_size_limit("acct-1", 0).await;

    app.dispatch(Intent::RefreshMail).await;
    offline.store(true, Ordering::SeqCst);
    app.dispatch(Intent::OpenMessage {
        message: msg("acct-1", "huge"),
    })
    .await;
    assert!(
        !app.reading_view().load_error,
        "with the cap lifted the oversized body warmed like any other",
    );
}

#[tokio::test]
async fn the_warm_covers_a_message_whose_size_the_adapter_never_reported() {
    // An absent size is no opinion, never "small"; Graph reports none at all, and treating
    // that as over the cap would silently stop warming an entire provider's mail.
    let mut unknown = message("unknown", "a", "No size reported");
    unknown.size = None;
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let provider = FakeProvider::with(vec![unknown]);
    let offline = provider.failure_switch();
    let app = app(vec![account("acct-1", provider)], &surfaces);

    app.dispatch(Intent::RefreshMail).await;
    offline.store(true, Ordering::SeqCst);

    app.dispatch(Intent::OpenMessage {
        message: msg("acct-1", "unknown"),
    })
    .await;
    assert!(
        !app.reading_view().load_error,
        "a message with no reported size is warmed like any other",
    );
}

#[tokio::test]
async fn the_warm_pass_covers_the_whole_window_not_a_newest_cap() {
    // More messages than the old 500-newest prefetch cap: with the cap, the tail of the
    // window stayed cold forever; the pass must drain the engine's missing-body list
    // completely.
    let messages: Vec<_> = (0..600)
        .map(|i| message(&format!("m{i}"), "a", &format!("Subject {i}")))
        .collect();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let provider = FakeProvider::with(messages);
    let offline = provider.failure_switch();
    let app = app(vec![account("acct-1", provider)], &surfaces);

    app.dispatch(Intent::RefreshMail).await;
    offline.store(true, Ordering::SeqCst);

    // Message #599 is deep past any 500-message cap; it must still read from the cache.
    app.dispatch(Intent::OpenMessage {
        message: msg("acct-1", "m599"),
    })
    .await;
    let reading = app.reading_view();
    assert!(
        !reading.load_error,
        "the pass warmed the whole window, not just the newest 500",
    );
    assert!(reading.plain.is_some());
}

#[tokio::test]
async fn a_fresh_app_starts_on_the_cap_its_form_factor_chose() {
    // The seam is only worth having if the runtime reads it, so assert the wiring rather than
    // the number: `form_factor`'s own tests own which value each side gets.
    let mut huge = message("huge", "a", "Holiday photos");
    huge.size = Some(64 * 1024 * 1024);
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let provider = FakeProvider::with(vec![huge]);
    let offline = provider.failure_switch();
    let app = app(vec![account("acct-1", provider)], &surfaces);

    app.dispatch(Intent::RefreshMail).await;
    offline.store(true, Ordering::SeqCst);
    app.dispatch(Intent::OpenMessage {
        message: msg("acct-1", "huge"),
    })
    .await;

    let capped = crate::default_prefetch_size_limit().is_some_and(|cap| cap < 64 * 1024 * 1024);
    assert_eq!(
        app.reading_view().load_error,
        capped,
        "a build that caps the warm leaves this body for the open; one that does not warms it",
    );
}
