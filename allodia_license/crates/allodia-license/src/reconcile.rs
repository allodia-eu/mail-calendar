// SPDX-FileCopyrightText: 2026 Allodia
// SPDX-License-Identifier: LicenseRef-Allodia-1.0

//! Deciding what a device should do about the difference between what it holds and what the
//! service holds: its mail accounts, and its writing styles.
//!
//! Pure: no clock, no network, no storage. Everything that decides an outcome is an argument, so
//! every rule below is a test rather than something observed once on a device.
//!
//! **Nothing here applies anything.** A decision says what differs, and what to do about it is the
//! caller's. An account arriving from another device is an offer: it cannot work until the person
//! enters its password anyway, so auto-applying would buy nothing and cost the ability to say no. A
//! writing style needs nothing of the kind, and its caller applies what arrives.
//!
//! **Three states, not two.** A device remembers the version it last synced *and* what the item
//! looked like then. Without that second half, "the server is newer" cannot be told from "we both
//! changed", and an update would silently overwrite an edit made here.
//!
//! One set of rules for every collection: [`Verdict`] is what they share, and [`Decision`] is the
//! same thing in the account list's own words.

use serde::Serialize;

use crate::{
    accounts::{AccountList, SyncedAccount, SyncedConfig},
    collection::{SyncedRecord, Tombstone},
};

/// What the reconciler needs of what travels: a stable form to compare, and a way to recognise
/// the record for an item this device set up on its own.
pub trait Fingerprint: Serialize {
    /// Whether this item, which this device has never synced, is the one `other` describes.
    fn is_same_as(&self, other: &Self) -> bool;
}

/// What this device remembers about an item it has synced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncState {
    /// The service's id for this item.
    pub id: String,
    /// The version this device last read. Every write names it.
    pub version: u64,
    /// What the item looked like at that version, as a fingerprint.
    ///
    /// The base of a three-way comparison. Comparing the item against the *server's* would only
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

/// Something for the device to do, or to ask about, for one account.
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

/// Something for the device to do about one item, in the words every collection shares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict<R> {
    /// The service has never seen this item. Upload it and keep the id that comes back.
    Upload {
        /// This device's id for it.
        local_id: String,
    },
    /// This device changed an item the service has not. Push it.
    Push {
        /// This device's id for it.
        local_id: String,
        /// The record to write.
        id: String,
        /// The version to name.
        version: u64,
    },
    /// The service has a newer version and this device has not touched it.
    UpdateAvailable {
        /// This device's id for it.
        local_id: String,
        /// What the service holds.
        current: Box<R>,
    },
    /// Both sides changed, so neither is right by itself.
    Conflict {
        /// This device's id for it.
        local_id: String,
        /// What the service holds.
        current: Box<R>,
    },
    /// An item this device has not got, from one of the person's others.
    Offer {
        /// What the service holds.
        current: Box<R>,
    },
    /// This device has the item already, under a record it never learned about. Adopt the
    /// record rather than uploading a second one.
    Adopt {
        /// This device's id for it.
        local_id: String,
        /// The record it turns out to be.
        current: Box<R>,
    },
    /// Removed on another device.
    RemovedElsewhere {
        /// This device's id for it.
        local_id: String,
    },
}

impl From<Verdict<SyncedAccount>> for Decision {
    fn from(verdict: Verdict<SyncedAccount>) -> Self {
        match verdict {
            Verdict::Upload { local_id } => Self::Upload {
                account_id: local_id,
            },
            Verdict::Push {
                local_id,
                id,
                version,
            } => Self::Push {
                account_id: local_id,
                id,
                version,
            },
            Verdict::UpdateAvailable { local_id, current } => Self::UpdateAvailable {
                account_id: local_id,
                current,
            },
            Verdict::Conflict { local_id, current } => Self::Conflict {
                account_id: local_id,
                current,
            },
            Verdict::Offer { current } => Self::Offer { current },
            Verdict::Adopt { local_id, current } => Self::Adopt {
                account_id: local_id,
                current,
            },
            Verdict::RemovedElsewhere { local_id } => Self::RemovedElsewhere {
                account_id: local_id,
            },
        }
    }
}

/// Work out what to do about the difference between `local` and what the service holds.
///
/// Order matters in one place: a removal is applied before an offer, so an account someone deleted
/// elsewhere is not offered back to them in the same pass.
#[must_use]
pub fn reconcile(local: &[LocalAccount], remote: &AccountList) -> Vec<Decision> {
    let mine = local.iter().map(|account| Mine {
        local_id: &account.account_id,
        payload: &account.config,
        sync: account.sync.as_ref(),
    });
    decide(mine, &remote.accounts, &remote.deleted)
        .into_iter()
        .map(Decision::from)
        .collect()
}

/// One item on this device, as the reconciler reads it.
pub(crate) struct Mine<'a, T> {
    pub(crate) local_id: &'a str,
    pub(crate) payload: &'a T,
    pub(crate) sync: Option<&'a SyncState>,
}

/// The rules, for any collection.
pub(crate) fn decide<'a, R: SyncedRecord>(
    local: impl IntoIterator<Item = Mine<'a, R::Payload>>,
    records: &[R],
    deleted: &[Tombstone],
) -> Vec<Verdict<R>>
where
    R::Payload: 'a,
{
    let mut verdicts = Vec::new();
    let mut claimed: Vec<String> = Vec::new();

    for item in local {
        // A detached item is invisible in both directions. Its id stays claimed so the record is
        // not offered back as if this device had never seen it.
        if let Some(state) = item.sync
            && state.detached
        {
            claimed.push(state.id.clone());
            continue;
        }
        match item.sync {
            Some(state) => {
                claimed.push(state.id.clone());
                verdicts.extend(known(&item, state, records, deleted));
            }
            None => verdicts.push(unknown(&item, records, &mut claimed)),
        }
    }

    // Anything the service holds that no local item speaks for.
    for record in records {
        let id = record.id();
        if claimed.iter().any(|mine| mine == id) || deleted.iter().any(|gone| gone.id == id) {
            continue;
        }
        verdicts.push(Verdict::Offer {
            current: Box::new(record.clone()),
        });
    }
    verdicts
}

/// An item this device has synced before.
fn known<R: SyncedRecord>(
    item: &Mine<'_, R::Payload>,
    state: &SyncState,
    records: &[R],
    deleted: &[Tombstone],
) -> Option<Verdict<R>> {
    let local_id = item.local_id.to_owned();
    if deleted.iter().any(|gone| gone.id == state.id) {
        return Some(Verdict::RemovedElsewhere { local_id });
    }
    let here = fingerprint(item.payload) != state.fingerprint;
    let Some(record) = records.iter().find(|held| held.id() == state.id) else {
        // Not in this answer at all, which a `since` delta says nothing about: a record that did
        // not change is simply absent. Silence is not a deletion.
        return here.then(|| Verdict::Push {
            local_id,
            id: state.id.clone(),
            version: state.version,
        });
    };
    let there = record.version() != state.version;
    match (here, there) {
        (true, true) => Some(Verdict::Conflict {
            local_id,
            current: Box::new(record.clone()),
        }),
        (false, true) => Some(Verdict::UpdateAvailable {
            local_id,
            current: Box::new(record.clone()),
        }),
        (true, false) => Some(Verdict::Push {
            local_id,
            id: state.id.clone(),
            version: state.version,
        }),
        (false, false) => None,
    }
}

/// An item the service has never been told about by *this* device.
///
/// It may still be one the service knows: two devices setting the same mailbox up independently is
/// ordinary, and uploading a second record for it is the duplicate the opaque id was chosen to
/// avoid. What counts as the same is the payload's own rule ([`Fingerprint::is_same_as`]).
fn unknown<R: SyncedRecord>(
    item: &Mine<'_, R::Payload>,
    records: &[R],
    claimed: &mut Vec<String>,
) -> Verdict<R> {
    let local_id = item.local_id.to_owned();
    let existing = records
        .iter()
        .find(|record| item.payload.is_same_as(record.payload()));
    match existing {
        Some(record) => {
            claimed.push(record.id().to_owned());
            Verdict::Adopt {
                local_id,
                current: Box::new(record.clone()),
            }
        }
        None => Verdict::Upload { local_id },
    }
}

/// A stable summary of what travels, for telling "this device changed it" from "it did not".
///
/// Serialized rather than hashed: it is compared, never published, and a form a person can read is
/// worth more in a bug report than eight bytes of digest. Serde writes a struct's fields in
/// declaration order, so the same value always produces the same string.
#[must_use]
pub fn fingerprint<T: Fingerprint>(payload: &T) -> String {
    serde_json::to_string(payload).unwrap_or_default()
}

#[cfg(test)]
#[path = "reconcile_tests.rs"]
mod reconcile_tests;
