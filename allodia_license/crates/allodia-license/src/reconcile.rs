// SPDX-FileCopyrightText: 2026 Allodia
// SPDX-License-Identifier: LicenseRef-Allodia-1.0

//! Deciding what a device should do about the difference between its accounts and the service's.
//!
//! Pure: no clock, no network, no storage. Everything that decides an outcome is an argument, so
//! every rule below is a test rather than something observed once on a device.
//!
//! **Nothing here applies anything.** Each decision is a thing to offer, and applying it is the
//! person's: an account arriving from another device cannot work until they enter its password
//! anyway, so auto-applying would buy nothing and cost the ability to say no.
//!
//! **Three states, not two.** A device remembers the version it last synced *and* what the config
//! looked like then. Without that second half, "the server is newer" cannot be told from "we both
//! changed", and an update would silently overwrite an edit made here.

use crate::accounts::{AccountList, SyncedAccount, SyncedConfig};

/// What this device remembers about an account it has synced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncState {
    /// The service's id for this account.
    pub id: String,
    /// The version this device last read. Every write names it.
    pub version: u64,
    /// What the config looked like at that version, as a fingerprint.
    ///
    /// The base of a three-way comparison. Comparing the config against the *server's* would only
    /// ever say "they differ", never which side moved.
    pub fingerprint: String,
    /// The person told this device to keep its own settings.
    ///
    /// Fully local from then on: it neither pulls nor pushes. Pushing would go on feeding the
    /// other devices a hostname that is right only on this network, which is the noise detaching
    /// was meant to end.
    pub detached: bool,
}

/// One of this device's accounts, as the reconciler sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalAccount {
    /// The core's own id for the account.
    pub account_id: String,
    /// Its settings, projected into the shape the service holds.
    pub config: SyncedConfig,
    /// What this device remembers about syncing it. `None` for one the service has never seen.
    pub sync: Option<SyncState>,
}

/// Something for the device to do, or to ask about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// The service has never seen this account. Upload it and keep the id that comes back.
    Upload {
        /// Which local account.
        account_id: String,
    },
    /// This device changed an account the service has not. Push it.
    Push {
        /// Which local account.
        account_id: String,
        /// The record to write.
        id: String,
        /// The version to name.
        version: u64,
    },
    /// The service has a newer version and this device has not touched it. Offer to apply.
    UpdateAvailable {
        /// Which local account.
        account_id: String,
        /// What the service holds.
        current: Box<SyncedAccount>,
    },
    /// Both sides changed. The person picks, and keeping theirs is what detaching is for.
    Conflict {
        /// Which local account.
        account_id: String,
        /// What the service holds.
        current: Box<SyncedAccount>,
    },
    /// An account this device has not got, from one of the person's others.
    Offer {
        /// What the service holds.
        current: Box<SyncedAccount>,
    },
    /// This device has the account already, under a record it never learned about.
    ///
    /// Adopt the service's id rather than uploading a second record for the same mailbox, which
    /// is the duplicate that would otherwise accumulate when two devices set up the same account
    /// independently.
    Adopt {
        /// Which local account.
        account_id: String,
        /// The record it turns out to be.
        current: Box<SyncedAccount>,
    },
    /// Removed on another device. Ask before removing it here.
    RemovedElsewhere {
        /// Which local account.
        account_id: String,
    },
}

/// Work out what to do about the difference between `local` and what the service holds.
///
/// Order matters in one place: a removal is applied before an offer, so an account someone deleted
/// elsewhere is not offered back to them in the same pass.
#[must_use]
pub fn reconcile(local: &[LocalAccount], remote: &AccountList) -> Vec<Decision> {
    let mut decisions = Vec::new();
    let mut claimed: Vec<&str> = Vec::new();

    for account in local {
        // A detached account is invisible in both directions. Its id stays claimed so the record
        // is not offered back as if this device had never seen it.
        if let Some(state) = &account.sync
            && state.detached
        {
            claimed.push(state.id.as_str());
            continue;
        }
        match &account.sync {
            Some(state) => {
                claimed.push(state.id.as_str());
                decisions.extend(known(account, state, remote));
            }
            None => decisions.push(unknown(account, remote, &mut claimed)),
        }
    }

    // Anything the service holds that no local account speaks for.
    let removed: Vec<&str> = remote.deleted.iter().map(|gone| gone.id.as_str()).collect();
    for record in &remote.accounts {
        if claimed.contains(&record.id.as_str()) || removed.contains(&record.id.as_str()) {
            continue;
        }
        decisions.push(Decision::Offer {
            current: Box::new(record.clone()),
        });
    }
    decisions
}

/// An account this device has synced before.
fn known(account: &LocalAccount, state: &SyncState, remote: &AccountList) -> Option<Decision> {
    if remote.deleted.iter().any(|gone| gone.id == state.id) {
        return Some(Decision::RemovedElsewhere {
            account_id: account.account_id.clone(),
        });
    }
    let Some(record) = remote.accounts.iter().find(|held| held.id == state.id) else {
        // Not in this answer at all, which a `since` delta says nothing about: a record that did
        // not change is simply absent. Silence is not a deletion.
        return locally_changed(account, state).then(|| Decision::Push {
            account_id: account.account_id.clone(),
            id: state.id.clone(),
            version: state.version,
        });
    };
    let here = locally_changed(account, state);
    let there = record.version != state.version;
    match (here, there) {
        (true, true) => Some(Decision::Conflict {
            account_id: account.account_id.clone(),
            current: Box::new(record.clone()),
        }),
        (false, true) => Some(Decision::UpdateAvailable {
            account_id: account.account_id.clone(),
            current: Box::new(record.clone()),
        }),
        (true, false) => Some(Decision::Push {
            account_id: account.account_id.clone(),
            id: state.id.clone(),
            version: state.version,
        }),
        (false, false) => None,
    }
}

/// An account the service has never been told about by *this* device.
///
/// It may still be one the service knows: two devices setting the same mailbox up independently is
/// ordinary, and uploading a second record for it is the duplicate the opaque id was chosen to
/// avoid. Matching is address and kind, never host.
fn unknown<'a>(
    account: &'a LocalAccount,
    remote: &'a AccountList,
    claimed: &mut Vec<&'a str>,
) -> Decision {
    let existing = remote
        .accounts
        .iter()
        .find(|record| record.config.is_same_account_as(&account.config));
    match existing {
        Some(record) => {
            claimed.push(record.id.as_str());
            Decision::Adopt {
                account_id: account.account_id.clone(),
                current: Box::new(record.clone()),
            }
        }
        None => Decision::Upload {
            account_id: account.account_id.clone(),
        },
    }
}

/// Whether this device has changed the account since it last synced.
fn locally_changed(account: &LocalAccount, state: &SyncState) -> bool {
    fingerprint(&account.config) != state.fingerprint
}

/// A stable summary of a config, for telling "this device changed it" from "it did not".
///
/// Serialized rather than hashed: it is compared, never published, and a form a person can read is
/// worth more in a log or a bug report than eight bytes of digest. Serde writes a struct's fields
/// in declaration order, so the same config always produces the same string.
#[must_use]
pub fn fingerprint(config: &SyncedConfig) -> String {
    serde_json::to_string(config).unwrap_or_default()
}

#[cfg(test)]
#[path = "reconcile_tests.rs"]
mod reconcile_tests;
