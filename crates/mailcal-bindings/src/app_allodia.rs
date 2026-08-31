//! The Allodia sign-in FFI methods on [`MailcalApp`]. Split out of `allodia.rs`, which holds the
//! records and the stored shape, to keep each file under the 500-line limit; UniFFI collects these
//! exported methods crate-wide.
//!
//! Two calls with a person in between, the same shape every other browser sign-in here has:
//! [`MailcalApp::begin_allodia_sign_in`] discovers the service and mints the authorization URL plus
//! an opaque `pending` handle the host holds, and
//! [`MailcalApp::complete_allodia_sign_in`] takes the browser's redirect back, exchanges the code,
//! asks the service whose account it is, and writes the grant to the host's secure store itself.
//!
//! **Both block.** They make network round trips, so a host calls them off the main thread;
//! exactly as it already does for `begin_jmap_login` and `detect_account_settings`.

#[cfg(feature = "allodia-license")]
use crate::allodia::StoredAccount;
use crate::{
    MailcalApp, MailcalError,
    allodia::{ACCOUNT_ID, AllodiaAccount, AllodiaSignInStart},
};

/// What the host round-trips as the opaque `pending` handle: the CSRF `state`, the PKCE verifier,
/// and what discovery found.
///
/// It carries the verifier, so it is never persisted. The endpoints ride along because the exchange
/// has to reach the **same** token endpoint the browser was sent to; re-discovering them after the
/// code has been issued would add two more ways to lose a code that cannot be re-obtained.
#[cfg(feature = "allodia-license")]
#[derive(serde::Serialize, serde::Deserialize)]
struct PendingAllodiaSignIn {
    redirect_uri: String,
    state: String,
    verifier: String,
    endpoints: allodia_license::Endpoints,
}

/// The answer every entry point gives in a build that carries no Allodia registration.
///
/// A client checks [`allodia_sign_in_available`](crate::allodia_sign_in_available) first and never
/// reaches these, so this is a wiring bug rather than something a user can provoke, which is why
/// it is one sentence and not a diagnosis.
#[cfg(not(feature = "allodia-license"))]
pub(crate) fn unavailable<T>() -> Result<T, MailcalError> {
    Err(MailcalError::Config(
        "this build carries no Allodia sign-in".to_owned(),
    ))
}

#[uniffi::export]
impl MailcalApp {
    /// Starts an Allodia sign-in: reads the service's own OAuth metadata and builds the PKCE
    /// authorization URL to open.
    ///
    /// `redirect_uri` stays the host's because only the host knows it: the desktop and mobile
    /// clients that claim a URI scheme redirect to `<application-id>://account-oauth`, and Linux
    /// binds a loopback port per flow.
    ///
    /// **Blocking** (it makes two discovery round trips); call it off the main thread.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Config`] if this build carries no Allodia sign-in, or
    /// [`MailcalError::Connect`] if the service's metadata could not be read.
    pub fn begin_allodia_sign_in(
        &self,
        redirect_uri: String,
    ) -> Result<AllodiaSignInStart, MailcalError> {
        #[cfg(not(feature = "allodia-license"))]
        {
            drop(redirect_uri);
            unavailable()
        }
        #[cfg(feature = "allodia-license")]
        {
            self.begin_allodia(redirect_uri, allodia_license::Prompt::SignIn)
        }
    }

    /// Starts an Allodia **registration**: the same flow, asking the service for its sign-up page
    /// rather than its sign-in one.
    ///
    /// Two entry points rather than one flag, because a client draws two controls and the call
    /// site should say which. [`Self::complete_allodia_sign_in`] finishes either.
    ///
    /// A service that does not advertise `prompt=create` gets an ordinary authorization request,
    /// so this never fails for want of support: its sign-in page is where someone registers
    /// anyway.
    ///
    /// **Blocking**; call it off the main thread.
    ///
    /// # Errors
    ///
    /// As [`Self::begin_allodia_sign_in`].
    pub fn begin_allodia_registration(
        &self,
        redirect_uri: String,
    ) -> Result<AllodiaSignInStart, MailcalError> {
        #[cfg(not(feature = "allodia-license"))]
        {
            drop(redirect_uri);
            unavailable()
        }
        #[cfg(feature = "allodia-license")]
        {
            self.begin_allodia(redirect_uri, allodia_license::Prompt::Create)
        }
    }

    /// The page where someone manages the account, including deleting it.
    ///
    /// `None` when this build has no Allodia sign-in, which is the same answer
    /// [`crate::allodia_sign_in_available`] gives and the same reason to draw nothing.
    ///
    /// A client opens it in the platform's **in-app browser tab**: the one the authorization
    /// request already uses; because that shares the system browser's cookies and the page then
    /// opens already signed in. An embedded web view has its own cookie jar and would show a login
    /// page instead.
    #[must_use]
    pub fn allodia_account_url(&self) -> Option<String> {
        #[cfg(not(feature = "allodia-license"))]
        {
            None
        }
        #[cfg(feature = "allodia-license")]
        {
            allodia_license::available().then(allodia_license::account_url)
        }
    }

    /// Completes an Allodia sign-in from the host's held `pending` handle and the browser's
    /// redirect `callback_url`: validates the redirect, exchanges the code, asks the service whose
    /// account it is, and **writes the grant to the host's secure store** through
    /// [`crate::credential_store`]. Returns who signed in; the host stores nothing itself.
    ///
    /// **Blocking**; call it off the main thread.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Config`] if this build carries no Allodia sign-in or `pending` is
    /// malformed, and [`MailcalError::Connect`] if the user cancelled, the exchange failed, the
    /// service issued no refresh token, or it would not say whose account this is. In every case
    /// nothing is stored and any previous sign-in is left as it was.
    pub fn complete_allodia_sign_in(
        &self,
        pending: String,
        callback_url: String,
    ) -> Result<AllodiaAccount, MailcalError> {
        #[cfg(not(feature = "allodia-license"))]
        {
            drop((pending, callback_url));
            unavailable()
        }
        #[cfg(feature = "allodia-license")]
        {
            let pending: PendingAllodiaSignIn = serde_json::from_str(&pending)
                .map_err(|err| MailcalError::Config(err.to_string()))?;
            let signin = allodia_license::SignIn::resume(&pending.redirect_uri, pending.endpoints)
                .map_err(|err| MailcalError::Config(err.to_string()))?;
            log::info!("allodia: redirect received; exchanging the code");
            let tokens = self
                .runtime
                .block_on(signin.complete(
                    &callback_url,
                    &pending.state,
                    &pending.verifier,
                    time::OffsetDateTime::now_utc(),
                ))
                .map_err(|err| {
                    // The service's own machine-readable reason, which is the difference between a
                    // stale code and a token minted for the wrong audience. Without it the user
                    // only ever sees "signing in didn't work".
                    log::warn!("allodia: token exchange FAILED; {err}");
                    MailcalError::Connect(err.to_string())
                })?;
            // `offline_access` is always requested, so a refresh token is mandatory. Without one
            // the sign-in is a session that expires within the hour and cannot come back, which is
            // the whole problem OAuth was chosen here to avoid.
            let refresh_token = tokens.refresh_token.ok_or_else(|| {
                log::warn!(
                    "allodia: exchange succeeded but the service issued NO refresh token; granted \
                     scope(s): [{}]",
                    tokens.scope,
                );
                MailcalError::Connect(
                    "the account service issued no refresh token (offline_access was requested)"
                        .to_owned(),
                )
            })?;
            let identity = self
                .runtime
                .block_on(signin.identity(tokens.access_token.expose()))
                .map_err(|err| {
                    log::warn!("allodia: the service would not say whose account this is; {err}");
                    MailcalError::Connect(err.to_string())
                })?;
            // What the service issued, which is what every later "may this build do X?" is
            // answered against. A response that named no scope means it granted what was asked
            // for, so the request is the record; see `GrantedScopes::from_response`.
            let granted = mailcal_oauth::GrantedScopes::from_response(
                &tokens.scope,
                signin.requested_scopes(),
            );
            let missing = granted.missing(
                &allodia_license::Feature::ALL
                    .iter()
                    .map(|feature| feature.scope())
                    .collect::<Vec<_>>(),
            );
            if !missing.is_empty() {
                // Not a failure: the sign-in worked and the person is signed in. It is the one
                // line that explains a feature being asleep on an account that looks fine.
                log::info!(
                    "allodia: signed in, but the service issued no [{}]; those features stay off \
                     until it does",
                    missing.join(", "),
                );
            }
            let stored = StoredAccount {
                email: identity.email,
                name: identity.name,
                refresh_token: refresh_token.expose().to_owned(),
                granted_scopes: Some(granted.as_slice().to_vec()),
                end_session_endpoint: signin.end_session_url(),
            };
            // Store before reporting success. A grant only this process knows about is one the next
            // launch has no way to find, and the user would have signed in to nothing.
            let config = stored
                .to_toml()
                .map_err(|err| MailcalError::Config(err.to_string()))?;
            self.credential_store
                .persist(ACCOUNT_ID.to_owned(), config)
                .map_err(|err| {
                    log::error!("allodia: the grant could not be stored; {err}");
                    MailcalError::Connect(err.to_string())
                })?;
            let account = stored.account();
            *self.allodia.lock().expect("allodia account lock") = Some(stored);
            // The token held for the process was minted from the grant this one replaces, and the
            // service refuses it the moment the new authorisation supersedes that grant. Left in
            // place it is presented for its full hour, so signing in reads as being signed out a
            // fraction of a second later.
            self.forget_allodia_access_token();
            // And this sign-in is the freshest word on what the grant may do, so the screen stops
            // asking now rather than at the next refresh.
            if let Some(health) =
                crate::allodia_health::health_from_scopes(Some(&granted.as_slice().to_vec()))
            {
                self.note_allodia_health(health);
            }
            log::info!("allodia: signed in; the grant is stored");
            Ok(account)
        }
    }

    /// Who is signed in to an Allodia account, or `None`.
    ///
    /// Cheap and local; it reads what the last launch restored or the last sign-in wrote, and
    /// never asks the service. Nothing a client draws from an Allodia account may wait on the
    /// network, that rule is `entitlement.md`'s and starts here.
    #[must_use]
    pub fn allodia_account(&self) -> Option<AllodiaAccount> {
        self.allodia
            .lock()
            .expect("allodia account lock")
            .as_ref()
            .map(|stored| stored.account())
    }

    /// Signs out: forgets the account, erases its stored grant, and returns where to send the
    /// browser to end **its** session, when the service advertised one.
    ///
    /// Erasing comes first and touches no network, so a sign-out cannot fail halfway and leave
    /// someone signed in. The returned URL is the client's to open afterwards, and it is
    /// optional in both senses: `None` when nothing was signed in or the service advertised no
    /// endpoint, and a failure to open it changes nothing here.
    ///
    /// ⚠️ **It does not end this install's grant at the service, and neither does anything else
    /// here.** That endpoint closes the browser session and the tokens bound to it, but a refresh
    /// token carrying `offline_access` is preserved on purpose, and this build requests
    /// `offline_access`. What the erase guarantees is that **this install** can no longer use the
    /// grant; the grant itself lives at the service until it expires. `entitlement.md` records why
    /// revoking it is not currently open to a public client.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Connect`] if the platform store refused the delete. The account is
    /// forgotten in memory either way: an entry that outlives the sign-out comes back at the next
    /// launch with no explanation, so the host is told rather than the removal being reverted.
    pub fn sign_out_of_allodia(&self) -> Result<Option<String>, MailcalError> {
        let stored = self.allodia.lock().expect("allodia account lock").take();
        let Some(stored) = stored else {
            return Ok(None);
        };
        let end_session = stored.end_session_endpoint.clone();
        // Same rule as the sign-in above: the grant is gone, so nothing minted from it may be
        // presented again. A signed-out device holding a live token is one that goes on making
        // authenticated requests for as long as the token lasts.
        #[cfg(feature = "allodia-license")]
        self.forget_allodia_access_token();
        // And what this device remembers having said to the service. Those record ids belong to
        // the account that is leaving; carried into the next sign-in they match nothing, so
        // nothing is claimed and the person is offered back the mail accounts they already have.
        // Their own choices about which accounts travel are not part of that and stay.
        #[cfg(feature = "allodia-license")]
        if let Ok(bookkeeping) = self.allodia_bookkeeping()
            && let Err(err) = bookkeeping.forget_the_session()
        {
            // Not fatal, and not a reason to keep somebody signed in: the next pass adopts what it
            // finds, which is the same repair a bookkeeping that will not parse already gets.
            log::warn!("allodia: the sync bookkeeping could not be cleared; {err}");
        }
        log::info!("allodia: signing out; erasing the stored grant");
        self.credential_store
            .delete(ACCOUNT_ID.to_owned())
            .map_err(|err| {
                log::error!("allodia: the stored grant could not be erased; {err}");
                MailcalError::Connect(err.to_string())
            })?;
        Ok(end_session)
    }
}

/// The part of a sign-in that is not exported: shared by the two entry points above.
impl MailcalApp {
    /// The body both entry points share: discover, build the URL, and package the handle.
    #[cfg(feature = "allodia-license")]
    fn begin_allodia(
        &self,
        redirect_uri: String,
        prompt: allodia_license::Prompt,
    ) -> Result<AllodiaSignInStart, MailcalError> {
        {
            log::info!("allodia: sign-in requested; reading the service's OAuth metadata");
            let signin = self
                .runtime
                .block_on(allodia_license::SignIn::discover(&redirect_uri))
                .map_err(|err| {
                    log::warn!("allodia: sign-in could not start; {err}");
                    MailcalError::Connect(err.to_string())
                })?;
            let request = signin.begin(prompt);
            let pending = PendingAllodiaSignIn {
                redirect_uri,
                state: request.state,
                verifier: request.pkce.verifier().to_owned(),
                endpoints: signin.endpoints(),
            };
            let pending = serde_json::to_string(&pending)
                .map_err(|err| MailcalError::Config(err.to_string()))?;
            log::info!("allodia: opening the authorization page; awaiting the redirect");
            Ok(AllodiaSignInStart {
                authorization_url: request.authorization_url,
                pending,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use crate::{
        LogLevel, MailcalApp, allodia_sign_in_available,
        tests::{ChannelObserver, NullLogger},
    };

    /// A demo app: bundled fixtures, and a credential store that **refuses** every write, which is
    /// what makes the sign-out test below mean something.
    fn app() -> std::sync::Arc<MailcalApp> {
        let (tx, _rx) = mpsc::channel();
        MailcalApp::new_demo(
            Box::new(ChannelObserver { tx }),
            Box::new(NullLogger),
            LogLevel::Info,
            "Etc/UTC".to_owned(),
        )
    }

    /// The surface exists in every build; what changes is the answer. A client draws the button
    /// from this, so a build that cannot sign in has to say so rather than offer a dead route.
    #[test]
    fn the_route_is_offered_only_by_a_build_that_carries_one() {
        #[cfg(not(feature = "allodia-license"))]
        assert!(!allodia_sign_in_available());
        #[cfg(feature = "allodia-license")]
        assert_eq!(allodia_sign_in_available(), allodia_license::available());
    }

    #[test]
    fn nobody_is_signed_in_until_somebody_signs_in() {
        assert_eq!(app().allodia_account(), None);
    }

    /// Signing out when nobody is signed in touches the store at all, and this app's store refuses
    /// every call, so if it did, this would fail. The rule it pins: the desired end state is that
    /// nothing is stored, and that is already true.
    #[test]
    fn signing_out_when_nobody_is_signed_in_is_a_success_and_writes_nothing() {
        assert!(app().sign_out_of_allodia().is_ok());
    }

    /// A build with no registration refuses both halves rather than reaching the network. A client
    /// checks availability first, so this is the answer to a wiring bug, but it must be an answer
    /// and not a hang.
    #[cfg(not(feature = "allodia-license"))]
    #[test]
    fn a_build_without_a_registration_refuses_the_flow_offline() {
        let app = app();
        assert!(
            app.begin_allodia_sign_in("app://account-oauth".to_owned())
                .is_err()
        );
        assert!(
            app.complete_allodia_sign_in("{}".to_owned(), "app://account-oauth?code=x".to_owned())
                .is_err()
        );
    }
}
