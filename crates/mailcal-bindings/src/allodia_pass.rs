//! One pass over the difference between this device's accounts and the person's other devices'.
//!
//! **Deciding is `allodia-license`'s, applying is here.** `reconcile` is a pure function with no
//! clock, no network and no storage, so every rule it holds is a test; this file is the part that
//! cannot be; it writes to the service and to the bookkeeping, in an order chosen so that a pass
//! interrupted anywhere leaves a device that reconciles correctly at the next one.
//!
//! **What is applied without asking, and what is not.** Three decisions are this device's own
//! settings going out: an account the service has never seen, one this device changed, and one it
//! turns out the service already holds. None of them needs a person, and holding them back would
//! mean a device that syncs nothing until somebody opens a screen. Everything else: an account
//! arriving, one that moved elsewhere, one removed elsewhere; reaches a person, because none of
//! them can be applied without a password this device does not have, or a choice only its owner
//! can make.
//!
//! **A failure applies to one account, not to the pass.** One record the service refuses leaves
//! the others to be written; the exception is a refused token, which every later call would fail
//! on identically, and which is reported once rather than five times.

use std::collections::BTreeMap;

use allodia_license::{
    AccountList, AccountService, ConflictWith, Decision, Error, LocalAccount, SyncState,
    SyncedAccount, SyncedConfig, Transport, fingerprint, reconcile, to_synced,
};

use crate::{
    AllodiaGrantHealth, MailcalApp, MailcalError,
    allodia_sync::{AllodiaAccountChange, AllodiaAccountOffer, AllodiaSyncReport},
    sync_state::{StoredSyncState, SyncBookkeeping},
};

/// Everything one pass needs, so the part that talks to the service can be exercised without one.
pub(crate) struct Pass<'a> {
    /// The deployment this build talks to.
    pub(crate) service: &'a AccountService,
    /// How its requests are made.
    pub(crate) transport: &'a dyn Transport,
    /// A live access token for it.
    pub(crate) token: &'a str,
    /// What this device remembers about what it has synced.
    pub(crate) bookkeeping: &'a SyncBookkeeping,
}

/// This device's accounts, as the reconciler sees them.
///
/// An account whose config cannot be represented in the shape the service holds is **left out
/// rather than refused**: it goes on working here, it is simply not synced. The alternative is a
/// person whose whole account list stops syncing because one account has a split TLS name.
///
/// **An excluded account is handled two ways, and both are needed.** One the service already holds
/// is passed on as *detached*, which keeps its record claimed; dropping it here would leave the
/// record unspoken for and offer the account straight back. One the service has never seen is left
/// out entirely: there is no record to claim, and including it would upload the very account the
/// person asked this device to keep to itself.
pub(crate) fn local_accounts(
    configs: &BTreeMap<String, String>,
    bookkeeping: &SyncBookkeeping,
) -> Vec<LocalAccount> {
    let mut local = Vec::with_capacity(configs.len());
    for (account_id, stored) in configs {
        let handle = mailcal_account::account_log_handle(account_id);
        let entry = bookkeeping.get(account_id);
        let excluded = bookkeeping.is_excluded(account_id);
        if excluded && entry.is_none() {
            log::info!("allodia: [{handle}] kept to this device, nothing to send");
            continue;
        }
        match to_synced(stored) {
            Ok(config) => local.push(LocalAccount {
                account_id: account_id.clone(),
                config,
                sync: entry.map(|entry| into_sync_state(entry, excluded)),
            }),
            Err(reason) => {
                log::info!("allodia: [{handle}] this account is not synced; {reason}");
            }
        }
    }
    local
}

/// The bookkeeping's entry, as the reconciler's input.
fn into_sync_state(stored: StoredSyncState, excluded: bool) -> SyncState {
    SyncState {
        id: stored.id,
        version: stored.version,
        fingerprint: stored.fingerprint,
        detached: excluded,
    }
}

impl Pass<'_> {
    /// Read what the service holds.
    ///
    /// A whole list every time rather than a delta. It is a handful of records, the delta's `since`
    /// is an optimisation the service's own documentation says is not authoritative, and a pass
    /// that read only changes could not tell an account it has never seen from one that simply did
    /// not move.
    ///
    /// # Errors
    ///
    /// Whatever the service said, **unmapped**. A refusal is evidence about the sign-in and has
    /// to be recorded before it is turned into a message, and this type holds no app to record it
    /// on: so the caller maps it through
    /// [`report_allodia`](MailcalApp::report_allodia) rather than being handed a
    /// `MailcalError` it can no longer classify.
    pub(crate) fn read(&self) -> Result<AccountList, Error> {
        self.service.list_accounts(self.transport, self.token, None)
    }

    /// Work out what to do, do this device's half, and hand back the rest.
    pub(crate) fn apply(&self, local: &[LocalAccount], remote: &AccountList) -> AllodiaSyncReport {
        let by_id: BTreeMap<&str, &SyncedConfig> = local
            .iter()
            .map(|account| (account.account_id.as_str(), &account.config))
            .collect();
        let mut sync_report = AllodiaSyncReport::default();

        for decision in reconcile(local, remote) {
            match decision {
                Decision::Upload { account_id } => {
                    if let Some(config) = by_id.get(account_id.as_str())
                        && self.upload(&account_id, config)
                    {
                        sync_report.sent += 1;
                    }
                }
                Decision::Push {
                    account_id,
                    id,
                    version,
                } => {
                    if let Some(config) = by_id.get(account_id.as_str())
                        && self.push(&account_id, &id, version, config)
                    {
                        sync_report.sent += 1;
                    }
                }
                Decision::Adopt {
                    account_id,
                    current,
                } => self.adopt(&account_id, &current),
                Decision::Offer { current } => sync_report
                    .offers
                    .push(AllodiaAccountOffer::from_record(&current)),
                Decision::UpdateAvailable {
                    account_id,
                    current,
                } => sync_report.changed_elsewhere.push(AllodiaAccountChange {
                    email: current.config.email().to_owned(),
                    account_id,
                    also_changed_here: false,
                }),
                Decision::Conflict {
                    account_id,
                    current,
                } => sync_report.changed_elsewhere.push(AllodiaAccountChange {
                    email: current.config.email().to_owned(),
                    account_id,
                    also_changed_here: true,
                }),
                Decision::RemovedElsewhere { account_id } => {
                    let email = by_id
                        .get(account_id.as_str())
                        .map(|config| config.email().to_owned())
                        .unwrap_or_default();
                    sync_report.removed_elsewhere.push(AllodiaAccountChange {
                        account_id,
                        email,
                        also_changed_here: false,
                    });
                }
            }
        }
        sync_report
    }

    /// Store an account the service has never seen. `true` when it landed.
    fn upload(&self, account_id: &str, config: &SyncedConfig) -> bool {
        let Some(key) = self.create_key(account_id) else {
            return false;
        };
        match self
            .service
            .create_account(self.transport, self.token, config, &key)
        {
            Ok(record) => {
                // Confirmed, so the key has done its work. Keeping it would make the *next* create
                // for this account a replay of this one.
                self.forget_create_key(account_id);
                self.remember(account_id, &record, fingerprint(config))
            }
            // The create landed once and its record has since been deleted. Re-sending would
            // resurrect an account somebody removed, so this pass reports the refusal, and drops
            // the key, because holding a key whose answer can no longer change is how an account
            // gets wedged.
            Err(error @ Error::Conflict(Some(ConflictWith::Tombstone(_)))) => {
                self.forget_create_key(account_id);
                self.complain(account_id, "was removed while it was being sent", &error);
                false
            }
            Err(error) => {
                // The key is kept: this may be a response that never arrived, and the retry has to
                // present the same one or it makes a second account.
                self.complain(account_id, "could not be sent to the service", &error);
                false
            }
        }
    }

    /// The key this create must present: the one a previous attempt left, or a fresh one.
    ///
    /// Written down **before** the request, because the failure it guards against is a response
    /// that never arrives, and a key that only exists in this stack frame is gone by the time the
    /// retry needs it.
    fn create_key(&self, account_id: &str) -> Option<String> {
        if let Some(existing) = self.bookkeeping.pending_create_key(account_id) {
            return Some(existing);
        }
        let key = fresh_create_key(account_id);
        match self
            .bookkeeping
            .set_pending_create_key(account_id, Some(&key))
        {
            Ok(()) => Some(key),
            Err(error) => {
                // Sending without a key that survives is worse than not sending: a retry would
                // then be indistinguishable from a second account.
                log::error!(
                    "allodia: [{}] this account was not sent because its retry key could not be \
                     stored; {error}",
                    mailcal_account::account_log_handle(account_id),
                );
                None
            }
        }
    }

    /// Drop the key of a create that will never be retried.
    fn forget_create_key(&self, account_id: &str) {
        if let Err(error) = self.bookkeeping.set_pending_create_key(account_id, None) {
            log::warn!(
                "allodia: [{}] the finished create's key could not be cleared; {error}",
                mailcal_account::account_log_handle(account_id),
            );
        }
    }

    /// Replace a record this device has changed. `true` when it landed.
    fn push(&self, account_id: &str, id: &str, version: u64, config: &SyncedConfig) -> bool {
        match self
            .service
            .update_account(self.transport, self.token, id, version, config)
        {
            Ok(record) => self.remember(account_id, &record, fingerprint(config)),
            Err(error) => {
                self.complain(account_id, "could not be updated at the service", &error);
                false
            }
        }
    }

    /// Claim the record the service already holds for an account this device set up on its own.
    ///
    /// The base recorded is the **service's** settings rather than this device's, so the next pass
    /// sees a local change and pushes. Two devices that set the same mailbox up independently
    /// disagree about a hostname more often than not, and a pair that both believe they are in
    /// step while holding different settings is the state nothing later can resolve.
    fn adopt(&self, account_id: &str, record: &SyncedAccount) {
        let base = fingerprint(&record.config);
        if self.remember(account_id, record, base) {
            log::info!(
                "allodia: [{}] this account was already at the service; adopting its record",
                mailcal_account::account_log_handle(account_id),
            );
        }
    }

    /// Write down what this device now knows about an account. `true` when it stuck.
    fn remember(&self, account_id: &str, record: &SyncedAccount, base: String) -> bool {
        let state = StoredSyncState {
            id: record.id.clone(),
            version: record.version,
            fingerprint: base,
        };
        match self.bookkeeping.set(account_id, state) {
            Ok(()) => true,
            Err(error) => {
                // The write landed and the note about it did not, so the next pass will not know
                // this record is spoken for and will offer it back. Annoying rather than
                // destructive, and worth an `error!` because the store refusing is the cause.
                log::error!(
                    "allodia: [{}] the account was synced but the note about it could not be \
                     stored ({error}); it will be offered back at the next pass",
                    mailcal_account::account_log_handle(account_id),
                );
                false
            }
        }
    }

    /// Say what one account's write did, without saying which account it was.
    fn complain(&self, account_id: &str, what: &str, error: &Error) {
        log::warn!(
            "allodia: [{}] this account {what}; {error}",
            mailcal_account::account_log_handle(account_id),
        );
    }
}

/// Remove an account's record, so the person's other devices learn it went.
///
/// Best effort by design: the removal here has already happened and cannot be undone, so a service
/// that could not be reached must not turn it into a failure. What it costs is that the record
/// survives, and the account comes back as an **offer** on this device's next pass; visible, and
/// something the person can ignore, rather than an account quietly resurrected.
pub(crate) fn forget_at_service(pass: &Pass<'_>, account_id: &str, state: &StoredSyncState) {
    let handle = mailcal_account::account_log_handle(account_id);
    match pass
        .service
        .delete_account(pass.transport, pass.token, &state.id, state.version)
    {
        Ok(()) => log::info!("allodia: [{handle}] the account's record is removed at the service"),
        Err(error) => log::warn!(
            "allodia: [{handle}] the account was removed here, but its record could not be \
             removed at the service ({error}); it will be offered back until it is"
        ),
    }
}

/// A key for a create that has not been attempted before.
///
/// **The account alone is not enough.** A key derived only from it is the same for the account's
/// whole life, so once its record is deleted the next create replays the first and is refused;
/// permanently, because neither the key nor the answer ever changes, and each delete stacks
/// another layer that no fixed number of retries unwinds. The moment is what makes this create
/// a different one; [`Pass::create_key`] is what makes it survive a retry.
///
/// The account's half is FNV-1a, written out rather than taken from
/// [`std::collections::hash_map::DefaultHasher`], whose output is explicitly not stable across
/// Rust releases.
fn fresh_create_key(account_id: &str) -> String {
    let digest = account_id
        .bytes()
        .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
            hash.wrapping_mul(0x0100_0000_01b3) ^ u64::from(byte)
        });
    let moment = time::OffsetDateTime::now_utc().unix_timestamp_nanos();
    format!("account-{digest:016x}-{moment}")
}

/// What a client is told when the service could not be reached or would not answer.
///
/// A refused token is the one worth telling apart: it means the sign-in has to be made again,
/// while everything else means try later.
pub(crate) fn report(error: Error) -> MailcalError {
    match error {
        Error::Unauthorized => MailcalError::Connect(
            "the Allodia account service refused this device's sign-in".to_owned(),
        ),
        other => MailcalError::Connect(other.to_string()),
    }
}

/// What a refusal from the account service says about the sign-in, or `None` when it says
/// nothing.
///
/// Only [`Error::Unauthorized`] is evidence: the service had the token and would not accept it,
/// which is a grant revoked here or an account removed on another device. A transport failure, a
/// status this version cannot read, a body it cannot parse and a conflict are all things that
/// happen to a perfectly good sign-in; recording any of them would sign somebody out over an
/// afternoon of bad weather at the service.
pub(crate) fn health_of(error: &Error) -> Option<AllodiaGrantHealth> {
    matches!(error, Error::Unauthorized).then_some(AllodiaGrantHealth::SignedOut)
}

impl MailcalApp {
    /// Record what a failed call to the account service says about the sign-in, and map it for a
    /// client.
    ///
    /// The single seam every call goes through, so a new endpoint cannot forget to classify: it
    /// gets the recording by using the mapper it needs anyway.
    pub(crate) fn report_allodia(&self, error: Error) -> MailcalError {
        if let Some(health) = health_of(&error) {
            self.note_allodia_health(health);
        }
        report(error)
    }
}

#[cfg(test)]
#[path = "allodia_pass_tests.rs"]
mod tests;
