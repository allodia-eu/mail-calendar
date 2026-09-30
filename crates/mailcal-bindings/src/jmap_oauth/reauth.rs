//! Signing an existing OAuth JMAP account back in, when the server has stopped accepting its
//! stored grant.
//!
//! `docs/provider-oauth.md` rule 12 raises `signin_expired_accounts` for a grant that is
//! **dead**; expired or revoked, minting no access token at all. For Microsoft and Google the
//! remedy has always been one button, because their sign-in can simply be re-run from a build-time
//! client id. A JMAP account had no such button: its endpoints and client id are *discovered*, and
//! nothing in the FFI could re-authorise an account that already existed, so the banner pointed at
//! Settings; where the only cure was to remove the account and add it back.
//!
//! # Re-authorise from the persisted grant, never from a re-discovery
//!
//! [`MailcalApp::begin_jmap_reauth`] makes **no network calls**. It reads the account's own
//! `[jmap.oauth]` grant: the authorization endpoint, the registered client id, the redirect URI,
//! the scopes and the RFC 8707 resource indicator, and builds a fresh PKCE authorisation from
//! exactly those. That is what those fields are persisted for (`mailcal_account::OAuthGrant`:
//! "kept so a re-consent needs no re-discovery"), and it is the only way to be sure the
//! re-authorisation asks the *same* server, as the *same* registered client, for the *same*
//! scopes. Re-running RFC 7591 registration instead would mint a **second** client id on the
//! user's account for every reconnect, and leave the first one orphaned.
//!
//! # The completion swaps the grant in place
//!
//! [`MailcalApp::complete_jmap_reauth`] does not go back through `add_account`: this account
//! already exists, with a sync depth the user chose and mail already downloaded. It connects with
//! the new grant, persists it through the host's
//! [`AccountCredentialStore`](crate::credential_store), replaces the registry entry, and retracts
//! the expired-sign-in prompt: the JMAP twin of what `complete_microsoft_login` does for a
//! Microsoft re-consent.

use engine_core::ids::AccountId;
use mailcal_account::JmapAccountConfig;

use super::{JmapLoginStart, start_login};
use crate::{
    MailcalApp, MailcalError, account_registry::JmapLookup, account_repair::CredentialPersistence,
    boot,
};

#[uniffi::export]
impl MailcalApp {
    /// Starts a re-authentication of the existing OAuth JMAP account `account_id`: builds a fresh
    /// PKCE authorization URL from the account's **persisted** grant, for the host to open in its
    /// platform auth session exactly as it does a first sign-in. Pass the redirect back to
    /// [`MailcalApp::complete_jmap_reauth`] with the same `account_id`.
    ///
    /// Unlike [`MailcalApp::begin_jmap_login`] this makes no network calls and re-registers
    /// nothing, so it is fast, but it is still safe to call off the main thread, and hosts
    /// should, since the two paths otherwise look identical from a client.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Config`] if `account_id` is unknown, is not a JMAP account, or is
    /// a JMAP account authenticated with a stored password/API token, which has no browser flow
    /// to re-run and must be re-entered in Settings. Ask `MailcalApp::account_provider` first:
    /// only [`AccountProvider::JmapOauth`](crate::AccountProvider::JmapOauth) can take this path.
    pub fn begin_jmap_reauth(&self, account_id: String) -> Result<JmapLoginStart, MailcalError> {
        let config = self.stored_jmap_config(&account_id)?;
        let grant = config.oauth.as_ref().ok_or_else(|| {
            MailcalError::Config(
                "this JMAP account signs in with a stored password or API token, which has no \
                 sign-in to re-run"
                    .to_owned(),
            )
        })?;
        log::info!(
            "jmap oauth: re-authenticating an existing account against its stored authorization \
             endpoint {} (no re-discovery, no re-registration)",
            grant.authorize_endpoint,
        );
        start_login(config.email.clone(), config.base_url.clone(), grant)
    }

    /// Completes a re-authentication started by [`MailcalApp::begin_jmap_reauth`]: exchanges the
    /// redirect for a fresh grant, connects `account_id` with it, persists it through the host's
    /// [`AccountCredentialStore`](crate::credential_store), and retracts the account's
    /// expired-sign-in prompt.
    ///
    /// The account keeps its identity, its settings and its downloaded mail: only the credential
    /// changes: so a host does **not** call `add_account` afterwards, and does **not** write the
    /// secure store itself: the core does both, because it is the side that knows the new grant
    /// actually connects.
    ///
    /// **Blocking** (token exchange plus a provider connect); call it off the main thread.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Config`] if `pending` is malformed **or the user signed in as a
    /// different account** than the one being reconnected, and [`MailcalError::Connect`] if the
    /// exchange, the connect with the new grant, or the credential write failed. In every failure
    /// case the account's existing stored grant is left untouched and its prompt stays up for
    /// another attempt.
    pub fn complete_jmap_reauth(
        &self,
        account_id: String,
        pending: String,
        callback_url: String,
    ) -> Result<(), MailcalError> {
        // Prove the account is still a known OAuth JMAP one before spending a code on it: the
        // user may have removed it while the browser was up.
        self.stored_jmap_config(&account_id)?;
        let config = self
            .exchange_jmap_login(pending, callback_url)
            .inspect_err(|err| {
                // Support asks "I tapped Sign in again and nothing happened"; say what the server
                // said. The message is the OAuth protocol string, which predates any token mint and
                // names endpoints, never the address or a secret.
                log::warn!(
                    "jmap oauth: re-authentication did not complete ({err}): the stored grant \
                     is unchanged and the reconnect prompt stays up"
                );
            })?;
        same_account(&account_id, &config)?;
        let config_toml = config
            .to_toml()
            .map_err(|err| MailcalError::Config(err.to_string()))?;
        // Register the NEW grant before dialing with it, exactly as `add_account` does: the dial's
        // first act is to mint an access token, and a server that rotates hands back a replacement
        // right there. Without an entry carrying this grant, that rotation would land on the *old*
        // config; with one, it lands where the persist below reads from.
        //
        // The displaced entry is kept, which matters here more than anywhere: this is the
        // re-authentication path, so there is always a live account underneath, and a failure must
        // give it back untouched rather than delete it.
        let sink = crate::token_sink::token_sink(&self.registry, &self.credential_store);
        // FreshSignIn: the grant this account had is dead, so any token state this process still
        // holds for it describes a credential that no longer exists.
        let prepared = boot::prepare_stored_account(
            &config_toml,
            &sink,
            mailcal_account::CredentialOrigin::FreshSignIn,
        )?;
        self.install_repaired_account(
            &account_id,
            prepared,
            CredentialPersistence::RegisteredGrant,
            "jmap oauth",
        )?;
        log::info!("jmap oauth: re-authentication complete: the account is connected again");
        Ok(())
    }
}

impl MailcalApp {
    /// The stored JMAP config for `account_id`, or a [`MailcalError::Config`] naming why this
    /// account cannot be re-authenticated. Cloned out from under the registry lock, which is
    /// never held across the network work that follows.
    fn stored_jmap_config(&self, account_id: &str) -> Result<JmapAccountConfig, MailcalError> {
        self.registry.jmap_config(account_id).map_err(|reason| {
            MailcalError::Config(
                match reason {
                    JmapLookup::NotJmap => "this is not a JMAP account",
                    JmapLookup::Unknown => "no such account",
                }
                .to_owned(),
            )
        })
    }
}

/// Checks that the account just signed into is the one being reconnected, returning its
/// [`AccountId`].
///
/// The authorisation page lets the user pick; `login_hint` targets an address, it does not pin
/// one: so someone with two mailboxes at the same provider can complete this flow as the wrong
/// one. Writing that grant into `account_id`'s slot would point one account's config at another
/// person's mail while still displaying the original address: silent, and impossible to see from
/// the UI. So it is refused, and the reconnect prompt stays up.
fn same_account(account_id: &str, config: &JmapAccountConfig) -> Result<AccountId, MailcalError> {
    let signed_in = config
        .account_id()
        .map_err(|err| MailcalError::Engine(err.to_string()))?;
    if signed_in.as_str() != account_id {
        log::warn!(
            "jmap oauth: the re-authentication signed in a DIFFERENT account than the one being \
             reconnected; discarding the new grant",
        );
        return Err(MailcalError::Config(
            "that sign-in is for a different account than the one being reconnected".to_owned(),
        ));
    }
    Ok(signed_in)
}

#[cfg(test)]
mod tests {
    use mailcal_account::{OAuthGrant, Secret};

    use super::{JmapAccountConfig, same_account};
    use crate::{AccountProvider, ConnectedAccount};

    fn grant() -> OAuthGrant {
        OAuthGrant {
            client_id: "client-abc".to_owned(),
            client_secret: None,
            refresh_token: Secret::new("rt-value".to_owned()),
            authorize_endpoint: "https://api.example.com/oauth/authorize".to_owned(),
            token_endpoint: "https://api.example.com/oauth/refresh".to_owned(),
            redirect_uri: "eu.allodia.mailcal://jmap-oauth".to_owned(),
            scopes: vec!["offline_access".to_owned()],
            resource: Some("https://api.example.com/jmap/session".to_owned()),
            issuer: None,
        }
    }

    fn config(email: &str, oauth: Option<OAuthGrant>) -> JmapAccountConfig {
        JmapAccountConfig {
            email: email.to_owned(),
            base_url: "https://api.example.com".to_owned(),
            password: oauth.is_none().then(|| Secret::new("secret".to_owned())),
            token: None,
            oauth,
        }
    }

    /// The whole reconnect button hangs off this: a host reads `account_provider` to decide
    /// whether there is a sign-in to re-run at all, and a JMAP account can be either kind.
    #[test]
    fn an_oauth_jmap_account_reports_a_different_provider_than_a_secret_one() {
        let signed_in = ConnectedAccount::Jmap {
            config: config("alice@example.com", Some(grant())),
            tokens: None,
        };
        let pasted_secret = ConnectedAccount::Jmap {
            config: config("alice@example.com", None),
            tokens: None,
        };

        assert!(matches!(signed_in.provider(), AccountProvider::JmapOauth,));
        assert!(matches!(pasted_secret.provider(), AccountProvider::Jmap));
    }

    #[test]
    fn re_authenticating_the_same_address_is_accepted() {
        let config = config("alice@example.com", Some(grant()));
        let id = config.account_id().unwrap();

        assert_eq!(
            same_account(id.as_str(), &config).unwrap().as_str(),
            id.as_str(),
        );
    }

    /// `login_hint` is a hint, not a constraint, so the browser can come back as a different
    /// mailbox. Accepting it would file one person's grant under another's account id.
    #[test]
    fn re_authenticating_as_a_different_address_is_refused() {
        let other = config("bob@example.com", Some(grant()));
        let reconnecting = config("alice@example.com", Some(grant()))
            .account_id()
            .unwrap();

        let err = same_account(reconnecting.as_str(), &other).unwrap_err();

        assert!(
            err.to_string().contains("different account"),
            "the message must say what went wrong: {err}",
        );
    }
}
