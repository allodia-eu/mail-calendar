//! Gated live check that the **product** JMAP account path works end to end against
//! the Stalwart harness: build a [`JmapAccountConfig`] from Basic credentials, connect
//! its provider, sync mailboxes + email, and download a message's raw source through
//! the engine's `fetch_message_source` (the reading path).
//!
//! This exercises the product core's wiring, which the engine's own tests don't; the
//! product config → engine `JmapConfig` mapping, the account-id scheme, and Basic auth
//! : not just the engine adapter. It **skips** unless `STALWART_HTTP_ADDR` is set, so
//! the offline `cargo test` stays green; CI sets it after `docker compose up --wait`.
//!
//! Run locally:
//! ```sh
//! (cd docker/stalwart && docker compose up -d --wait)
//! STALWART_HTTP_ADDR=127.0.0.1:28080 STALWART_ACCOUNT=alice@test.local \
//!   STALWART_PASSWORD=harness-alice-pw cargo test -p mailcal-account --test live_jmap -- --nocapture
//! ```

use engine_api::{
    AccountId, Engine, Event, EventDeletion, Horizon, Reconciled, TimeZoneId, UtcDateTime,
    WriteGuard,
};
use engine_core::{sync::SyncUpdate, time::LocalDateTime};
use engine_provider::Provider;
use mailcal_account::{
    EventEdit, JmapAccountConfig, Secret, build_event_draft, build_event_patch,
    connect_jmap_calendar_providers, connect_jmap_mail_providers,
};

/// The harness connection details from the environment, or `None` to skip.
fn harness() -> Option<(String, String, String)> {
    let addr = std::env::var("STALWART_HTTP_ADDR").ok()?;
    let account =
        std::env::var("STALWART_ACCOUNT").unwrap_or_else(|_| "alice@test.local".to_owned());
    let password =
        std::env::var("STALWART_PASSWORD").unwrap_or_else(|_| "harness-alice-pw".to_owned());
    Some((addr, account, password))
}

/// A Basic-auth JMAP config pointed at the loopback harness (plaintext HTTP).
fn basic_config(addr: &str, account: &str, password: &str) -> JmapAccountConfig {
    JmapAccountConfig {
        email: account.to_owned(),
        base_url: format!("http://{addr}"),
        password: Some(Secret::new(password.to_owned())),
        token: None,
        oauth: None,
    }
}

#[tokio::test]
async fn jmap_account_connects_syncs_and_reads_a_body() {
    let Some((addr, account, password)) = harness() else {
        eprintln!("skipping jmap live test: STALWART_HTTP_ADDR unset");
        return;
    };
    let config = basic_config(&addr, &account, &password);
    let id = config.account_id().expect("account id");

    // One provider covers the whole account (JMAP's scope is account-wide).
    let providers = connect_jmap_mail_providers(&config, None)
        .await
        .expect("connect JMAP provider");
    assert_eq!(providers.len(), 1, "one account-wide JMAP provider");
    let provider = providers.first().expect("a provider");
    // The engine advertises message-source support now that a download template exists,
    // so the reading path will work.
    assert!(
        provider.connection_info().capabilities.message_source(),
        "JMAP advertises on-demand message-source fetch"
    );
    // And the width the body warm paces itself by, which the server states in its session
    // (RFC 8620 §2 `maxConcurrentRequests`; the harness grants 4). This is the basic-auth
    // path, so what it proves is the adapter reaching the host through `connect_one`; the
    // OAuth path wraps the provider and has to forward the value instead, which is
    // `delegate_info`'s job, covered by its own tests.
    assert!(
        provider.connection_info().concurrent_fetches > 1,
        "the session's concurrency limit reaches the host, rather than the default of one",
    );

    // Mailboxes sync (the folder sidebar).
    let mailboxes = provider
        .sync_mailboxes(&id, None)
        .await
        .expect("sync mailboxes");
    let mailbox_count = match &mailboxes.update {
        SyncUpdate::Snapshot { objects, .. } => objects.len(),
        SyncUpdate::Delta { changed, .. } => changed.len(),
    };
    assert!(mailbox_count > 0, "the account has mailboxes");

    // Email syncs; find a known seed message and read its full body via blob download.
    let emails = provider.sync_email(&id, None).await.expect("sync email");
    let SyncUpdate::Snapshot { objects, .. } = &emails.update else {
        panic!("first email sync is a snapshot");
    };
    let seed = objects
        .iter()
        .find(|message| message.envelope.subject.as_deref() == Some("Harness baseline message"))
        .expect("the seeded baseline message is present");
    assert!(
        seed.blob_id.is_some(),
        "the synced message carries its blobId"
    );

    let raw = provider
        .fetch_message_source(&id, seed)
        .await
        .expect("download the raw message source");
    let text = String::from_utf8_lossy(raw.as_bytes());
    assert!(
        text.contains("Subject: Harness baseline message"),
        "the downloaded bytes are the real RFC 5322 source: {text}"
    );
}

/// The headline of the write migration, proven live: a JMAP calendar account can
/// **create → read-its-own-write → patch → delete** through the same host code CalDAV
/// uses. Before the migration this failed silently with `InvalidState`, because JMAP has no
/// whole-document `put_event` verb; the intent-carrying `EventDraft` / `EventPatch` /
/// `EventDeletion` path is what makes it work.
///
/// It also proves two claims the offline tests structurally cannot: **read-your-writes**
/// (the store holds the server's copy the instant the call returns: no second sync), and
/// that the adapter, not the core builder, is what **refuses an inverted edit**.
#[tokio::test]
async fn jmap_calendar_create_read_patch_delete_round_trips() {
    let Some((addr, account, password)) = harness() else {
        eprintln!("skipping jmap calendar live test: STALWART_HTTP_ADDR unset");
        return;
    };
    let config = basic_config(&addr, &account, &password);
    let id = config.account_id().expect("account id");

    // The JMAP *calendar* provider: the same neutral `Provider` the app writes through.
    let providers = connect_jmap_calendar_providers(&config, None)
        .await
        .expect("connect JMAP calendar provider");
    let provider = providers.first().expect("a calendar provider");
    // JMAP now advertises writable calendars, but with no per-object guard, so a stale edit
    // is last-writer-wins. This is exactly the `WriteGuard::Absent` the core collapses to
    // `can_write: true`.
    assert_eq!(
        provider
            .connection_info()
            .capabilities
            .calendar_write_guard(),
        Some(WriteGuard::Absent),
        "JMAP advertises writable calendars without a per-object precondition"
    );

    let engine = Engine::open_in_memory().expect("in-memory store");
    let zone = TimeZoneId::utc();
    let horizon = wide_horizon();

    // Precondition: a calendar must be synced once before a write (the reconcile needs an
    // expansion window). The sync is also how we learn a `CalendarId` to create into.
    engine
        .sync_calendar(provider, &id, horizon, &zone)
        .await
        .expect("initial calendar sync");
    let calendar = engine
        .calendars(&id)
        .await
        .expect("read calendars")
        .into_iter()
        .next()
        .expect("the harness seeds a calendar");

    // Unique per run so a shared harness (and a re-run after a mid-test failure) never
    // collides; the delete at the end removes it on the happy path.
    let uid = format!("core-live-{}@test.local", unique_suffix());

    // CREATE; via the very builder `App::create_event` uses.
    let draft = build_event_draft(
        calendar.id.clone(),
        &uid,
        "Core live create",
        "2026-09-01T09:00:00Z",
        "2026-09-01T09:30:00Z",
        false,
        None,
        None,
        Some("Core live room"),
        None,
        now(),
    )
    .expect("build create draft");
    let write = engine
        .create_calendar_event(provider, &id, "core-live-create", &draft)
        .await
        .expect("create the event");
    assert!(
        matches!(write.reconciled, Reconciled::Applied(_)),
        "the create reconciled into the store (read-your-writes)"
    );

    // READ-YOUR-WRITE: no second sync; the store already holds the server's copy.
    let stored = find_by_uid(&engine, &id, &uid)
        .await
        .expect("the created event is in the store with no re-sync");
    assert_eq!(stored.title, "Core live create");
    assert_eq!(
        stored.locations.first().and_then(|l| l.name.as_deref()),
        Some("Core live room"),
        "the location stated on the create survived the server round-trip"
    );

    // PATCH: a retitle, again visible without a re-sync.
    let (target, patch) = build_event_patch(
        &stored,
        &EventEdit {
            title: Some("Core live edit".to_owned()),
            ..EventEdit::default()
        },
        now(),
    )
    .expect("build retitle patch");
    engine
        .patch_calendar_event(provider, &id, "core-live-patch", &stored, target, patch)
        .await
        .expect("patch the event");
    let edited = find_by_uid(&engine, &id, &uid)
        .await
        .expect("the edited event is in the store");
    assert_eq!(edited.title, "Core live edit");

    // The core builder no longer judges the interval; the adapter refuses an inverted edit.
    // This is what makes dropping the hand-rolled `end <= start` guard safe.
    let (bad_target, bad_patch) = build_event_patch(
        &edited,
        &EventEdit {
            start: Some(LocalDateTime::new(2026, 9, 1, 10, 0, 0).unwrap()),
            end: Some(LocalDateTime::new(2026, 9, 1, 9, 0, 0).unwrap()),
            ..EventEdit::default()
        },
        now(),
    )
    .expect("the builder emits the intent without judging it");
    assert!(
        engine
            .patch_calendar_event(
                provider,
                &id,
                "core-live-bad",
                &edited,
                bad_target,
                bad_patch
            )
            .await
            .is_err(),
        "the adapter refuses an end-before-start edit"
    );

    // DELETE; guarded on the revision we read, and tombstoned locally without a sync.
    let deletion = EventDeletion::of(&edited);
    engine
        .delete_calendar_event(provider, &id, "core-live-delete", Some(&edited), &deletion)
        .await
        .expect("delete the event");
    assert!(
        find_by_uid(&engine, &id, &uid).await.is_none(),
        "the delete tombstoned the event locally, no re-sync needed"
    );
}

/// A horizon wide enough to hold the fixed 2026 test event regardless of the harness clock.
fn wide_horizon() -> Horizon {
    let start = UtcDateTime::new(2026, 1, 1, 0, 0, 0).unwrap();
    let end = UtcDateTime::new(2028, 1, 1, 0, 0, 0).unwrap();
    Horizon::new(start, end).unwrap()
}

/// The system clock as a civil UTC time: the `DTSTAMP` a create/patch needs.
fn now() -> UtcDateTime {
    let now = time::OffsetDateTime::now_utc();
    UtcDateTime::new(
        now.year(),
        u8::from(now.month()),
        now.day(),
        now.hour(),
        now.minute(),
        now.second(),
    )
    .expect("a civil UTC time from the system clock is representable")
}

/// A process-unique suffix, so concurrent/re-run live tests never share an event uid.
fn unique_suffix() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("after the epoch")
        .as_nanos()
}

/// The stored master [`Event`] carrying `uid`, or `None` if the store has none.
async fn find_by_uid(engine: &Engine, id: &AccountId, uid: &str) -> Option<Event> {
    engine
        .events(id)
        .await
        .unwrap_or_default()
        .into_iter()
        .find(|event| event.uid.as_str() == uid)
}
