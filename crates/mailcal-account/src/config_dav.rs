//! The DAV half of a standards account: its CardDAV section, the endpoint its calendar and its
//! contacts each connect to, and the id of an account that has no mailbox.
//!
//! Split from `config.rs`, which holds the account as a whole; this is the part a calendar and
//! contacts account without mail is made of.

use engine_core::ids::{AccountId, IdError};
use provider_caldav::Credentials;
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

/// The credential a DAV endpoint of `account` presents: the account's grant as a bearer token on
/// an OAuth account, else Basic with the endpoint's own login.
pub(crate) async fn dav_credentials(
    account: &AccountConfig,
    endpoint: DavEndpoint<'_>,
    tokens: ImapTokens<'_>,
) -> Result<Credentials, AccountError> {
    if account.is_oauth() {
        let tokens = tokens.ok_or(AccountError::MissingCredential(
            imap_credentials::NO_TOKEN_SOURCE,
        ))?;
        return Ok(Credentials::Bearer(tokens.access_token().await?));
    }
    let password = endpoint
        .password
        .ok_or_else(|| AccountError::CalDavDiscovery("no DAV credential stored".to_owned()))?;
    Ok(Credentials::Basic {
        username: endpoint.username.to_owned(),
        password: password.expose().to_owned(),
    })
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
