use std::sync::{Arc, mpsc};

use super::{OAuthAccount, PendingConsent, Provider, same_address};
use crate::{
    ConnectedAccount, LogLevel, MailcalApp, MailcalError,
    tests::{
        ChannelObserver, NullLogger, RecordingCredentialStore, RecordingStoreHandle, temp_data_dir,
    },
};

fn app(name: &str) -> Arc<MailcalApp> {
    let (tx, _rx) = mpsc::channel();
    MailcalApp::new_accounts(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        Vec::new(),
        temp_data_dir(name).to_string_lossy().into_owned(),
        "Etc/UTC".to_owned(),
        crate::analytics::test_device(),
        Box::new(RecordingStoreHandle(Arc::new(
            RecordingCredentialStore::default(),
        ))),
    )
    .expect("an account-less app boots")
}

fn microsoft_entry() -> ConnectedAccount {
    let config = mailcal_account::MicrosoftConfig {
        email: "Alice@Example.com".to_owned(),
        client_id: "client-abc".to_owned(),
        tenant: "organizations".to_owned(),
        redirect_uri: "eu.allodia.mailcal://auth".to_owned(),
        scopes: Vec::new(),
        refresh_token: mailcal_account::Secret::new("refresh".to_owned()),
        granted_scopes: None,
        shape: mailcal_account::AccountShape::read("capabilities = [\"mail\"]").unwrap(),
    };
    let id = config.account_id().unwrap();
    let tokens = mailcal_account::GraphTokenSource::new(
        &config,
        id,
        None,
        mailcal_account::CredentialOrigin::FreshSignIn,
    )
    .unwrap();
    ConnectedAccount::Microsoft { config, tokens }
}

#[test]
fn the_same_address_in_another_case_is_the_same_account() {
    assert!(same_address("Alice@Example.com", " alice@example.com").is_ok());
    assert!(matches!(
        same_address("alice@example.com", "bob@example.com"),
        Err(MailcalError::Config(_))
    ));
}

#[test]
fn a_microsoft_account_signs_in_again_at_its_own_tenant_for_what_it_is_used_for() {
    let account = OAuthAccount::of(&microsoft_entry()).expect("a Microsoft account");
    assert_eq!(account.provider, Provider::Microsoft);
    assert_eq!(account.tenant, "organizations");
    assert_eq!(account.email, "Alice@Example.com");
    assert_eq!(
        account.capabilities.iter().collect::<Vec<_>>(),
        vec![mailcal_account::Capability::Mail]
    );
}

#[test]
fn only_an_account_that_signs_in_at_microsoft_or_google_can_start_one() {
    let app = app("consent-unknown");
    assert!(matches!(
        app.begin_account_consent(
            "nobody@example.com@graph.microsoft.com".to_owned(),
            "eu.allodia.mailcal://auth".to_owned(),
            Vec::new(),
        ),
        Err(MailcalError::Config(_))
    ));
}

/// The account was removed while the browser was up: nothing is exchanged, and nothing is
/// registered in its place.
#[test]
fn completing_for_a_removed_account_is_refused_before_any_exchange() {
    let app = app("consent-removed");
    let pending = serde_json::to_string(&PendingConsent {
        account_id: "alice@example.com@graph.microsoft.com".to_owned(),
        provider: Provider::Microsoft,
        login: "{}".to_owned(),
    })
    .unwrap();
    let result =
        app.complete_account_consent(pending, "eu.allodia.mailcal://auth?code=x".to_owned());
    assert!(matches!(result, Err(MailcalError::Config(_))), "{result:?}");
    assert!(
        !app.registry
            .contains("alice@example.com@graph.microsoft.com")
    );
}
