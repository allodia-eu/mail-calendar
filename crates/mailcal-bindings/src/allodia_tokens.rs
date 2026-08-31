//! The access token every call to the account service is made with.
//!
//! What a device keeps between launches is a refresh token, in the host's secure store beside the
//! mail accounts; what a request needs is an access token that lives about an hour. This is the
//! piece in between, and it holds one for the process rather than minting one per call: a sync
//! pass makes several requests, and paying a token round trip for each would triple the time
//! somebody is waiting.
//!
//! **A rotation is stored before the token it came with is used.** The service may hand back a
//! replacement refresh token, and the one it replaced is spent the moment the response is written.
//! A replacement that reaches no store is a grant the next launch cannot present, and against a
//! server that treats a replayed refresh token as theft, presenting the old one revokes the whole
//! grant rather than merely failing. `credential_store` carries the long version.

use std::sync::{Arc, Mutex};

use allodia_license::Refresher;
use mailcal_oauth::TokenSet;
use time::{Duration, OffsetDateTime};

use crate::{
    AllodiaGrantHealth, MailcalApp, MailcalError, allodia::ACCOUNT_ID, allodia_transport::block_on,
};

/// How long before expiry a token is treated as spent.
///
/// The same margin the mail providers use, and for the same reason: a token handed out with
/// seconds left dies mid-request, and the failure that follows names the request rather than
/// the token.
const REFRESH_SKEW: Duration = Duration::minutes(5);

/// What this process is holding, and what it needs to mint one.
#[derive(Debug, Default)]
pub(crate) struct Tokens {
    /// The discovered client, built once. Discovery is two requests whose answer does not
    /// change between them.
    refresher: tokio::sync::OnceCell<Arc<Refresher>>,
    /// The access token in hand, until it is near expiry.
    access: Mutex<Option<TokenSet>>,
}

impl Tokens {
    /// The token in hand, if it will still be live when it arrives.
    fn live(&self, now: OffsetDateTime) -> Option<String> {
        self.access
            .lock()
            .expect("allodia token lock")
            .as_ref()
            .filter(|tokens| !tokens.is_expired(now, REFRESH_SKEW))
            .map(|tokens| tokens.access_token.expose().to_owned())
    }

    /// Drop what is held, because the grant it was minted from is gone.
    fn forget(&self) {
        *self.access.lock().expect("allodia token lock") = None;
    }
}

impl MailcalApp {
    /// An access token for the account service, minting one if what is held has run out.
    ///
    /// **Blocking**: it can make up to three round trips (discovery, twice, and the refresh).
    /// Every caller is already a blocking FFI method a host runs off its main thread.
    ///
    /// # Errors
    ///
    /// [`MailcalError::Config`] when nobody is signed in: a caller that reached here without
    /// checking has a wiring bug, not a person to tell. [`MailcalError::Connect`] when the
    /// service could not be reached or refused the grant; the second is a sign-in that has to
    /// be made again, and is reported rather than acted on, because a service having a bad
    /// afternoon must not sign anybody out.
    pub(crate) fn allodia_access_token(&self) -> Result<String, MailcalError> {
        let now = OffsetDateTime::now_utc();
        if let Some(token) = self.allodia_tokens.live(now) {
            return Ok(token);
        }
        let refresh_token = {
            let signed_in = self.allodia.lock().expect("allodia account lock");
            let stored = signed_in.as_ref().ok_or_else(|| {
                MailcalError::Config("no Allodia account is signed in".to_owned())
            })?;
            stored.refresh_token.clone()
        };

        let refresher = self.allodia_refresher()?;
        log::info!("allodia: the access token has run out; refreshing the grant");
        let minted = block_on(
            self.runtime.handle(),
            refresher.refresh(&refresh_token, now),
        )
        .map_err(|error| {
            // Whether this says anything about the grant, and what. A refusal the service gave is
            // evidence; anything else is a bad afternoon and must change nothing.
            if let allodia_license::SignInError::OAuth(oauth) = &error
                && let Some(health) = AllodiaGrantHealth::from_refusal(oauth.refusal())
            {
                self.note_allodia_health(health);
            }
            log::warn!("allodia: the grant could not be refreshed; {error}");
            MailcalError::Connect(error.to_string())
        })?;

        // A refresh that worked is the strongest evidence there is that the sign-in is alive, and
        // the response may also name a scope set narrower than this build wants: the state a
        // person is in after a scope is added. Both are recorded here rather than guessed at the
        // point some feature fails.
        self.record_allodia_grant_scopes(&minted);
        if let Some(rotated) = &minted.refresh_token {
            self.store_rotated_allodia_grant(rotated.expose());
        }
        let token = minted.access_token.expose().to_owned();
        *self
            .allodia_tokens
            .access
            .lock()
            .expect("allodia token lock") = Some(minted);
        Ok(token)
    }

    /// The discovered client, built on first use and kept for the process.
    fn allodia_refresher(&self) -> Result<Arc<Refresher>, MailcalError> {
        let built = block_on(
            self.runtime.handle(),
            self.allodia_tokens
                .refresher
                .get_or_try_init(|| async { Refresher::discover().await.map(Arc::new) }),
        );
        built.cloned().map_err(|error| {
            log::warn!("allodia: the account service's metadata could not be read; {error}");
            MailcalError::Connect(error.to_string())
        })
    }

    /// Throw away the access token held for the process, because the grant it came from is gone.
    ///
    /// **Every path that replaces or erases the stored grant must call this**, and the reason is
    /// not tidiness. The token is cached until it is near expiry (about an hour) so a sign-in
    /// that stored a *new* grant and left the old token in place goes on presenting it, and the
    /// service refuses it: the new authorisation superseded the grant it was minted from. What that
    /// looks like is a person signing in successfully and being told, a fraction of a second later,
    /// that they are signed out.
    ///
    /// It stayed invisible while the only way to sign in was from a signed-out state, where there
    /// is no cached token to be stale. Offering "sign in again" to somebody already signed in is
    /// what reaches it.
    pub(crate) fn forget_allodia_access_token(&self) {
        self.allodia_tokens.forget();
    }

    /// Record what this device now knows about its sign-in.
    ///
    /// Idempotent and quiet when nothing changed: the log line is what a support session reads to
    /// see when a person's grant stopped being usable, and repeating it every refresh would bury
    /// the moment it happened.
    pub(crate) fn note_allodia_health(&self, health: AllodiaGrantHealth) {
        let mut held = self.allodia_health.lock().expect("allodia health lock");
        if *held == health {
            return;
        }
        // Never who: a log line describes the user's mail and names no address
        // (`docs/logging.md`).
        match health {
            AllodiaGrantHealth::Ok => log::info!("allodia: the sign-in is usable again"),
            AllodiaGrantHealth::NeedsReauth => log::warn!(
                "allodia: the stored sign-in predates a permission this build needs; the account \
                 list stays put until the person signs in again"
            ),
            AllodiaGrantHealth::SignedOut => log::warn!(
                "allodia: the account service REFUSED the stored sign-in; it was revoked here or \
                 the account was removed elsewhere"
            ),
        }
        *held = health;
    }

    /// What this device knows about its Allodia sign-in.
    pub(crate) fn allodia_health(&self) -> AllodiaGrantHealth {
        *self.allodia_health.lock().expect("allodia health lock")
    }

    /// Store the scope set a token response named, and read the health off it.
    ///
    /// The response names one only when it differs from the request (RFC 6749 §5.1), so the
    /// refresher's own requested set is the fallback; `GrantedScopes::from_response` holds that
    /// rule. Persisting it is what makes the next launch's check local instead of a round trip
    /// that fails.
    fn record_allodia_grant_scopes(&self, minted: &TokenSet) {
        let Some(refresher) = self.allodia_tokens.refresher.get() else {
            return;
        };
        let granted = mailcal_oauth::GrantedScopes::from_response(
            &minted.scope,
            refresher.requested_scopes(),
        );
        let health = {
            let mut signed_in = self.allodia.lock().expect("allodia account lock");
            let Some(stored) = signed_in.as_mut() else {
                return;
            };
            let scopes = granted.as_slice().to_vec();
            let changed = stored.granted_scopes.as_ref() != Some(&scopes);
            stored.granted_scopes = Some(scopes);
            let health = crate::allodia_health::health_from_scopes(stored.granted_scopes.as_ref());
            (changed, health)
        };
        let (changed, health) = health;
        if let Some(health) = health {
            self.note_allodia_health(health);
        }
        // Written back only when it actually moved. A store write per refresh is a keychain
        // prompt's worth of noise on some hosts, for a value that changes about once a release.
        if changed {
            self.persist_allodia_grant();
        }
    }

    /// Record a rotated refresh token, in memory and in the host's store.
    ///
    /// Nothing is returned and nothing is rolled back, because there is nothing to roll back
    /// to: the token this replaces is already spent, and the session carries on from the one
    /// in hand. A store that refused is an account that will need signing in again at the next
    /// launch, which is what the `error!` says.
    fn store_rotated_allodia_grant(&self, rotated: &str) {
        let config = {
            let mut signed_in = self.allodia.lock().expect("allodia account lock");
            let Some(stored) = signed_in.as_mut() else {
                // Signed out while the refresh was in flight. The grant is gone on purpose;
                // writing it back would resurrect it.
                return;
            };
            stored.refresh_token = rotated.to_owned();
            stored.to_toml()
        };
        let config = match config {
            Ok(config) => config,
            Err(error) => {
                log::error!("allodia: the rotated grant could not be serialized; {error}");
                return;
            }
        };
        match self.credential_store.persist(ACCOUNT_ID.to_owned(), config) {
            Ok(()) => log::info!("allodia: the rotated grant is stored"),
            Err(error) => log::error!(
                "allodia: the rotated grant could not be stored ({error}); this install will \
                 need signing in again at the next launch"
            ),
        }
    }

    /// Write the stored grant back as it now stands.
    ///
    /// Best-effort, like the rotation above: what is in memory is what this session uses, and a
    /// store that refused costs the next launch a re-read it can survive: the scope set goes back
    /// to not-known, which withholds nothing.
    fn persist_allodia_grant(&self) {
        let config = {
            let signed_in = self.allodia.lock().expect("allodia account lock");
            let Some(stored) = signed_in.as_ref() else {
                return;
            };
            stored.to_toml()
        };
        match config {
            Ok(config) => {
                if let Err(error) = self.credential_store.persist(ACCOUNT_ID.to_owned(), config) {
                    log::warn!("allodia: the grant's permissions could not be stored; {error}");
                }
            }
            Err(error) => {
                log::error!("allodia: the grant could not be serialized; {error}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use mailcal_oauth::Secret;

    use super::{Duration, REFRESH_SKEW, Tokens};

    fn held(expires_in_minutes: i64) -> mailcal_oauth::TokenSet {
        mailcal_oauth::TokenSet {
            access_token: Secret::new("AT".to_owned()),
            refresh_token: None,
            expires_at: time::OffsetDateTime::now_utc() + Duration::minutes(expires_in_minutes),
            scope: String::new(),
            token_type: "Bearer".to_owned(),
        }
    }

    /// The regression this exists for, found by signing in again on a real account.
    ///
    /// A token is held for about an hour, so a sign-in that stored a NEW grant and left the old
    /// token cached went on presenting it, and the service refused it, because the new
    /// authorisation superseded the grant it came from. On screen that is signing in successfully
    /// and being told a fraction of a second later that you are signed out. Nothing cleared this
    /// cache on sign-in OR on sign-out; it stayed hidden while the only way in was from a
    /// signed-out state, where there is no stale token to present.
    #[test]
    fn forgetting_the_grant_forgets_the_token_minted_from_it() {
        let tokens = Tokens::default();
        let now = time::OffsetDateTime::now_utc();
        *tokens.access.lock().unwrap() = Some(held(45));
        assert_eq!(
            tokens.live(now).as_deref(),
            Some("AT"),
            "a live token is served from the cache, which is the whole reason a stale one is a bug"
        );
        tokens.forget();
        assert!(
            tokens.live(now).is_none(),
            "after the grant is replaced or erased, nothing minted from it may be presented again"
        );
    }

    #[test]
    fn a_token_inside_the_skew_is_already_spent() {
        let tokens = Tokens::default();
        *tokens.access.lock().unwrap() = Some(held(REFRESH_SKEW.whole_minutes() - 1));
        assert!(
            tokens.live(time::OffsetDateTime::now_utc()).is_none(),
            "a token handed out with seconds left dies mid-request"
        );
    }
}
