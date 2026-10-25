//! Where an account's calendar and contacts connects start: the principal each named last time.
//!
//! RFC 6764 §6 asks a client to cache "the service details that were successfully used (user
//! identity, principal-URL with full scheme/host/port details)" and reuse them, and to repeat
//! the bootstrap when a later connection fails; RFC 6352 §9.3 calls the principal URL "the key
//! piece of information" a CardDAV client needs. The principal is named by the account's own
//! server, so it outlives a home on a host such as iCloud's `pNN`, which is never stored.
//!
//! These are a cache, not what setup stored: the configured server stays in the account's
//! config (an account without mail takes its id from it), and these sit beside it in a file of
//! their own, which holds nothing secret. A use whose stored principal is gone connects from the
//! configured server again, and a change to the account's servers forgets them.

use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

/// The principal each DAV use of one account named on its last successful connect.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DavStarts {
    /// The calendar's principal URL, absolute.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calendar: Option<String>,
    /// The address book's principal URL, absolute. A different server from the calendar's on
    /// some providers (`contacts.icloud.com` beside `caldav.icloud.com`), so kept apart even when
    /// both connect through one configured endpoint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contacts: Option<String>,
}

impl DavStarts {
    /// Whether neither use has a principal stored.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.calendar.is_none() && self.contacts.is_none()
    }

    /// `self` with each principal `learned` names in place of the stored one; a use `learned`
    /// knows nothing about keeps what it had.
    #[must_use]
    pub fn updated_with(&self, learned: &Self) -> Self {
        Self {
            calendar: learned.calendar.clone().or_else(|| self.calendar.clone()),
            contacts: learned.contacts.clone().or_else(|| self.contacts.clone()),
        }
    }
}

/// The file's name, in the app data directory.
const FILE_NAME: &str = "dav-principals.toml";

/// The file of every account's [`DavStarts`], keyed by account id.
#[derive(Debug, Clone)]
pub struct DavStartsFile {
    path: PathBuf,
}

impl DavStartsFile {
    /// The file inside the app data directory `base`.
    #[must_use]
    pub fn in_dir(base: impl AsRef<Path>) -> Self {
        Self {
            path: base.as_ref().join(FILE_NAME),
        }
    }

    /// Every account's starts. A missing or unreadable file is empty: a cache that cannot be read
    /// only costs the next connect its shortcut, since it starts from the configured server.
    #[must_use]
    pub fn load(&self) -> BTreeMap<String, DavStarts> {
        fs::read_to_string(&self.path)
            .ok()
            .and_then(|body| toml::from_str(&body).ok())
            .unwrap_or_default()
    }

    /// Stores `starts` for `account`, or forgets the account's entry when `starts` is empty.
    ///
    /// # Errors
    ///
    /// Returns an [`io::Error`] when the file cannot be written.
    pub fn store(&self, account: &str, starts: &DavStarts) -> io::Result<()> {
        let mut all = self.load();
        if starts.is_empty() {
            if all.remove(account).is_none() {
                return Ok(());
            }
        } else {
            all.insert(account.to_owned(), starts.clone());
        }
        let body =
            toml::to_string(&all).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&self.path, body)
    }
}

#[cfg(test)]
#[path = "dav_starts_tests.rs"]
mod tests;
