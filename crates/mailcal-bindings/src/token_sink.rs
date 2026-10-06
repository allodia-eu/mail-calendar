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
use mailcal_oauth::GrantedScopes;

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

    /// Stores what the provider now grants, when it moved. Losing this write costs less than
    /// losing a rotation: the next refresh names the scopes again, and nothing is spent.
    async fn scopes_granted(&self, account: &AccountId, granted: &GrantedScopes) {
        let Some((family, encoded)) = self.registry.record_granted_scopes(account, granted) else {
            return;
        };
        let handle = mailcal_account::account_log_handle(account.as_str());
        let stored = encoded.and_then(|config_toml| {
            self.store
                .persist(account.as_str().to_owned(), config_toml)
                .map_err(|err| err.to_string())
        });
        match stored {
            Ok(()) => log::info!(
                "oauth: [{handle}] the {family} server now grants this account {} permission(s); \
                 saved to this device's secure store",
                granted.as_slice().len(),
            ),
            Err(err) => log::warn!(
                "oauth: [{handle}] the {family} server now grants this account {} permission(s), \
                 but they could not be saved ({err}); the next renewal tries again",
                granted.as_slice().len(),
            ),
        }
    }

    /// Stores whether the account is a personal one. Losing this write costs one more question
    /// at the next connect.
    async fn affiliation_found(&self, account: &AccountId, affiliation: &engine_api::Affiliation) {
        let Some(encoded) = self.registry.record_affiliation(account, affiliation) else {
            return;
        };
        let handle = mailcal_account::account_log_handle(account.as_str());
        let stored = encoded.and_then(|config_toml| {
            self.store
                .persist(account.as_str().to_owned(), config_toml)
                .map_err(|err| err.to_string())
        });
        match stored {
            Ok(()) => log::info!("graph: [{handle}] saved whether the account is a personal one"),
            Err(err) => log::warn!(
                "graph: [{handle}] could not save whether the account is a personal one ({err}); \
                 asking again at the next connect"
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
#[path = "token_sink_scope_tests.rs"]
mod scope_tests;

#[cfg(test)]
#[path = "token_sink_affiliation_tests.rs"]
mod affiliation_tests;

#[cfg(test)]
#[path = "token_sink_tests.rs"]
mod token_sink_tests;
