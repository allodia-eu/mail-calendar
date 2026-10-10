use std::sync::{Arc, Mutex};

use engine_api::AccountId;
use mailcal_account::{
    GoogleConfig, GraphTokenSource, JmapAccountConfig, MicrosoftConfig, OAuthGrant, Secret,
    TokenSink,
};

use super::BindingTokenSink;
use crate::{
    ConnectedAccount, SharedRegistry, account_registry::AccountRegistry,
    credential_store::AccountCredentialStore,
};

/// A host store that records what it was asked to persist.
struct Recorder(Mutex<Vec<(String, String)>>);

impl AccountCredentialStore for Recorder {
    fn persist(
        &self,
        account_id: String,
        config_toml: String,
    ) -> Result<(), crate::CredentialStoreError> {
        self.0
            .lock()
            .expect("recorder mutex poisoned")
            .push((account_id, config_toml));
        Ok(())
    }

    fn delete(&self, _account_id: String) -> Result<(), crate::CredentialStoreError> {
        Ok(())
    }
}

fn oauth_jmap_account() -> (AccountId, SharedRegistry) {
    let config = JmapAccountConfig {
        email: "alice@example.com".to_owned(),
        base_url: "https://api.example.com".to_owned(),
        password: None,
        token: None,
        oauth: Some(OAuthGrant {
            client_id: "client-abc".to_owned(),
            client_secret: None,
            refresh_token: Secret::new("original-refresh".to_owned()),
            authorize_endpoint: "https://api.example.com/oauth/authorize".to_owned(),
            token_endpoint: "https://api.example.com/oauth/refresh".to_owned(),
            redirect_uri: "eu.allodia.mailcal://jmap-oauth".to_owned(),
            scopes: vec!["offline_access".to_owned()],
            resource: None,
            issuer: None,
        }),
        shape: mailcal_account::AccountShape::default(),
    };
    let id = config.account_id().expect("a valid account id");
    let registry = AccountRegistry::new();
    registry.pre_register(
        id.as_str().to_owned(),
        ConnectedAccount::Jmap {
            config,
            tokens: None,
            session: crate::jmap_session::JmapSession::default(),
        },
    );
    (id, registry)
}

fn sink(registry: &SharedRegistry, store: Arc<dyn AccountCredentialStore>) -> BindingTokenSink {
    BindingTokenSink {
        registry: Arc::clone(registry),
        store,
    }
}

/// The path a rotated JMAP refresh token has to survive to outlive the process. It had no
/// test at all, which is how a host that never registered its store went unnoticed until a
/// ratcheting server revoked a real account's grant.
#[tokio::test]
async fn a_rotated_jmap_token_reaches_the_host_store_as_the_new_config() {
    let (id, registry) = oauth_jmap_account();
    let recorder = Arc::new(Recorder(Mutex::new(Vec::new())));
    let sink = sink(
        &registry,
        Arc::clone(&recorder) as Arc<dyn AccountCredentialStore>,
    );

    sink.refresh_token_rotated(&id, "rotated-refresh").await;

    let written = recorder.0.lock().expect("recorder mutex poisoned");
    let (account_id, toml) = written.first().expect("the store was asked to persist");
    assert_eq!(account_id, id.as_str());
    let parsed = mailcal_account::load_jmap_str(toml).expect("valid config TOML");
    assert_eq!(
        parsed.oauth.expect("an oauth grant").refresh_token.expose(),
        "rotated-refresh",
        "the persisted config must carry the NEW token, not the one it replaced",
    );
}

/// Both halves advance together: the in-memory registry, so the *rest of this session* keeps
/// refreshing from the current token, and the host store, so the *next launch* does. The
/// pair used to come apart: the registry always advanced, the store only if the host had
/// got round to registering one, which is precisely why the loss was invisible until the
/// following launch.
#[tokio::test]
async fn the_registry_and_the_store_advance_together() {
    let (id, registry) = oauth_jmap_account();
    let recorder = Arc::new(Recorder(Mutex::new(Vec::new())));
    let sink = sink(
        &registry,
        Arc::clone(&recorder) as Arc<dyn AccountCredentialStore>,
    );

    sink.refresh_token_rotated(&id, "rotated-refresh").await;

    assert_eq!(
        recorder.0.lock().expect("recorder mutex poisoned").len(),
        1,
        "the host store is written on the same rotation, not a later one",
    );
    let config = registry
        .jmap_config(id.as_str())
        .expect("the account is still registered as JMAP");
    assert_eq!(
        config
            .oauth
            .as_ref()
            .expect("an oauth grant")
            .refresh_token
            .expose(),
        "rotated-refresh",
    );
}

/// All three OAuth families re-persist through the **one** host store. They had a port each,
/// with identical signatures and identical implementations in every client, and a host that
/// wired two of the three looked wired. There is one port now, and this is the check that it
/// carries every family rather than only the one that was tested.
/// A rotation rewrites the whole stored document, so everything the account's config says
/// about the account beyond its token (its pinned id, what it is used for, its links, the
/// scopes it was granted) must come through the rewrite untouched.
#[tokio::test]
async fn a_rotation_keeps_everything_the_config_says_about_the_account() {
    let shape = mailcal_account::AccountShape::read(concat!(
        "id = \"pinned@graph.microsoft.com\"\n",
        "capabilities = [\"mail\"]\n",
        "[links]\n",
        "calendar = \"alice@dav:cloud.example\"\n",
    ))
    .expect("a valid shape");
    let microsoft = MicrosoftConfig {
        email: "alice@example.com".to_owned(),
        client_id: "client-abc".to_owned(),
        tenant: "common".to_owned(),
        redirect_uri: "eu.allodia.mailcal://auth".to_owned(),
        scopes: vec!["offline_access".to_owned()],
        refresh_token: Secret::new("original-refresh".to_owned()),
        granted_scopes: Some(vec!["offline_access".to_owned()]),
        affiliation: None,
        shape,
    };
    let id = microsoft.account_id().expect("a valid account id");
    let tokens = GraphTokenSource::new(
        &microsoft,
        id.clone(),
        None,
        mailcal_account::CredentialOrigin::FreshSignIn,
    )
    .expect("a token source over a well-formed config");
    let registry = AccountRegistry::new();
    registry.pre_register(
        id.as_str().to_owned(),
        ConnectedAccount::Microsoft {
            config: microsoft,
            tokens,
        },
    );
    let recorder = Arc::new(Recorder(Mutex::new(Vec::new())));
    let sink = sink(
        &registry,
        Arc::clone(&recorder) as Arc<dyn AccountCredentialStore>,
    );

    sink.refresh_token_rotated(&id, "rotated-graph").await;

    let written = recorder.0.lock().expect("recorder mutex poisoned");
    let (stored_id, toml) = &written[0];
    assert_eq!(stored_id, "pinned@graph.microsoft.com");
    let reread = mailcal_account::load_microsoft_str(toml).expect("a valid config");
    assert_eq!(reread.refresh_token.expose(), "rotated-graph");
    assert_eq!(
        reread.account_id().unwrap().as_str(),
        "pinned@graph.microsoft.com"
    );
    assert_eq!(
        reread.shape.links.calendar.as_ref().map(AccountId::as_str),
        Some("alice@dav:cloud.example")
    );
    assert_eq!(
        reread.granted_scopes,
        Some(vec!["offline_access".to_owned()])
    );
}

#[tokio::test]
async fn every_provider_family_re_persists_through_the_one_host_store() {
    let microsoft = MicrosoftConfig {
        email: "alice@example.com".to_owned(),
        client_id: "client-abc".to_owned(),
        tenant: "common".to_owned(),
        redirect_uri: "eu.allodia.mailcal://auth".to_owned(),
        scopes: vec!["offline_access".to_owned()],
        refresh_token: Secret::new("original-refresh".to_owned()),
        granted_scopes: None,
        affiliation: None,
        shape: mailcal_account::AccountShape::default(),
    };
    let google = GoogleConfig {
        email: "alice@example.com".to_owned(),
        client_id: "client-abc".to_owned(),
        client_secret: None,
        redirect_uri: "eu.allodia.mailcal://auth".to_owned(),
        scopes: vec!["offline_access".to_owned()],
        refresh_token: Secret::new("original-refresh".to_owned()),
        granted_scopes: None,
        affiliation: None,
        shape: mailcal_account::AccountShape::default(),
    };
    let microsoft_id = microsoft.account_id().expect("a valid account id");
    let google_id = google.account_id().expect("a valid account id");
    let tokens = GraphTokenSource::new(
        &microsoft,
        microsoft_id.clone(),
        None,
        mailcal_account::CredentialOrigin::FreshSignIn,
    )
    .expect("a token source over a well-formed config");
    let registry = AccountRegistry::new();
    registry.pre_register(
        microsoft_id.as_str().to_owned(),
        ConnectedAccount::Microsoft {
            config: microsoft,
            tokens: Arc::clone(&tokens),
        },
    );
    registry.pre_register(
        google_id.as_str().to_owned(),
        ConnectedAccount::Google {
            config: google,
            tokens,
        },
    );
    let recorder = Arc::new(Recorder(Mutex::new(Vec::new())));
    let sink = sink(
        &registry,
        Arc::clone(&recorder) as Arc<dyn AccountCredentialStore>,
    );

    sink.refresh_token_rotated(&microsoft_id, "rotated-graph")
        .await;
    sink.refresh_token_rotated(&google_id, "rotated-google")
        .await;

    let written = recorder.0.lock().expect("recorder mutex poisoned");
    let ids: Vec<&str> = written.iter().map(|(id, _)| id.as_str()).collect();
    assert_eq!(
        ids,
        vec![microsoft_id.as_str(), google_id.as_str()],
        "both families reached the same store, in the order they rotated",
    );
    assert!(
        written[0].1.contains("rotated-graph"),
        "the Microsoft config carries its new token",
    );
    assert!(
        written[1].1.contains("rotated-google"),
        "the Google config carries its new token",
    );
}

/// A JMAP account authenticated with a pasted secret has no grant to rotate; the sink must
/// leave it alone rather than writing a config with an `oauth` section it never had.
#[tokio::test]
async fn a_stored_secret_jmap_account_is_left_untouched() {
    let config = JmapAccountConfig {
        email: "alice@example.com".to_owned(),
        base_url: "https://api.example.com".to_owned(),
        password: Some(Secret::new("app-password".to_owned())),
        token: None,
        oauth: None,
        shape: mailcal_account::AccountShape::default(),
    };
    let id = config.account_id().expect("a valid account id");
    let registry = AccountRegistry::new();
    registry.pre_register(
        id.as_str().to_owned(),
        ConnectedAccount::Jmap {
            config,
            tokens: None,
            session: crate::jmap_session::JmapSession::default(),
        },
    );
    let recorder = Arc::new(Recorder(Mutex::new(Vec::new())));
    let sink = sink(
        &registry,
        Arc::clone(&recorder) as Arc<dyn AccountCredentialStore>,
    );

    sink.refresh_token_rotated(&id, "rotated-refresh").await;

    assert!(
        recorder
            .0
            .lock()
            .expect("recorder mutex poisoned")
            .is_empty(),
        "nothing to rotate, so nothing to persist",
    );
}
