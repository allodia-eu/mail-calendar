//! The DAV half of a standards account: its CardDAV section, the endpoint its calendar and its
//! contacts each connect to, and the id of an account that has no mailbox.
//!
//! Split from `config.rs`, which holds the account as a whole; this is the part a calendar and
//! contacts account without mail is made of.

use std::sync::Arc;

use engine_core::ids::{AccountId, IdError};
use engine_provider::{Credential, CredentialSource, CredentialSubject, FixedCredential};
use mailcal_oauth::DavUse;
use serde::Deserialize;

use crate::{AccountConfig, AccountError, AccountTokens, Secret, imap_credentials};

mod issuer_bound;

use issuer_bound::IssuerBound;

/// A CardDAV address-book endpoint.
///
/// Optional even beside a `[caldav]` section: nearly every server that speaks CalDAV for an
/// account speaks CardDAV at the same origin with the same login, so contacts are found from
/// the calendar's endpoint when this section is absent. It exists for the server that splits
/// them, and for an account used for contacts alone.
#[derive(Debug, Clone, Deserialize)]
pub struct CardDavAccount {
    /// The base URL discovery starts from (e.g. `https://cloud.example/remote.php/dav`).
    pub base_url: String,
    /// The login username.
    pub username: String,
    /// The login password (or app password). `None` on an OAuth account, which presents the
    /// account's grant instead.
    #[serde(default)]
    pub password: Option<Secret>,
}

/// Where one DAV capability connects, and as whom.
#[derive(Debug, Clone, Copy)]
pub(crate) struct DavEndpoint<'a> {
    /// The base URL discovery starts from.
    pub(crate) base_url: &'a str,
    /// The login username.
    pub(crate) username: &'a str,
    /// The stored password, when the account has one.
    pub(crate) password: Option<&'a Secret>,
}

impl AccountConfig {
    /// Where the account's calendar connects, when it has a calendar endpoint.
    pub(crate) fn caldav_endpoint(&self) -> Option<DavEndpoint<'_>> {
        self.caldav.as_ref().map(|caldav| DavEndpoint {
            base_url: &caldav.base_url,
            username: &caldav.username,
            password: caldav.password.as_ref(),
        })
    }

    /// Where the account's contacts connect: its own `[carddav]` section, or the calendar's
    /// endpoint when it has none.
    pub(crate) fn carddav_endpoint(&self) -> Option<DavEndpoint<'_>> {
        self.carddav
            .as_ref()
            .map(|carddav| DavEndpoint {
                base_url: &carddav.base_url,
                username: &carddav.username,
                password: carddav.password.as_ref(),
            })
            .or_else(|| self.caldav_endpoint())
    }

    /// The id of an account without a mailbox: its DAV login and host, tagged `dav:` so it can
    /// never meet an IMAP account's `username@host` for the same login.
    pub(crate) fn dav_account_id(&self) -> Result<AccountId, IdError> {
        let endpoint = self.caldav_endpoint().or_else(|| self.carddav_endpoint());
        let (username, host) = endpoint.map_or((String::new(), String::new()), |endpoint| {
            (
                endpoint.username.trim().to_lowercase(),
                base_host(endpoint.base_url),
            )
        });
        AccountId::try_from(format!("{username}@dav:{host}").as_str())
    }
}

/// The credential source a DAV endpoint of `account` presents for `dav_use`: Basic with the
/// endpoint's own login, or, on an OAuth account, the account's own token source, asked per
/// request and sent only where `draft-ietf-mailmaint-oauth-public` lets it go
/// (`docs/mail-oauth.md` rule 10).
///
/// A grant reaches a DAV server when its sign-in asked for that use's scope and the server's URL
/// is one its issuer's tokens are for (§3.6), and the token then goes as Bearer under an
/// [`IssuerBound`] source that applies the issuer's rule to every origin discovery moves to.
/// Anything else is [`AccountError::DavNeedsOwnSignIn`], before a token is minted.
pub(crate) fn dav_credential_source(
    account: &AccountConfig,
    endpoint: DavEndpoint<'_>,
    dav_use: DavUse,
    tokens: AccountTokens<'_>,
) -> Result<Arc<dyn CredentialSource>, AccountError> {
    if let Some(grant) = &account.oauth {
        if !grant_reaches(grant, endpoint.base_url, dav_use) {
            return Err(AccountError::dav_needs_own_sign_in(dav_use));
        }
        let tokens = tokens.ok_or(AccountError::MissingCredential(
            imap_credentials::NO_TOKEN_SOURCE,
        ))?;
        return Ok(Arc::new(IssuerBound {
            tokens: Arc::clone(tokens),
            issuer: grant.issuer_identifier(),
            protected_resources: grant.protected_resources.clone(),
        }));
    }
    let password = endpoint
        .password
        .ok_or_else(|| AccountError::CalDavDiscovery("no DAV credential stored".to_owned()))?;
    Ok(Arc::new(FixedCredential::new(
        dav_subject(account, endpoint, password, dav_use)?,
        Credential::password(endpoint.username, password.expose()),
    )))
}

/// Who a DAV endpoint signing in with its own stored `password` signs in as, to the engine: the
/// account's own subject when it is the mailbox's login (the same username and password), and
/// otherwise one naming the account and `dav_use`, so two logins never share a subject.
fn dav_subject(
    account: &AccountConfig,
    endpoint: DavEndpoint<'_>,
    password: &Secret,
    dav_use: DavUse,
) -> Result<CredentialSubject, AccountError> {
    let mailbox_login = account.imap.as_ref().is_some_and(|imap| {
        imap.username == endpoint.username
            && imap
                .password
                .as_ref()
                .is_some_and(|mailbox| mailbox.expose() == password.expose())
    });
    if mailbox_login {
        return account.credential_subject();
    }
    let id = account
        .account_id()
        .map_err(|_| AccountError::MissingCredential("the account has no id to sign in as"))?;
    let dav_use = match dav_use {
        DavUse::Calendars => "calendars",
        DavUse::Contacts => "contacts",
    };
    Ok(CredentialSubject::new(format!("{}#{dav_use}", id.as_str())))
}

/// Whether `grant`'s token may go to the DAV server at `base_url` for `dav_use`
/// (`docs/mail-oauth.md` rule 10): its sign-in asked for that use's scope, and the server's URL is
/// one its issuer's tokens are for (§3.6). Whether the token may travel to it over plaintext is
/// the engine's rule, not this one's: a plaintext address off this device fails its connect as a
/// server reached only over plaintext (`AccountError::InsecureServer`), not as one the grant does
/// not reach.
///
/// The URL is parsed as the transport parses it, so an IPv6 literal, an internationalised name or
/// a userinfo part is judged as the request will go.
#[must_use]
pub fn grant_reaches(grant: &crate::OAuthGrant, base_url: &str, dav_use: DavUse) -> bool {
    let url = reqwest::Url::parse(&crate::setup::normalize_caldav_base_url(base_url)).ok();
    mailcal_oauth::requests_dav_use(&grant.scopes, dav_use)
        && url.is_some_and(|url| grant.token_may_reach(url.as_str()))
}

/// The host (with any port), lowercased, of a base URL: no URL parsing, so the id it goes into
/// is the same string however the URL was spelled after the host.
fn base_host(base_url: &str) -> String {
    let rest = base_url
        .trim()
        .split_once("://")
        .map_or(base_url.trim(), |(_, rest)| rest);
    rest.split('/').next().unwrap_or(rest).to_lowercase()
}

#[cfg(test)]
#[path = "config_dav_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "config_dav_grant_tests.rs"]
mod grant_tests;

#[cfg(test)]
#[path = "config_dav_hop_tests.rs"]
mod hop_tests;
