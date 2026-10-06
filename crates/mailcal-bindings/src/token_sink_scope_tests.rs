//! The granted scopes a refresh names: recorded over the stored set when they moved, and never
//! written when they did not.

use std::sync::{Arc, Mutex};

use engine_api::AccountId;
use mailcal_account::{GoogleConfig, Secret, TokenSink};
use mailcal_oauth::GrantedScopes;

use super::BindingTokenSink;
use crate::{
    ConnectedAccount, account_registry::AccountRegistry, credential_store::AccountCredentialStore,
};

/// A host store that records what it was asked to persist.
#[derive(Default)]
struct Store(Mutex<Vec<String>>);

impl AccountCredentialStore for Store {
    fn persist(
        &self,
        _account_id: String,
        config_toml: String,
    ) -> Result<(), crate::CredentialStoreError> {
        self.0
            .lock()
            .expect("store mutex poisoned")
            .push(config_toml);
        Ok(())
    }

    fn delete(&self, _account_id: String) -> Result<(), crate::CredentialStoreError> {
        Ok(())
    }
}

fn scopes(names: &[&str]) -> Vec<String> {
    names.iter().map(|&name| name.to_owned()).collect()
}

/// A registered Google account whose stored config records `stored` as its granted scopes, and a
/// sink over it that writes to the returned store.
fn google_account(stored: Option<&[&str]>) -> (AccountId, BindingTokenSink, Arc<Store>) {
    let config = GoogleConfig {
        email: "alice@example.com".to_owned(),
        client_id: "client-abc".to_owned(),
        client_secret: None,
        redirect_uri: "eu.allodia.mailcal://auth".to_owned(),
        scopes: scopes(&[
            "https://mail.google.com/",
            "https://www.googleapis.com/auth/calendar",
        ]),
        refresh_token: Secret::new("original-refresh".to_owned()),
        granted_scopes: stored.map(scopes),
        affiliation: None,
        shape: mailcal_account::AccountShape::default(),
    };
    let id = config.account_id().expect("a valid account id");
    let tokens = mailcal_account::google_token_source(
        &config,
        id.clone(),
        None,
        mailcal_account::CredentialOrigin::Stored,
    )
    .expect("a token source over a well-formed config");
    let registry = AccountRegistry::new();
    registry.pre_register(
        id.as_str().to_owned(),
        ConnectedAccount::Google { config, tokens },
    );
    let store = Arc::new(Store::default());
    let sink = BindingTokenSink {
        registry,
        store: Arc::clone(&store) as Arc<dyn AccountCredentialStore>,
    };
    (id, sink, store)
}

fn granted(names: &[&str]) -> GrantedScopes {
    GrantedScopes::from_stored(scopes(names))
}

fn stored_scopes(store: &Store) -> Vec<Option<Vec<String>>> {
    store
        .0
        .lock()
        .expect("store mutex poisoned")
        .iter()
        .map(|toml| {
            mailcal_account::load_google_str(toml)
                .expect("a valid config")
                .granted_scopes
        })
        .collect()
}

/// Consent withdrawn at the provider: the next refresh names less, and the stored set follows it
/// down rather than going on claiming the calendar.
#[tokio::test]
async fn a_narrower_grant_replaces_the_stored_one() {
    let (id, sink, store) = google_account(Some(&[
        "https://mail.google.com/",
        "https://www.googleapis.com/auth/calendar",
    ]));

    sink.scopes_granted(&id, &granted(&["https://mail.google.com/"]))
        .await;

    assert_eq!(
        stored_scopes(&store),
        [Some(scopes(&["https://mail.google.com/"]))]
    );
}

/// An account stored before granted scopes were recorded learns them at its first refresh.
#[tokio::test]
async fn an_account_stored_without_granted_scopes_records_the_first_it_is_told() {
    let (id, sink, store) = google_account(None);

    sink.scopes_granted(&id, &granted(&["https://mail.google.com/"]))
        .await;

    assert_eq!(
        stored_scopes(&store),
        [Some(scopes(&["https://mail.google.com/"]))]
    );
}

/// Almost every refresh names what is already stored, and a write per refresh is a keychain
/// prompt on some hosts. The order a server lists them in is not a change.
#[tokio::test]
async fn an_unchanged_grant_is_not_written_in_any_order() {
    let (id, sink, store) = google_account(Some(&["a", "b"]));

    sink.scopes_granted(&id, &granted(&["b", "a"])).await;

    assert!(stored_scopes(&store).is_empty());
}

/// A refresh token rotated later in the session writes the newest granted set with it, because
/// both live in the one registry entry.
#[tokio::test]
async fn a_later_rotation_keeps_the_recorded_grant() {
    let (id, sink, store) = google_account(Some(&["a", "b"]));

    sink.scopes_granted(&id, &granted(&["a"])).await;
    sink.refresh_token_rotated(&id, "rotated").await;

    assert_eq!(
        stored_scopes(&store),
        [Some(scopes(&["a"])), Some(scopes(&["a"]))]
    );
}
