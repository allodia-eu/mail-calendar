//! The host's OS-secure-store writer: the one port through which a credential the **core**
//! changed becomes a credential the **host** has stored.
//!
//! # Why this is a constructor argument and not a setter
//!
//! Every core that can refresh an access token can be handed a **rotated refresh token**, and a
//! rotation that reaches no store is not "kept in memory": it is a stored credential the server
//! has already moved past. Against an authorization server that detects a replayed refresh token
//! (OAuth 2.1 / RFC 9700 (Fastmail answers `invalid_grant) ratchet or client_id mismatch`)
//! presenting the superseded token does not merely fail, it **revokes the whole grant**.
//!
//! So this is a required parameter of both constructors, and there is no setter. That is not
//! style. The interactive constructor's *last statement* starts the background dial, and a real
//! launch on a production device measured the first OAuth refresh beginning **6 ms** later;
//! while the host was still blocked inside the
//! constructor and had run no code at all. Two of the four clients then installed their stores
//! from a **UI-thread post** (Android's `mainHandler.post`, Windows' `TryEnqueue`), so whether a
//! rotation 660 ms later was persisted or dropped was decided by whether the main thread got a
//! turn first. It usually did. "Usually" is not a property, and the failure is silent for an
//! hour and then permanent.
//!
//! Taking the store here makes the question unaskable: the slot cannot be empty, so no code path
//! needs to handle an empty one and no log line needs to report one.
//!
//! # Why one port and not one per provider
//!
//! There were three; `MicrosoftCredentialStore`, `GoogleCredentialStore`,
//! `JmapCredentialStore`; with byte-identical signatures, and all nine client implementations
//! were the same single line: hand the pair to the platform's secure store. Three ports is what
//! made forgetting the third one cheap, which is exactly what the headless background worker did
//! for as long as it existed. The account id already says which account this is, and the host
//! keys its store on that, nothing downstream of `persist` ever branched on the family.
//!
//! # Why both writes report a result
//!
//! Every method here can fail, because every platform store can: a Keychain can be locked, a
//! Credential Manager entry can exceed its size limit, an Android Keystore key can be
//! invalidated by a biometric enrolment. Until this port said so, it could not; `persist`
//! returned nothing, so the core logged `re-persisted a rotated refresh token to the host store`
//! whether or not a single byte had been written. A line that cannot report a failure is not a
//! report, and this one sat on the exact path whose failure mode is a revoked grant.
//!
//! What the core does with the answer differs by *when* it asked, and the difference is not
//! stylistic:
//!
//! - **Adding an account**: the whole add is rolled back and the error is returned. Nothing has
//!   been shown to the user yet, so a visible "could not save this account" beats an account that
//!   works until the next launch and then silently isn't there.
//! - **A rotation**, nothing can be rolled back. The old refresh token is already spent, and the
//!   new one is the only one the server will accept; there is no earlier state to return to. So the
//!   session carries on from the token in memory and the failure is logged at `error!`, which is
//!   the honest description of an account that will fail to authenticate at next launch.
//! - **Removing an account**: the account is gone from the runtime either way, so the error is
//!   returned for the host to surface rather than reverting a removal the user asked for. An
//!   undeleted entry is a *zombie account*: it comes back at the next launch with no explanation.

/// Why a write to the platform's secure store did not happen.
///
/// One variant carrying the host's own message: the core never branches on the reason (there is
/// nothing it could do differently for a locked Keychain versus a full one), it only needs to
/// know that the write did not land and to be able to say why in the log.
#[derive(Debug, uniffi::Error, thiserror::Error)]
pub enum CredentialStoreError {
    /// The platform store refused the write or the delete.
    #[error("credential store: {0}")]
    Store(String),
}

/// The host's OS-secure-store writer for account credentials, and the **only** path by which an
/// account's stored credential is created, replaced or erased.
///
/// The core calls this when it adds an account, when a provider **rotates** an account's refresh
/// token, and when an account is removed; all three through the same platform store (Keychain,
/// Credential Manager, Android Keystore). Keeping the write native is deliberate: encryption,
/// access control and chunking are the platform's job. Deciding *when* a credential must be
/// durable (and what happens when it cannot be) is the core's.
///
/// Implementations must be safe to call from any thread and should not block: the caller is an
/// account connect or a token refresh on the core's runtime, with providers waiting on it.
#[uniffi::export(callback_interface)]
pub trait AccountCredentialStore: Send + Sync {
    /// Persists `config_toml` for `account_id`, replacing the stored entry.
    ///
    /// # Errors
    ///
    /// Throws [`CredentialStoreError`] when the platform store refused the write. Report the
    /// failure rather than swallowing it: the core cannot see a store it did not write to, and
    /// what it does next depends on this answer.
    fn persist(&self, account_id: String, config_toml: String) -> Result<(), CredentialStoreError>;

    /// Erases the stored entry for `account_id`. Deleting an id that is not stored is a
    /// success, not an error: the desired end state is that nothing is stored for it.
    ///
    /// # Errors
    ///
    /// Throws [`CredentialStoreError`] when the platform store refused the delete.
    fn delete(&self, account_id: String) -> Result<(), CredentialStoreError>;
}

/// The store for a core that has no stored credentials to keep: the in-memory demo and showcase
/// apps, whose accounts are bundled fixtures with no OAuth grant behind them.
///
/// Both methods fail rather than doing nothing, because "there is nothing to persist" is a claim
/// about those two builders and not about this type; if a credential ever reaches here, the
/// claim has stopped being true, and reporting that beats a silent drop.
pub(crate) struct NoStoredCredentials;

impl NoStoredCredentials {
    /// The one answer both methods give, so the reason is written once.
    fn refuse(operation: &str, account_id: &str) -> Result<(), CredentialStoreError> {
        log::error!(
            "credentials: [{}] asked to {operation} this account's stored credential in the \
             demo/showcase core, which has no store: this build's accounts are bundled fixtures \
             and should have no credential to keep",
            mailcal_account::account_log_handle(account_id),
        );
        Err(CredentialStoreError::Store(
            "this build has no credential store: its accounts are bundled fixtures".to_owned(),
        ))
    }
}

impl AccountCredentialStore for NoStoredCredentials {
    fn persist(
        &self,
        account_id: String,
        _config_toml: String,
    ) -> Result<(), CredentialStoreError> {
        Self::refuse("write", &account_id)
    }

    fn delete(&self, account_id: String) -> Result<(), CredentialStoreError> {
        Self::refuse("erase", &account_id)
    }
}
