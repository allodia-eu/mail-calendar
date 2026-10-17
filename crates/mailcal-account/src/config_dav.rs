//! The DAV half of a standards account: its CardDAV section, the endpoint its calendar and its
//! contacts each connect to, and the id of an account that has no mailbox.
//!
//! Split from `config.rs`, which holds the account as a whole; this is the part a calendar and
//! contacts account without mail is made of.

use std::sync::Arc;

use engine_core::ids::{AccountId, IdError};
use mailcal_oauth::DavUse;
use provider_caldav::{CredentialHosts, Credentials};
use serde::Deserialize;

use crate::{AccountConfig, AccountError, ImapTokens, Secret, imap_credentials};

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

/// How a DAV endpoint of `account` authenticates: the credential, and the hosts it may follow a
/// discovery redirect to when that is not the transport's default.
pub(crate) struct DavAuth {
    pub(crate) credentials: Credentials,
    pub(crate) hosts: Option<Arc<dyn CredentialHosts>>,
}

/// The credential a DAV endpoint of `account` presents for `dav_use`: Basic with the endpoint's
/// own login, or, on an OAuth account, the grant's access token, sent only where
/// `draft-ietf-mailmaint-oauth-public` lets it go (`docs/mail-oauth.md` rule 10).
///
/// A grant reaches a DAV server when its sign-in asked for that use's scope and the server's
/// host is one its issuer's tokens are for (§3.6), and the token then goes as Bearer under a
/// [`CredentialHosts`] policy that applies the same host rule to every redirect discovery
/// follows. Anything else is [`AccountError::DavNeedsOwnSignIn`], before a request is made.
pub(crate) async fn dav_credentials(
    account: &AccountConfig,
    endpoint: DavEndpoint<'_>,
    dav_use: DavUse,
    tokens: ImapTokens<'_>,
) -> Result<DavAuth, AccountError> {
    if let Some(grant) = &account.oauth {
        // The host as the transport's own policy check reads it, so an IPv6 literal, an
        // internationalised name or a userinfo part is judged as the request will go. The token
        // travels over TLS only; a loopback server (a local test server) is the one exception.
        let url =
            reqwest::Url::parse(&crate::setup::normalize_caldav_base_url(endpoint.base_url)).ok();
        let host = url
            .as_ref()
            .and_then(|url| url.host_str().map(str::to_ascii_lowercase));
        let over_tls = url.as_ref().is_some_and(|url| {
            url.scheme() == "https"
                || host
                    .as_deref()
                    .is_some_and(|host| matches!(host, "localhost" | "127.0.0.1" | "[::1]"))
        });
        if !over_tls
            || !mailcal_oauth::requests_dav_use(&grant.scopes, dav_use)
            || !host.is_some_and(|host| grant.token_may_reach(&host))
        {
            return Err(AccountError::DavNeedsOwnSignIn(match dav_use {
                DavUse::Calendars => "calendar",
                DavUse::Contacts => "contacts",
            }));
        }
        let tokens = tokens.ok_or(AccountError::MissingCredential(
            imap_credentials::NO_TOKEN_SOURCE,
        ))?;
        return Ok(DavAuth {
            credentials: Credentials::Bearer(tokens.access_token().await?),
            hosts: Some(Arc::new(IssuerHosts {
                issuer: grant.issuer_identifier(),
                protected_resources: grant.protected_resources.clone(),
            })),
        });
    }
    let password = endpoint
        .password
        .ok_or_else(|| AccountError::CalDavDiscovery("no DAV credential stored".to_owned()))?;
    Ok(DavAuth {
        credentials: Credentials::Basic {
            username: endpoint.username.to_owned(),
            password: password.expose().to_owned(),
        },
        hosts: None,
    })
}

/// The hosts a grant's tokens are for, as the DAV transport asks about each redirect.
struct IssuerHosts {
    issuer: String,
    protected_resources: Vec<String>,
}

impl CredentialHosts for IssuerHosts {
    fn permits(&self, host: &str) -> bool {
        mailcal_oauth::token_may_reach(&self.issuer, &self.protected_resources, host)
    }
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
