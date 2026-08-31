//! One credential state per account **per process**, however many cores exist.
//!
//! # The hazard this closes
//!
//! [`super::GraphTokenSource`] serializes refreshes so that many folder providers share one, and
//! that works because they share one *source*. It stops working the moment there are two sources
//! for the same account, because two sources are two single-flights.
//!
//! Two sources happen. A host can construct the core more than once in one process: on Android a
//! one-time `MailSyncWorker` and the periodic one can overlap, and `MailcalApplication.liveCore` is
//! a `WeakReference`, so a background worker may fail to find a warm core and build a cold one
//! beside it. Each core reads the same refresh token out of the host's store and builds its own
//! source.
//!
//! Measured on a device, two cold cores 6 ms apart:
//!
//! ```text
//! 10:58:26.473  oauth: jmap [acct:05f4]: refreshed in 307ms; ... the server ROTATED the refresh token
//! 10:58:26.641  oauth: jmap [acct:05f4]: refreshed in 302ms; ... the server ROTATED the refresh token
//! ```
//!
//! Two rotations of one grant, 168 ms apart, from two independent refreshers. Both presented the
//! same stored token, so the second was a **replay** of one the first had already superseded;
//! which on a ratcheting authorization server revokes the grant outright.
//!
//! # Why a lock is not the fix
//!
//! The obvious answer is a process-wide mutex per account, so the second refresh waits. It does not
//! work: when the second source acquires the lock, its *own* `TokenState` still holds the token it
//! read at boot. The first refresh advanced a different core's state and the host's store, not this
//! one's. So the second source dutifully waits its turn and then presents the spent token anyway.
//!
//! What has to be shared is not the lock but the **state**: the current refresh token, the cached
//! access token, the remembered failure, and the single-flight that guards them. Share those and
//! everything already built works across cores unchanged: the second source finds a valid cached
//! access token and never refreshes at all.
//!
//! # Adopt or replace
//!
//! Which leaves one decision the caller must make, and [`CredentialOrigin`] makes it say so. A
//! source built from the host's **store** adopts whatever this process already has, because the
//! live state is by definition at least as fresh as anything on disk. A source built from a **fresh
//! sign-in** must not: the credential the old state describes has just been replaced, and adopting
//! it would leave a newly repaired account refreshing with the dead grant it was repaired from.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock, Weak},
};

use engine_core::ids::AccountId;
use time::OffsetDateTime;

use super::failure::RefreshFailure;

/// Where the refresh token a source is being built over came from, and therefore whether the
/// source may adopt state this process already holds for that account.
///
/// An enum rather than a "remember to call `forget` first", because forgetting it is silent: the
/// account keeps working for the rest of the session on a cached access token and is dead at the
/// next launch, which is the exact failure shape this whole area keeps producing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialOrigin {
    /// Read from the host's secure store at boot. **Adopts** any live state for this account: it is
    /// the same credential, possibly already advanced past what was stored.
    Stored,
    /// Just obtained from a sign-in: a code exchange, a re-authentication, an account the host has
    /// only now handed us. **Replaces** any live state, because the credential it described is
    /// gone.
    FreshSignIn,
}

/// The mutable half of one account's credential, shared by every token source for that account in
/// this process.
pub(super) struct SharedCredential {
    pub(super) state: Mutex<TokenState>,
    /// Serializes the **refresh** so only one is ever in flight for this account; across cores,
    /// not merely across the folder providers of one core. Async, because it is deliberately
    /// held across the network round trip, which is the whole point (`state` never is).
    pub(super) refreshing: tokio::sync::Mutex<()>,
}

/// The mutable token state, guarded so many concurrent folder syncs (and now many cores) share
/// one refresh.
pub(super) struct TokenState {
    pub(super) access_token: String,
    pub(super) refresh_token: String,
    pub(super) expires_at: OffsetDateTime,
    /// The most recent **failed** refresh, so callers queued behind it take its outcome instead of
    /// posting their own request with the same refresh token. Cleared by the next success.
    pub(super) last_failure: Option<RefreshFailure>,
    /// How many refresh requests have been posted for this credential. Only ever read to make a
    /// log line tell the truth about *why* there is no cached token, and now genuinely
    /// process-wide, which is what that line always claimed.
    pub(super) attempts: u32,
}

/// Every live credential state in this process, keyed by account id.
///
/// `Weak`, so a state lives exactly as long as some token source is still using it: an account the
/// user removes takes its tokens out of memory with it, rather than leaving them in a map for the
/// life of the process.
static CREDENTIALS: OnceLock<Mutex<HashMap<String, Weak<SharedCredential>>>> = OnceLock::new();

fn credentials() -> &'static Mutex<HashMap<String, Weak<SharedCredential>>> {
    CREDENTIALS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// The shared state for `account`, adopting or replacing per `origin`.
///
/// `refresh_token` seeds a state that does not exist yet; it is **ignored** when an existing state
/// is adopted, which is the point, that state's token has already moved past it.
pub(super) fn credential_for(
    account: &AccountId,
    refresh_token: String,
    origin: CredentialOrigin,
) -> Arc<SharedCredential> {
    let key = account.as_str().to_owned();
    let mut live = credentials().lock().expect("credential registry poisoned");
    // Drop states nothing holds any more, so the map does not grow with every account ever added.
    live.retain(|_, weak| weak.strong_count() > 0);
    if origin == CredentialOrigin::Stored
        && let Some(existing) = live.get(&key).and_then(Weak::upgrade)
    {
        return existing;
    }
    let fresh = Arc::new(SharedCredential {
        state: Mutex::new(TokenState {
            access_token: String::new(),
            refresh_token,
            // Already "expired", so the first use always refreshes.
            expires_at: OffsetDateTime::UNIX_EPOCH,
            last_failure: None,
            attempts: 0,
        }),
        refreshing: tokio::sync::Mutex::new(()),
    });
    live.insert(key, Arc::downgrade(&fresh));
    fresh
}

#[cfg(test)]
mod tests {
    use super::*;

    fn account(id: &str) -> AccountId {
        AccountId::try_from(id).expect("a valid account id")
    }

    /// Two sources for the same account share one state, so there is one refresher per credential
    /// however many cores a host builds.
    #[test]
    fn a_second_source_for_the_same_account_adopts_the_live_state() {
        let id = account("shared@example.com@api.example.com");
        let first = credential_for(&id, "original".to_owned(), CredentialOrigin::Stored);
        first.state.lock().expect("state poisoned").refresh_token = "rotated-once".to_owned();

        // A second core boots the same account and reads the *stored* (now superseded) token.
        let second = credential_for(&id, "original".to_owned(), CredentialOrigin::Stored);

        assert!(
            Arc::ptr_eq(&first, &second),
            "the second core built its own state, so it will present the token the first replaced",
        );
        assert_eq!(
            second.state.lock().expect("state poisoned").refresh_token,
            "rotated-once",
            "the adopted state must keep the token it rotated to, not the one from the store",
        );
    }

    /// A fresh sign-in replaces the state, so a repaired account does not go on refreshing with the
    /// dead grant it was repaired from.
    #[test]
    fn a_fresh_sign_in_replaces_the_live_state() {
        let id = account("reauth@example.com@api.example.com");
        let dead = credential_for(&id, "revoked".to_owned(), CredentialOrigin::Stored);

        let repaired = credential_for(&id, "brand-new".to_owned(), CredentialOrigin::FreshSignIn);

        assert!(!Arc::ptr_eq(&dead, &repaired));
        assert_eq!(
            repaired.state.lock().expect("state poisoned").refresh_token,
            "brand-new",
        );
        // And a *later* stored source adopts the repaired one, not the dead one.
        let next = credential_for(&id, "revoked".to_owned(), CredentialOrigin::Stored);
        assert!(Arc::ptr_eq(&repaired, &next));
    }

    /// Different accounts never share, which is the other half of keying by id.
    #[test]
    fn two_accounts_keep_their_own_state() {
        let first = credential_for(
            &account("a@example.com@api.example.com"),
            "a".to_owned(),
            CredentialOrigin::Stored,
        );
        let second = credential_for(
            &account("b@example.com@api.example.com"),
            "b".to_owned(),
            CredentialOrigin::Stored,
        );

        assert!(!Arc::ptr_eq(&first, &second));
    }

    /// A state nothing holds is forgotten, so removing an account takes its tokens out of memory.
    #[test]
    fn a_state_with_no_remaining_source_is_dropped() {
        let id = account("gone@example.com@api.example.com");
        drop(credential_for(
            &id,
            "original".to_owned(),
            CredentialOrigin::Stored,
        ));

        let rebuilt = credential_for(&id, "from-the-store".to_owned(), CredentialOrigin::Stored);

        assert_eq!(
            rebuilt.state.lock().expect("state poisoned").refresh_token,
            "from-the-store",
            "a dead entry was upgraded, which would resurrect a removed account's tokens",
        );
    }
}
