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

use serde::{Deserialize, Serialize};

use crate::{
    AccountService, Error, Transport,
    collection::{ConflictWith, SyncedCollection, SyncedRecord, Tombstone},
    reconcile::Fingerprint,
};

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

impl Fingerprint for SyncedConfig {
    fn is_same_as(&self, other: &Self) -> bool {
        self.is_same_account_as(other)
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

impl SyncedRecord for SyncedAccount {
    type List = AccountList;
    type Payload = SyncedConfig;

    const PATH: &'static str = "accounts";
    const PAYLOAD: &'static str = "config";

    fn id(&self) -> &str {
        &self.id
    }

    fn version(&self) -> u64 {
        self.version
    }

    fn payload(&self) -> &SyncedConfig {
        &self.config
    }

    fn in_conflict(with: &ConflictWith) -> Option<&Self> {
        match with {
            ConflictWith::Record(current) => Some(current),
            _ => None,
        }
    }
}

/// An account the person removed on some device.
///
/// It comes back as an id rather than as settings, so a device can ask its owner before removing a
/// mailbox they may still want locally: a removal is a local decision everywhere it lands.
pub type DeletedAccount = Tombstone;

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
        SyncedCollection::<SyncedAccount>::new(self).list(transport, access_token, since)
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
        SyncedCollection::<SyncedAccount>::new(self).create(
            transport,
            access_token,
            config,
            idempotency_key,
        )
    }

    /// Replace a record the caller can already name.
    ///
    /// A write whose response was lost, re-sent and refused with the very settings it was
    /// writing, is success rather than a conflict.
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
        SyncedCollection::<SyncedAccount>::new(self).update(
            transport,
            access_token,
            id,
            version,
            config,
        )
    }

    /// Mark an account removed, so the person's other devices learn it went.
    ///
    /// A `404` is success: the caller wanted the record gone and it is gone, and treating "already
    /// absent" as a failure would leave a device retrying something that can never change. So is a
    /// refusal because the record is already a tombstone: reporting that as a conflict would have
    /// a device ask its owner about a removal that has already happened everywhere.
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
        SyncedCollection::<SyncedAccount>::new(self).delete(transport, access_token, id, version)
    }
}

#[cfg(test)]
#[path = "accounts_tests.rs"]
mod accounts_tests;
