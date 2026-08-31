// SPDX-FileCopyrightText: 2026 Allodia
// SPDX-License-Identifier: LicenseRef-Allodia-1.0

//! Keeping the list of mail accounts the same on every device.
//!
//! **What travels, and what does not.** For each account: the address, the server names and ports,
//! the user name, the connection settings. Never a password and never a token for the person's
//! provider: those stay in each device's own keystore and are entered once per device, which is
//! why an account arriving from here is an *offer* rather than a working account.
//!
//! **The identity is the server's, not the settings'.** A record is named by an opaque id minted on
//! the first store. The obvious alternative, deriving it from the account's own settings, forks the
//! moment two devices disagree about a hostname, and they do, because autodetect races strategies
//! and a different one can win on each ([`docs/account-autodetect.md`]). With an opaque id,
//! correcting a hostname is an edit to one record rather than the death of one and the birth of
//! another.
//!
//! **Every write names the version it read.** The server refuses any other with `409` and hands
//! back what it holds, so a device that has been offline cannot overwrite an edit made elsewhere,
//! and cannot revive an account it never learned was deleted.
//!
//! [`docs/account-autodetect.md`]: https://allodia.eu/docs/mail-calendar

use std::fmt::Write as _;

use serde::{Deserialize, Serialize};

use crate::{API_BASE_PATH, AccountService, Error, Method, Request, Transport};

/// How a JMAP account proves who it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JmapAuth {
    /// A discovered OAuth flow.
    OAuth,
    /// A password or API token, held in the device's keystore.
    Secret,
}

/// How a connection is protected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Security {
    /// TLS from the first byte.
    ImplicitTls,
    /// Upgraded in-band.
    Starttls,
}

/// Where mail is read from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImapEndpoint {
    /// The host name, never an address literal.
    pub host: String,
    /// The port, as configured rather than as defaulted.
    pub port: u16,
    /// How the connection is protected.
    pub security: Security,
    /// The user name to present, which is not always the email address.
    pub username: String,
}

/// Where mail is submitted. Absent for an account that only reads.
///
/// No user name of its own: submission reuses the reading credential, which is what the stored
/// config does too, so a field here would be a second answer to a question with one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SmtpEndpoint {
    /// The host name.
    pub host: String,
    /// The port.
    pub port: u16,
    /// How the connection is protected.
    pub security: Security,
}

/// Where the diary lives, when the account has one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalDavEndpoint {
    /// The collection root.
    pub base_url: String,
    /// The user name to present.
    pub username: String,
    /// Which calendar was chosen, when one was. `None` means "whatever the server offers", which
    /// is a different state from a calendar that has since gone.
    pub calendar: Option<String>,
}

/// One account's settings, in the shape the service stores.
///
/// The four kinds are the four the app configures, and they are not interchangeable: the same
/// address over IMAP and over JMAP is two accounts, not one, which is why `kind` is part of what
/// makes two records the same account rather than a detail inside one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum SyncedConfig {
    /// IMAP, with optional submission and diary.
    Imap {
        /// The address this account is for.
        email: String,
        /// Where mail is read from.
        imap: ImapEndpoint,
        /// Where mail is submitted, when it can be.
        smtp: Option<SmtpEndpoint>,
        /// Where the diary lives, when there is one.
        caldav: Option<CalDavEndpoint>,
    },
    /// JMAP, where one base URL carries everything.
    Jmap {
        /// The address this account is for.
        email: String,
        /// The session resource.
        #[serde(rename = "baseUrl")]
        base_url: String,
        /// How the account proves who it is.
        auth: JmapAuth,
    },
    /// Gmail and Google Calendar, over Google's own API.
    Google {
        /// The address this account is for. Everything else is derived.
        email: String,
    },
    /// Microsoft 365, over Graph.
    Microsoft {
        /// The address this account is for. Everything else is derived.
        email: String,
    },
}

impl SyncedConfig {
    /// The address this account is for.
    #[must_use]
    pub fn email(&self) -> &str {
        match self {
            Self::Imap { email, .. }
            | Self::Jmap { email, .. }
            | Self::Google { email }
            | Self::Microsoft { email } => email,
        }
    }

    /// The kind's wire label.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Imap { .. } => "imap",
            Self::Jmap { .. } => "jmap",
            Self::Google { .. } => "google",
            Self::Microsoft { .. } => "microsoft",
        }
    }

    /// Whether two records describe the same mailbox.
    ///
    /// Address **and** kind, because the same address over two protocols is two accounts, and
    /// deliberately not the host, which is the field that legitimately differs between devices on
    /// different networks and the one autodetect can race to different answers for.
    #[must_use]
    pub fn is_same_account_as(&self, other: &Self) -> bool {
        self.kind() == other.kind() && self.email().eq_ignore_ascii_case(other.email())
    }
}

/// A stored account, as the service hands it back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncedAccount {
    /// The sync id. Opaque, and the device keeps it beside the account.
    pub id: String,
    /// Bumped by every write, deletions included. The next write has to name it.
    pub version: u64,
    /// The settings themselves.
    pub config: SyncedConfig,
    /// When the record last changed, as the server wrote it (RFC 3339). Display only.
    pub updated_at: String,
}

/// An account the person removed on some device.
///
/// It comes back as an id rather than as settings, so a device can ask its owner before removing a
/// mailbox they may still want locally: a removal is a local decision everywhere it lands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeletedAccount {
    /// Which record went.
    pub id: String,
    /// The version the deletion itself wrote.
    pub version: u64,
    /// When it went (RFC 3339).
    pub deleted_at: String,
}

/// Everything the service holds for this person, or everything that changed since a moment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountList {
    /// The accounts themselves.
    pub accounts: Vec<SyncedAccount>,
    /// What has been removed.
    pub deleted: Vec<DeletedAccount>,
    /// The moment this answer describes, to pass back as `since`.
    pub synced_at: String,
}

/// What the server holds instead of what the caller expected.
///
/// Two shapes, because a record can be gone as well as moved, and replaying a create whose record
/// was since deleted answers with the tombstone rather than storing it again, which would be the
/// resurrection bug wearing a different hat.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ConflictWith {
    /// The record, at the version the server holds.
    Record(Box<SyncedAccount>),
    /// A tombstone: the account is gone, and this is when.
    Tombstone(Box<DeletedAccount>),
}

/// The `409` body: what the server holds now.
#[derive(Deserialize)]
struct ConflictBody {
    data: ConflictData,
}

#[derive(Deserialize)]
struct ConflictData {
    current: Option<ConflictWith>,
}

impl AccountService {
    /// Every account this person syncs, or (with `since`) what changed after it.
    ///
    /// `since` is an optimisation and not the source of truth: a row committed during an earlier
    /// read can carry a timestamp a delta would step over, so a full pull has to happen now and
    /// then regardless. Nothing here decides how often; that is the caller's.
    ///
    /// # Errors
    /// [`Error::Unauthorized`] when the token needs refreshing; [`Error::Transport`] when the
    /// request never arrived; [`Error::Malformed`] when the answer cannot be read.
    pub fn list_accounts(
        &self,
        transport: &dyn Transport,
        access_token: &str,
        since: Option<&str>,
    ) -> Result<AccountList, Error> {
        let mut url = format!("{}{API_BASE_PATH}/accounts", self.base_url());
        if let Some(since) = since {
            url.push_str("?since=");
            url.push_str(&encode_query(since));
        }
        let body = send(transport, &Request::get(url, access_token))?;
        serde_json::from_str(&body).map_err(|error| Error::Malformed(error.to_string()))
    }

    /// Store an account the service has never seen, and learn the id it minted.
    ///
    /// `idempotency_key` is the caller's own, held across retries of **this** create: a response
    /// that never arrived cannot be told from a new account, so without it a retry on a flaky
    /// connection leaves a second record behind.
    ///
    /// # Errors
    /// As [`AccountService::list_accounts`].
    pub fn create_account(
        &self,
        transport: &dyn Transport,
        access_token: &str,
        config: &SyncedConfig,
        idempotency_key: &str,
    ) -> Result<SyncedAccount, Error> {
        let body = send(
            transport,
            &Request {
                url: format!("{}{API_BASE_PATH}/accounts", self.base_url()),
                bearer: access_token.to_owned(),
                method: Method::Post,
                body: Some(wrap_config(config)?),
                idempotency_key: Some(idempotency_key.to_owned()),
            },
        )?;
        serde_json::from_str(&body).map_err(|error| Error::Malformed(error.to_string()))
    }

    /// Replace a record the caller can already name.
    ///
    /// # Errors
    /// [`Error::Conflict`] when `version` is not the one the server holds, carrying what it holds
    /// instead; otherwise as [`AccountService::list_accounts`].
    pub fn update_account(
        &self,
        transport: &dyn Transport,
        access_token: &str,
        id: &str,
        version: u64,
        config: &SyncedConfig,
    ) -> Result<SyncedAccount, Error> {
        let payload = serde_json::json!({ "version": version, "config": config });
        let sent = send(
            transport,
            &Request {
                url: format!("{}{API_BASE_PATH}/accounts/{id}", self.base_url()),
                bearer: access_token.to_owned(),
                method: Method::Put,
                body: Some(payload.to_string()),
                idempotency_key: None,
            },
        );
        match sent {
            Ok(body) => {
                serde_json::from_str(&body).map_err(|error| Error::Malformed(error.to_string()))
            }
            // A write whose response was lost re-sends the same base version and is refused, and
            // what the refusal carries is the write it was making. That is not a disagreement to
            // put in front of anybody: the settings the caller wanted stored are stored, and the
            // only thing missing was the receipt.
            Err(Error::Conflict(Some(ConflictWith::Record(current))))
                if current.config == *config =>
            {
                Ok(*current)
            }
            Err(other) => Err(other),
        }
    }

    /// Mark an account removed, so the person's other devices learn it went.
    ///
    /// A `404` is success: the caller wanted the record gone and it is gone, and treating "already
    /// absent" as a failure would leave a device retrying something that can never change.
    ///
    /// # Errors
    /// [`Error::Conflict`] when `version` is not the one the server holds: the person removed
    /// something that has since moved. Otherwise as [`AccountService::list_accounts`].
    pub fn delete_account(
        &self,
        transport: &dyn Transport,
        access_token: &str,
        id: &str,
        version: u64,
    ) -> Result<(), Error> {
        let request = Request {
            url: format!(
                "{}{API_BASE_PATH}/accounts/{id}?version={version}",
                self.base_url()
            ),
            bearer: access_token.to_owned(),
            method: Method::Delete,
            body: None,
            idempotency_key: None,
        };
        match send(transport, &request) {
            // Already gone is what the caller wanted, so it is not a second case. The service
            // answers a delete of an unknown record with a `200` and no tombstone; the `404` arm
            // is kept for a deployment that has not caught up.
            // The third is refusal because the record is already a tombstone: the account is gone,
            // which is what was asked for. Reporting that as a conflict would have a device ask
            // its owner about a removal that has already happened everywhere.
            Ok(_)
            | Err(
                Error::Unexpected { status: 404 }
                | Error::Conflict(Some(ConflictWith::Tombstone(_))),
            ) => Ok(()),
            Err(other) => Err(other),
        }
    }
}

/// Send one request and turn its status into this crate's errors.
fn send(transport: &dyn Transport, request: &Request) -> Result<String, Error> {
    let response = transport.send(request).map_err(Error::Transport)?;
    match response.status {
        200..=299 => Ok(response.body),
        401 | 403 => Err(Error::Unauthorized),
        409 => Err(conflict(&response.body)),
        status => Err(Error::Unexpected { status }),
    }
}

/// Read a `409` body, falling back to a bare conflict when it cannot be parsed.
///
/// A conflict whose payload is unreadable is still a conflict: reporting it as malformed would send
/// the caller down the "the service is broken" path when the answer is "re-read and try again".
fn conflict(body: &str) -> Error {
    let current = serde_json::from_str::<ConflictBody>(body)
        .ok()
        .and_then(|parsed| parsed.data.current);
    Error::Conflict(current)
}

/// The request body both writes share, minus the version.
fn wrap_config(config: &SyncedConfig) -> Result<String, Error> {
    serde_json::to_string(&serde_json::json!({ "config": config }))
        .map_err(|error| Error::Malformed(error.to_string()))
}

/// Percent-encode a query value.
///
/// Only the characters a timestamp can contain need escaping, and `+` is the one that matters: it
/// means a space to a form decoder, so a `+02:00` offset arrives as a space and the delta silently
/// covers the wrong window.
fn encode_query(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            other => {
                let _ = write!(encoded, "%{other:02X}");
            }
        }
    }
    encoded
}

#[cfg(test)]
#[path = "accounts_tests.rs"]
mod accounts_tests;
