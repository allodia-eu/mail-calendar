//! The own AI endpoint through the FFI: its key goes to the secure store and comes back at the
//! next launch without reaching the mail parsers, its settings decide the backend, and the gate's
//! verdict on its declaration is on the Writing style surface before anything is asked.

use std::sync::{Arc, mpsc};

use crate::{
    AiRoute, JurisdictionClass, LogLevel, MailcalApp, OwnEndpointError,
    tests::{ChannelObserver, NullLogger, RecordingCredentialStore, RecordingStoreHandle},
};

fn boot(
    data_dir: &std::path::Path,
    configs: Vec<String>,
    store: &Arc<RecordingCredentialStore>,
) -> Arc<MailcalApp> {
    let (tx, _rx) = mpsc::channel();
    MailcalApp::new_accounts(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        configs,
        data_dir.to_string_lossy().into_owned(),
        "Etc/UTC".to_owned(),
        crate::analytics::test_device(),
        Box::new(RecordingStoreHandle(Arc::clone(store))),
    )
    .expect("an app with no accounts boots")
}

#[test]
fn an_own_endpoint_is_saved_restored_and_removed() {
    let data_dir = crate::tests::temp_data_dir("ai-endpoint");
    let store = Arc::new(RecordingCredentialStore::default());
    let app = boot(&data_dir, Vec::new(), &store);
    assert!(!app.ai_available());
    assert!(app.writing_styles().route.is_none());

    app.set_own_ai_endpoint(
        "http://localhost:11434/v1".to_owned(),
        "mistral-small".to_owned(),
        Some(JurisdictionClass::EuNative),
        Some("sk-local".to_owned()),
    )
    .unwrap();

    assert!(app.ai_available());
    let snapshot = app.writing_styles();
    assert_eq!(snapshot.route, Some(AiRoute::OwnEndpoint));
    assert!(snapshot.refused.is_none());
    let persisted = store.persisted.lock().unwrap().clone();
    assert_eq!(persisted.len(), 1);
    assert_eq!(persisted[0].0, "ai-endpoint");
    assert!(persisted[0].1.contains("sk-local"));
    let endpoint = app.own_ai_endpoint().unwrap();
    assert!(endpoint.has_key);
    assert_eq!(endpoint.base_url, "http://localhost:11434/v1");

    // The next launch finds the key among the stored configs, keeps it out of the mail parsers
    // and builds the same backend.
    drop(app);
    let relaunched = boot(&data_dir, vec![persisted[0].1.clone()], &store);
    assert!(relaunched.ai_available());
    assert!(relaunched.account_connect_error().is_none());
    assert!(relaunched.own_ai_endpoint().unwrap().has_key);

    relaunched.clear_own_ai_endpoint().unwrap();
    assert!(!relaunched.ai_available());
    assert!(relaunched.own_ai_endpoint().is_none());
    assert_eq!(store.deleted.lock().unwrap().as_slice(), ["ai-endpoint"]);
    let _ = std::fs::remove_dir_all(data_dir);
}

/// A production build offers writing style only behind an entitlement: an own endpoint set up
/// earlier installs no backend and draws no category, and the snapshot says it is not offered.
#[test]
fn a_production_build_without_an_entitlement_offers_nothing() {
    let data_dir = crate::tests::temp_data_dir("ai-endpoint-production");
    let store = Arc::new(RecordingCredentialStore::default());
    let app = boot(&data_dir, Vec::new(), &store);
    assert!(app.writing_styles().offered);
    app.set_own_ai_endpoint(
        "http://localhost:11434/v1".to_owned(),
        "mistral-small".to_owned(),
        Some(JurisdictionClass::EuNative),
        None,
    )
    .unwrap();
    assert!(app.ai_available());

    app.as_production_build();

    assert!(!app.ai_available());
    let snapshot = app.writing_styles();
    assert!(!snapshot.offered);
    assert!(snapshot.route.is_none());
    // What was set up stays, for the day the person is offered it.
    assert!(app.own_ai_endpoint().is_some());
    let _ = std::fs::remove_dir_all(data_dir);
}

/// An edit that leaves the key out keeps the stored one: the Advanced screen can save a new model
/// without asking for the key again.
#[test]
fn an_edit_without_a_key_keeps_the_stored_one() {
    let data_dir = crate::tests::temp_data_dir("ai-endpoint-edit");
    let store = Arc::new(RecordingCredentialStore::default());
    let app = boot(&data_dir, Vec::new(), &store);
    app.set_own_ai_endpoint(
        "https://api.example.eu/v1".to_owned(),
        "small".to_owned(),
        Some(JurisdictionClass::EuNative),
        Some("sk-1".to_owned()),
    )
    .unwrap();

    app.set_own_ai_endpoint(
        "https://api.example.eu/v1".to_owned(),
        "large".to_owned(),
        Some(JurisdictionClass::EuNative),
        None,
    )
    .unwrap();

    let endpoint = app.own_ai_endpoint().unwrap();
    assert_eq!(endpoint.model, "large");
    assert!(endpoint.has_key);
    assert_eq!(store.persisted.lock().unwrap().len(), 1);
    let _ = std::fs::remove_dir_all(data_dir);
}

/// The strictest mode is the default, so an endpoint declared outside the EU is on the surface as
/// a refusal before the person asks for anything.
#[test]
fn a_declaration_the_mode_does_not_admit_is_reported_as_a_refusal() {
    let data_dir = crate::tests::temp_data_dir("ai-endpoint-refused");
    let store = Arc::new(RecordingCredentialStore::default());
    let app = boot(&data_dir, Vec::new(), &store);
    app.set_own_ai_endpoint(
        "https://api.example.com/v1".to_owned(),
        "gpt".to_owned(),
        Some(JurisdictionClass::NonEu),
        None,
    )
    .unwrap();

    let refused = app.writing_styles().refused.expect("the gate refuses");
    assert_eq!(refused.class, JurisdictionClass::NonEu);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn an_unusable_address_saves_nothing() {
    let data_dir = crate::tests::temp_data_dir("ai-endpoint-invalid");
    let store = Arc::new(RecordingCredentialStore::default());
    let app = boot(&data_dir, Vec::new(), &store);

    let error = app
        .set_own_ai_endpoint(
            "http://192.168.1.20:11434/v1".to_owned(),
            "m".to_owned(),
            None,
            Some("sk".to_owned()),
        )
        .unwrap_err();

    assert_eq!(error, OwnEndpointError::NotHttps);
    assert!(store.persisted.lock().unwrap().is_empty());
    assert!(app.own_ai_endpoint().is_none());
    assert!(!app.ai_available());
    let _ = std::fs::remove_dir_all(data_dir);
}
