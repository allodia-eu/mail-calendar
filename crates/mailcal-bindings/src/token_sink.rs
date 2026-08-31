//! The shared [`TokenSink`] every OAuth account's token refresh reports a **rotation** to.
//!
//! One instance serves all three families (Microsoft, Google, and an OAuth JMAP account) keyed by
//! the account id it is handed, so it lives here rather than inside any one provider's module. It
//! sat in `microsoft.rs` until a JMAP rotation started logging itself as
//! `[mailcal_bindings::microsoft]`, which is exactly the sort of thing a support log should not
//! have to be read past.
//!
//! What it does is small and load-bearing: ask the registry to advance the account's stored refresh
//! token, then hand the re-serialized config to the host's OS-secure-store writer. A rotation that
//! does not reach that writer leaves the stored credential **behind the server's**, which a
//! replay-detecting authorization server answers by revoking the grant outright: so every path out
//! of here that fails to persist says so in the log.
//!
//! The *mutation* is the registry's ([`AccountRegistry::rotate_refresh_token`]), and the *wording*
//! is this module's. That split is deliberate: the registry is the only holder of the durable half
//! of a credential, and these lines are the only ones a user ever reads about it.
//!
//! # Every line here lands on a user's device, so it says what happened: not how we are built
//!
//! The diagnostic log is a file the user can open and attach to a support request
//! ([`logging.md`](../../../docs/logging.md)), which makes it product surface, not developer
//! scratch. So a line names the account (by its non-identifying handle), what happened to their
//! sign-in, and what it means for them. It does **not** name our registry, our modules, our
//! serialization step, an issue number, or a rule in a design doc.
//!
//! That is not cosmetic, and it is not only about polish. An internal reference is a promise the
//! log cannot keep: it means nothing to the person reading it, it goes stale the moment the code
//! moves, and it invites the reader to conclude the app is talking about itself rather than about
//! their mail. The *reasoning* belongs in the comments right here, next to the code it explains,
//! where it stays true because a compiler and a reviewer are looking at it.

use std::sync::Arc;

use async_trait::async_trait;
use engine_api::AccountId;
use mailcal_account::TokenSink;

use crate::{SharedRegistry, account_registry::Rotation, credential_store::AccountCredentialStore};

/// The bindings' [`TokenSink`]: on a refresh-token rotation it advances the registry entry **and**
/// re-persists the account's config through the host's OS-secure-store writer, so the stored token
/// stays current across launches. One instance serves every OAuth account; **Microsoft, Google, or
/// an OAuth JMAP account**; keyed by the `account` arg; all three re-persist through the one host
/// store. A stored-secret account (IMAP, or a password/token JMAP one) has nothing to rotate and
/// no-ops.
pub(crate) struct BindingTokenSink {
    pub(crate) registry: SharedRegistry,
    pub(crate) store: Arc<dyn AccountCredentialStore>,
}

#[async_trait]
impl TokenSink for BindingTokenSink {
    async fn refresh_token_rotated(&self, account: &AccountId, new_refresh_token: &str) {
        let handle = mailcal_account::account_log_handle(account.as_str());
        let (family, config_toml) = match self
            .registry
            .rotate_refresh_token(account, new_refresh_token)
        {
            Rotation::Advanced {
                family,
                config_toml,
            } => (family, config_toml),
            // Registered, but with nothing to write. An account with no grant is silent; it is not
            // an error for a password account to be handed a rotation it cannot use. A config that
            // will not encode is the same consequence as a refused write, so it is an `error!`:
            // severity follows the outcome, never how obscure the cause is.
            Rotation::Nothing {
                encode_error,
                family,
            } => {
                if let Some(err) = encode_error {
                    log::error!(
                        "oauth: [{handle}] the {family} server renewed this account's sign-in, but \
                         the new one could not be prepared for storage ({err}). Mail keeps working \
                         until the app is restarted; after that this account will ask to be signed \
                         in again",
                    );
                }
                return;
            }
            // No entry at all: the rotation is lost. Every path that dials an account now has to go
            // through the registry to get a dial at all (see `AccountRegistry`), so this should be
            // unreachable, which is exactly why it shouts.
            //
            // It used to be a `warn!` claiming the credential "stays one generation behind until
            // the next rotation". Both halves were wrong, and the wording is what let
            // this sit in a production log for two days without anyone reading
            // consequence into it. On a ratcheting server there is no next rotation:
            // the stored token is the one the server has already moved past, so the
            // next launch presents a replay and the grant is revoked outright.
            Rotation::Unregistered => {
                log::error!(
                    "oauth: [{handle}] the server renewed this account's sign-in, but the app was \
                     not ready to save it, so the renewal was LOST. Mail keeps working until the \
                     app is restarted; after that this account will ask to be signed in again. This \
                     is a fault in the app, not a problem with the network or the mail server",
                );
                return;
            }
        };
        match self.store.persist(account.as_str().to_owned(), config_toml) {
            Ok(()) => {
                log::info!(
                    "oauth: [{handle}] the {family} server renewed this account's sign-in; the new \
                     one is saved to this device's secure store"
                );
            }
            // Nothing to roll back: the token this replaced is already spent, and the new one is
            // the only one the server will accept: so there is no earlier state to
            // return to and no honest way to fail the refresh. The session keeps
            // working from the token in memory; the *next* launch is the one that
            // breaks, which is why this says what will happen rather than what just
            // did.
            Err(err) => log::error!(
                "oauth: [{handle}] the {family} server renewed this account's sign-in, but this \
                 device's secure store refused to save it ({err}). Mail keeps working until the app \
                 is restarted; after that this account will ask to be signed in again",
            ),
        }
    }
}

/// Builds the shared [`TokenSink`] over the registry + the host's one credential store.
pub(crate) fn token_sink(
    registry: &SharedRegistry,
    store: &Arc<dyn AccountCredentialStore>,
) -> Arc<dyn TokenSink> {
    Arc::new(BindingTokenSink {
        registry: Arc::clone(registry),
        store: Arc::clone(store),
    })
}

#[cfg(test)]
mod token_sink_tests {
    use std::sync::{Arc, Mutex};

    use engine_api::AccountId;
    use mailcal_account::{
        GoogleConfig, GraphTokenSource, JmapAccountConfig, JmapOAuth, MicrosoftConfig, Secret,
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
            oauth: Some(JmapOAuth {
                client_id: "client-abc".to_owned(),
                client_secret: None,
                refresh_token: Secret::new("original-refresh".to_owned()),
                authorize_endpoint: "https://api.example.com/oauth/authorize".to_owned(),
                token_endpoint: "https://api.example.com/oauth/refresh".to_owned(),
                redirect_uri: "eu.allodia.mailcal://jmap-oauth".to_owned(),
                scopes: vec!["offline_access".to_owned()],
                resource: None,
            }),
        };
        let id = config.account_id().expect("a valid account id");
        let registry = AccountRegistry::new();
        registry.pre_register(
            id.as_str().to_owned(),
            ConnectedAccount::Jmap {
                config,
                tokens: None,
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
    #[tokio::test]
    async fn every_provider_family_re_persists_through_the_one_host_store() {
        let microsoft = MicrosoftConfig {
            email: "alice@example.com".to_owned(),
            client_id: "client-abc".to_owned(),
            tenant: "common".to_owned(),
            redirect_uri: "eu.allodia.mailcal://auth".to_owned(),
            scopes: vec!["offline_access".to_owned()],
            refresh_token: Secret::new("original-refresh".to_owned()),
        };
        let google = GoogleConfig {
            email: "alice@example.com".to_owned(),
            client_id: "client-abc".to_owned(),
            client_secret: None,
            redirect_uri: "eu.allodia.mailcal://auth".to_owned(),
            scopes: vec!["offline_access".to_owned()],
            refresh_token: Secret::new("original-refresh".to_owned()),
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
        };
        let id = config.account_id().expect("a valid account id");
        let registry = AccountRegistry::new();
        registry.pre_register(
            id.as_str().to_owned(),
            ConnectedAccount::Jmap {
                config,
                tokens: None,
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
}
