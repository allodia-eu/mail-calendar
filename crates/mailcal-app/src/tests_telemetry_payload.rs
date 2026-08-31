//! Consented-analytics tests, part two: **the payload cannot carry content.**
//!
//! A search query, an address, a subject, or a folder name must not reach the wire even when the
//! app is driven through the intents that carry them. The type system is supposed to make that
//! impossible; these tests check that it actually does, against a **real dispatch** rather than a
//! hand-built event; because the claim we make to users is about what actually leaves the device,
//! not about what a unit test constructs.
//!
//! The consent-gate half, and the shared fixtures, live in `tests_telemetry.rs`.

use engine_api::Engine;

use super::{
    App, SilentObserver, Telemetry, TimeZoneInit, app_with_analytics, device, one_account, scratch,
};
use crate::{FolderRef, Intent, PROPERTY_KEYS, Protocol};

/// The load-bearing privacy test. Drive a consented app through the intents that *do* carry
/// content (a search query, a message, an event title) and assert none of it reaches the wire.
///
/// This checks the real serialized bytes after a real dispatch, not a hand-built event, because
/// the claim being made to users is about what actually leaves the device.
#[tokio::test]
async fn no_content_reaches_the_wire() {
    let path = scratch("no-content");
    let (app, sink) = app_with_analytics(one_account(), path);
    app.set_accounts([("acct-1".to_owned(), Protocol::Imap)].into());
    app.set_analytics_consent(true);

    // Every one of these carries something that must never be sent.
    app.dispatch(Intent::Search(Some("severance package".to_owned())))
        .await;
    app.dispatch(Intent::SelectFolder {
        folder: FolderRef::from_parts("acct-1", "Legal/Confidential".to_owned()).unwrap(),
    })
    .await;
    app.dispatch(Intent::CreateEvent {
        title: "Divorce hearing".to_owned(),
        start: "2026-08-01T09:00:00Z".to_owned(),
        end: "2026-08-01T10:00:00Z".to_owned(),
        account: None,
        calendar: None,
        all_day: false,
        timezone: None,
        notes: Some("do not send this anywhere".to_owned()),
        location: Some("Courtroom 4B".to_owned()),
        recurrence: None,
    })
    .await;
    app.dispatch(Intent::SubmitMail {
        to: "lawyer@example.com".to_owned(),
        subject: "Re: settlement".to_owned(),
        body: "Please advise.".to_owned(),
    })
    .await;

    let wire = serde_json::to_string(&*sink.batches.lock().unwrap()).unwrap();
    for forbidden in [
        "severance",
        "package",
        "Legal",
        "Confidential",
        "Divorce",
        "hearing",
        // The event's notes/description and location; content, so never ours to send.
        "anywhere",
        "Courtroom",
        "lawyer",
        "example.com",
        "settlement",
        "advise",
        // The account's own identity and id; neither is ours to send.
        "me@acct-1.local",
        "@",
        "acct-1",
    ] {
        assert!(
            !wire.contains(forbidden),
            "{forbidden:?} reached the wire; payload was:\n{wire}"
        );
    }

    // …and the events themselves *were* recorded, so this isn't passing by sending nothing.
    assert!(wire.contains("search"), "the search was still counted");
    assert!(wire.contains("event_create"));
    assert!(wire.contains("composer_new"));
}

/// Every key we emit is on the whitelist the relay mirrors. If this fails, the relay would reject
/// the event in production: so the test is the contract between the two repositories.
#[tokio::test]
async fn every_property_key_is_whitelisted() {
    let path = scratch("keys");
    let (app, sink) = app_with_analytics(one_account(), path);
    app.set_accounts([("acct-1".to_owned(), Protocol::Jmap)].into());
    app.set_analytics_consent(true);
    app.dispatch(Intent::RefreshCalendar).await;

    let batches = sink.batches.lock().unwrap();
    assert!(!batches.is_empty());
    for batch in batches.iter() {
        for event in &batch.events {
            for key in event.properties.keys() {
                assert!(
                    PROPERTY_KEYS.contains(key),
                    "property key {key:?} is not in PROPERTY_KEYS: the relay would reject it"
                );
            }
        }
        // The context's own keys are fixed by its struct, but assert the serialization agrees so
        // a rename can't silently drift from the whitelist either.
        let context = serde_json::to_value(&batch.context).unwrap();
        for key in context.as_object().unwrap().keys() {
            assert!(
                PROPERTY_KEYS.contains(&key.as_str()),
                "context key {key:?} is not in PROPERTY_KEYS"
            );
        }
    }
}

/// The host's raw device facts are coarsened by the **core**, so no client can widen the payload
/// by reporting something more precise than we asked for.
#[tokio::test]
async fn the_core_coarsens_the_hosts_raw_device_facts() {
    let path = scratch("coarsen");
    // Three IMAP + one JMAP: enough accounts to land in a bucket rather than a raw count.
    let (app, sink) = app_with_analytics(one_account(), path);
    app.set_accounts(
        [
            ("a".to_owned(), Protocol::Imap),
            ("b".to_owned(), Protocol::Imap),
            ("c".to_owned(), Protocol::Imap),
            ("d".to_owned(), Protocol::Jmap),
        ]
        .into(),
    );
    app.set_analytics_consent(true);

    let batches = sink.batches.lock().unwrap();
    let context = &batches.first().expect("consent emits a batch").context;

    // The host said "15.4.1", only the major goes on the wire.
    assert_eq!(context.os_version, "15");
    // The host said "nl-NL", only the language we ship goes on the wire.
    assert_eq!(context.locale, "nl");
    // Four accounts is a bucket, not a count.
    assert_eq!(context.account_count, "3-5");
    // Protocols are unordered booleans, never a per-account tuple.
    assert!(context.has_imap && context.has_jmap && !context.has_graph);
    // A form factor, never a model string.
    assert_eq!(context.device_class, "mac-laptop");
}

/// The consent screen's "see exactly what we send" panel must show the truth. It is built from
/// the same `Batch` type the sink serializes, so the preview cannot drift from the wire; this
/// pins that.
#[tokio::test]
async fn the_payload_preview_matches_what_actually_goes_on_the_wire() {
    let path = scratch("preview");
    let (app, sink) = app_with_analytics(one_account(), path);
    app.set_accounts([("acct-1".to_owned(), Protocol::Graph)].into());

    // Before consent the preview is honest that no id exists yet.
    let before = app.analytics_payload_preview();
    assert!(before.contains("generated when you opt in"));
    assert!(before.contains("\"has_graph\": true"));

    app.set_analytics_consent(true);

    let preview: serde_json::Value =
        serde_json::from_str(&app.analytics_payload_preview()).unwrap();
    let sent = serde_json::to_value(
        sink.batches
            .lock()
            .unwrap()
            .iter()
            .find(|batch| batch.events.iter().any(|event| event.name == "app_opened"))
            .expect("consent emits an app_opened"),
    )
    .unwrap();

    assert_eq!(
        preview, sent,
        "the preview shown to the user must be the bytes we actually send"
    );
}

/// A build with **no relay baked in**, which is every local build, and any release built without
/// `ALLODIA_TELEMETRY_URL`; must still preview the payload *this device* would produce.
///
/// The regression this pins: the no-relay path used to fall back to `Telemetry::off`, which is the
/// demo/showcase shape and drops the device facts on the floor. The consent screen then rendered a
/// hollow payload (`os_version: "0"`, `device_class: "unknown"`, `app_version: "0.0.0"`) on the
/// one screen whose entire purpose is to tell the user the truth about what we send. Nothing caught
/// it, because every other test wires a sink.
#[tokio::test]
async fn a_build_with_no_relay_still_previews_the_real_device() {
    let path = scratch("unsent");
    let app = App::new(
        Engine::open_in_memory().unwrap(),
        one_account(),
        TimeZoneInit {
            device_zone: engine_api::TimeZoneId::utc(),
            prefs_path: Some(path.clone()),
        },
        None,
        std::sync::Arc::new(SilentObserver),
        // Exactly what `build_telemetry` constructs when `ALLODIA_TELEMETRY_URL` is absent.
        Telemetry::unsent(Some(path), device()),
    );

    let preview: serde_json::Value =
        serde_json::from_str(&app.analytics_payload_preview()).unwrap();
    let context = &preview["context"];

    assert_eq!(context["platform"], "macos");
    assert_eq!(context["os_version"], "15", "the real device, coarsened");
    assert_eq!(context["device_class"], "mac-laptop");
    assert_eq!(context["app_version"], "1.4.0");
    assert_eq!(context["locale"], "nl");
}
