//! Whether a Microsoft account is a personal one: stored once Graph has said, and never written
//! again for the same answer.

use std::sync::{Arc, Mutex};

use engine_api::{AccountId, Affiliation, OrganizationId};
use mailcal_account::{MicrosoftConfig, Secret, TokenSink};

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

/// A registered Microsoft account stored with `stored`, and a sink over it writing to the store.
fn microsoft_account(stored: Option<Affiliation>) -> (AccountId, BindingTokenSink, Arc<Store>) {
    let config = MicrosoftConfig {
        email: "alice@example.com".to_owned(),
        client_id: "client-abc".to_owned(),
        tenant: "common".to_owned(),
        redirect_uri: "eu.allodia.mailcal://auth".to_owned(),
        scopes: vec!["offline_access".to_owned()],
        refresh_token: Secret::new("refresh".to_owned()),
        granted_scopes: None,
        affiliation: stored,
        shape: mailcal_account::AccountShape::default(),
    };
    let id = config.account_id().expect("a valid account id");
    let tokens = mailcal_account::GraphTokenSource::new(
        &config,
        id.clone(),
        None,
        mailcal_account::CredentialOrigin::Stored,
    )
    .expect("a token source over a well-formed config");
    let registry = AccountRegistry::new();
    registry.pre_register(
        id.as_str().to_owned(),
        ConnectedAccount::Microsoft { config, tokens },
    );
    let store = Arc::new(Store::default());
    let sink = BindingTokenSink {
        registry,
        store: Arc::clone(&store) as Arc<dyn AccountCredentialStore>,
    };
    (id, sink, store)
}

fn stored(store: &Store) -> Vec<Option<Affiliation>> {
    store
        .0
        .lock()
        .expect("store mutex poisoned")
        .iter()
        .map(|toml| {
            mailcal_account::load_microsoft_str(toml)
                .expect("a valid config")
                .affiliation
        })
        .collect()
}

/// An account stored before its affiliation was recorded keeps the answer its first connect got.
#[tokio::test]
async fn an_account_stored_without_an_affiliation_records_the_first_answer() {
    let (id, sink, store) = microsoft_account(None);

    sink.affiliation_found(&id, &Affiliation::Personal).await;

    assert_eq!(stored(&store), [Some(Affiliation::Personal)]);
}

/// The same answer again writes nothing: a store write is a keychain prompt on some hosts.
#[tokio::test]
async fn the_same_answer_is_not_written_twice() {
    let (id, sink, store) = microsoft_account(Some(Affiliation::Personal));

    sink.affiliation_found(&id, &Affiliation::Personal).await;

    assert!(stored(&store).is_empty());
}

/// A different answer replaces the stored one, as when an address moves into an organisation.
#[tokio::test]
async fn a_different_answer_replaces_the_stored_one() {
    let (id, sink, store) = microsoft_account(Some(Affiliation::Personal));
    let organisation = Affiliation::Organization(
        OrganizationId::try_from("00000000-0000-4000-8000-00000000feed").unwrap(),
    );

    sink.affiliation_found(&id, &organisation).await;

    assert_eq!(stored(&store), [Some(organisation)]);
}
