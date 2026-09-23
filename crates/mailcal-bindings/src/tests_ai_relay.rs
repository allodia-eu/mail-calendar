//! Which backend AI requests go through in a build that carries the Allodia sign-in: the relay
//! for a signed-in account entitled to `ai`, an own endpoint over it when one is set up, and
//! nothing once nobody is signed in. The entitlement answer is written into the preferences before
//! launch, as a previous launch would have left it, so nothing here reaches the service.

use std::{
    collections::BTreeSet,
    sync::{Arc, mpsc},
};

use allodia_license::{Answer, Capability, Entitlement, Stored};

use crate::{
    AiRoute, JurisdictionClass, LogLevel, MailcalApp,
    tests::{ChannelObserver, NullLogger, RecordingCredentialStore, RecordingStoreHandle},
};

const GRANT: &str = "[allodia]\nemail = \"sam@example.eu\"\nrefresh_token = \"refresh\"\ngranted_scopes = [\"mailcal:entitlement:read\", \"mailcal:ai:use\"]\n";

/// A launch after one that learned the account's entitlement, fresh enough not to ask again.
fn boot(name: &str, capabilities: &[Capability]) -> (Arc<MailcalApp>, std::path::PathBuf) {
    let data_dir = crate::tests::temp_data_dir(name);
    std::fs::create_dir_all(&data_dir).unwrap();
    let stored = Stored {
        answer: Answer {
            entitlement: Entitlement {
                plan: "personal".to_owned(),
                capabilities: capabilities.iter().cloned().collect::<BTreeSet<_>>(),
                payment_status: None,
                current_period_end: None,
            },
            refresh_after_seconds: 86_400,
        },
        fetched_at: time::OffsetDateTime::now_utc().unix_timestamp(),
    };
    let mut prefs = mailcal_account::Preferences::default();
    prefs.ai.entitlement_answer = Some(serde_json::to_string(&stored).unwrap());
    mailcal_account::save_preferences(mailcal_account::preferences_path(&data_dir), &prefs)
        .unwrap();

    let (tx, _rx) = mpsc::channel();
    let app = MailcalApp::new_accounts(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        vec![GRANT.to_owned()],
        data_dir.to_string_lossy().into_owned(),
        "Etc/UTC".to_owned(),
        crate::analytics::test_device(),
        Box::new(RecordingStoreHandle(Arc::new(
            RecordingCredentialStore::default(),
        ))),
    )
    .expect("an app with no mail accounts boots");
    (app, data_dir)
}

#[test]
fn a_signed_in_account_entitled_to_ai_goes_through_the_relay() {
    let (app, data_dir) = boot(
        "relay-entitled",
        &[Capability::AccountsSync, Capability::Ai],
    );
    assert!(app.ai_available());
    assert_eq!(app.writing_styles().route, Some(AiRoute::Relay));
    // The relay is EU-native by construction, so the strictest mode admits it.
    assert!(app.writing_styles().refused.is_none());
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn a_plan_without_ai_offers_nothing() {
    let (app, data_dir) = boot("relay-not-entitled", &[Capability::AccountsSync]);
    assert!(!app.ai_available());
    assert!(app.writing_styles().route.is_none());
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn an_own_endpoint_wins_over_the_relay_and_signing_out_leaves_only_it() {
    let (app, data_dir) = boot("relay-and-own", &[Capability::Ai]);
    app.set_own_ai_endpoint(
        "http://localhost:11434/v1".to_owned(),
        "mistral-small".to_owned(),
        Some(JurisdictionClass::EuNative),
        None,
    )
    .unwrap();
    assert_eq!(app.writing_styles().route, Some(AiRoute::OwnEndpoint));

    app.clear_own_ai_endpoint().unwrap();
    assert_eq!(app.writing_styles().route, Some(AiRoute::Relay));

    app.sign_out_of_allodia().unwrap();
    assert!(app.writing_styles().route.is_none());
    // What the account was entitled to left with it.
    let prefs = mailcal_account::load_preferences(mailcal_account::preferences_path(&data_dir));
    assert!(prefs.ai.entitlement_answer.is_none());
    let _ = std::fs::remove_dir_all(data_dir);
}
