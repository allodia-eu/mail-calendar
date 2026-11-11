//! Account sync on [`MailcalApp`]: the bookkeeping, one pass and one sharing change. The records
//! they carry are in [`crate::allodia_sync`]; one pass's worth of work is [`crate::allodia_pass`],
//! and when each runs is [`crate::app_allodia_devices`]'s.
//!
//! **The bookkeeping store arrives after construction**, unlike the credential store next door.
//! That store is a constructor parameter because the constructor's last statement starts dialing
//! and a rotation can land before a host has run a line of its own code. What a missing store gets
//! is an error rather than a silent fallback: a pass that quietly remembered nothing would
//! re-offer every account, every time, and read as a service fault.
//!
//! **A pass and a sharing change block**: each makes a token round trip and a request per
//! account, so both run on a thread of their own.

use crate::{
    AllodiaGrantHealth, MailcalApp, MailcalError,
    allodia_sync::AllodiaAccountSyncMode,
    sync_state::{SyncBookkeeping, SyncStateStore},
};

/// The scope checks, kept out of the exported block: [`allodia_license::Feature`] is an internal
/// vocabulary and must not reach the FFI, where it would become a client's business to know which
/// scope gates what.
#[cfg(feature = "allodia-license")]
impl MailcalApp {
    /// Whether the stored sign-in permits `feature`.
    ///
    /// The fast path, and the reason the scope set is stored at all: a grant that predates the
    /// permission can be reported without the round trip that would fail. A grant whose scopes
    /// were never recorded answers `true`: not knowing is not a reason to withhold something,
    /// and the request stays the authority.
    #[must_use]
    pub(crate) fn allodia_grant_permits(&self, feature: allodia_license::Feature) -> bool {
        let signed_in = self.allodia.lock().expect("allodia account lock");
        signed_in.as_ref().is_none_or(|stored| {
            crate::allodia_health::grant_permits(stored.granted_scopes.as_ref(), feature)
        })
    }
}

#[uniffi::export]
impl MailcalApp {
    /// Installs where this device remembers what it has synced, reads what is already there, and
    /// runs a pass when someone is signed in.
    ///
    /// Ordinary preferences rather than the keystore: nothing in the blob is secret, and a
    /// keychain prompt in front of a background pass would be a prompt nobody is there to answer.
    /// See [`crate::sync_state`]. A launch that must not reach the person's other devices (one on
    /// canned or isolated accounts) installs none, and no pass runs.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Config`] when the host's store could not be read. Reported rather
    /// than treated as empty: "never synced" starts a pass that re-adopts every record, and a
    /// store that is merely unreadable today must not look like that.
    pub fn use_allodia_sync_state_store(
        &self,
        store: Box<dyn SyncStateStore>,
    ) -> Result<(), MailcalError> {
        let bookkeeping =
            SyncBookkeeping::load(store).map_err(|err| MailcalError::Config(err.to_string()))?;
        self.allodia_sync.install(std::sync::Arc::new(bookkeeping));
        self.request_allodia_pass();
        Ok(())
    }

    /// What this device knows about its Allodia sign-in; see [`AllodiaGrantHealth`].
    ///
    /// Cheap and local: it reports what a call has already learned and never asks the service, so a
    /// client may read it while drawing a screen. A build with no Allodia sign-in has no grant to
    /// have an opinion about and answers [`AllodiaGrantHealth::Ok`], which is what a client draws
    /// nothing for.
    #[must_use]
    pub fn allodia_grant_health(&self) -> AllodiaGrantHealth {
        #[cfg(not(feature = "allodia-license"))]
        {
            AllodiaGrantHealth::Ok
        }
        #[cfg(feature = "allodia-license")]
        {
            self.allodia_health()
        }
    }
}

impl MailcalApp {
    /// How this account is shared with the person's other devices, read from the bookkeeping. A
    /// device keeping none answers [`AllodiaAccountSyncMode::On`], which is what an account
    /// nobody has excluded is.
    #[cfg_attr(
        not(feature = "allodia-license"),
        expect(dead_code, reason = "only a build that can sign in draws the control")
    )]
    pub(crate) fn allodia_account_sync_mode(&self, account_id: &str) -> AllodiaAccountSyncMode {
        let Some(bookkeeping) = self.allodia_sync.bookkeeping() else {
            return AllodiaAccountSyncMode::On;
        };
        if !bookkeeping.is_excluded(account_id) {
            // Whether or not a record exists yet: an account waiting for its first pass is On, not
            // a third state.
            AllodiaAccountSyncMode::On
        } else if bookkeeping.get(account_id).is_some() {
            AllodiaAccountSyncMode::Paused
        } else {
            AllodiaAccountSyncMode::Off
        }
    }
}

impl MailcalApp {
    /// The installed bookkeeping, or the wiring bug that means there is none.
    ///
    /// Every caller is inside the feature: a build with no Allodia sign-in installs a store and
    /// never reads it.
    #[cfg(feature = "allodia-license")]
    pub(crate) fn allodia_bookkeeping(
        &self,
    ) -> Result<std::sync::Arc<SyncBookkeeping>, MailcalError> {
        self.allodia_sync.bookkeeping().ok_or_else(|| {
            MailcalError::Config(
                "no sync state store has been installed for the Allodia account".to_owned(),
            )
        })
    }
}

#[cfg(feature = "allodia-license")]
mod enabled {
    use allodia_license::AccountService;

    use crate::{
        MailcalApp, MailcalError,
        allodia_pass::{Pass, forget_at_service, local_accounts},
        allodia_sync::{AllodiaAccountSyncMode, AllodiaSyncReport},
        allodia_transport::HttpsTransport,
    };

    impl MailcalApp {
        /// One pass, from the token to the report.
        pub(crate) fn run_allodia_pass(&self) -> Result<AllodiaSyncReport, MailcalError> {
            let token = self.allodia_access_token("the account list")?;
            // Asked after the token, because minting one is what learns the grant's scopes on an
            // install that had never recorded them, and asked before the first request, because
            // that request cannot succeed and its refusal would read as the service being down.
            if !self.allodia_grant_permits(allodia_license::Feature::ReadAccounts) {
                self.note_allodia_health(crate::AllodiaGrantHealth::NeedsReauth);
                return Err(MailcalError::Connect(
                    "this device's Allodia sign-in does not yet include permission to read the \
                     account list"
                        .to_owned(),
                ));
            }
            let bookkeeping = self.allodia_bookkeeping()?;
            let transport = HttpsTransport::new(self.runtime.handle().clone())
                .map_err(MailcalError::Connect)?;
            let service = AccountService::new(allodia_license::host());
            let pass = Pass {
                service: &service,
                transport: &transport,
                token: &token,
                bookkeeping: &bookkeeping,
            };

            let remote = pass.read().map_err(|err| self.report_allodia(err))?;
            let local = local_accounts(&self.registry.stored_configs(), &bookkeeping);
            log::info!(
                "allodia: syncing {} account(s) against {} held by the service",
                local.len(),
                remote.accounts.len(),
            );
            Ok(pass.apply(&local, &remote))
        }

        /// Move one account to `mode`, doing whatever that position takes: **On** puts it back in
        /// the pass; **Paused** needs a record to pause, so an account never sent is sent first,
        /// and the other devices keep it while this one stops exchanging changes about it; **Off**
        /// removes the record, so the other devices are asked whether to drop the account too,
        /// and marks it excluded so no later pass puts it straight back. ⚠️ Off is the only
        /// position that reaches the other devices, and none touches a mailbox. A failure leaves
        /// the setting where it was.
        ///
        /// The order within each arm is the part that matters: nothing is written down here until
        /// the service has agreed, so a pass interrupted anywhere finds a device whose bookkeeping
        /// still describes something true.
        pub(crate) fn run_allodia_mode_change(
            &self,
            account_id: &str,
            mode: AllodiaAccountSyncMode,
        ) -> Result<(), MailcalError> {
            if self.allodia_account_sync_mode(account_id) == mode {
                return Ok(());
            }
            let bookkeeping = self.allodia_bookkeeping()?;
            let handle = mailcal_account::account_log_handle(account_id);
            match mode {
                // The pass that sends it follows the change, as after every change that took.
                AllodiaAccountSyncMode::On => {
                    bookkeeping
                        .set_excluded(account_id, false)
                        .map_err(|err| MailcalError::Connect(err.to_string()))?;
                    log::info!("allodia: [{handle}] shared with the person's other devices again");
                }
                AllodiaAccountSyncMode::Paused => {
                    // There has to be a record to pause. An account that has never been sent is
                    // sent first, or "the other devices keep it" would be a promise about nothing.
                    if bookkeeping.get(account_id).is_none() {
                        bookkeeping
                            .set_excluded(account_id, false)
                            .map_err(|err| MailcalError::Connect(err.to_string()))?;
                        self.run_allodia_pass()?;
                    }
                    bookkeeping
                        .set_excluded(account_id, true)
                        .map_err(|err| MailcalError::Connect(err.to_string()))?;
                    log::info!(
                        "allodia: [{handle}] stays with the other devices; changes stop here"
                    );
                }
                AllodiaAccountSyncMode::Off => {
                    if let Some(state) = bookkeeping.get(account_id) {
                        let token = self.allodia_access_token("removing an account")?;
                        let transport = HttpsTransport::new(self.runtime.handle().clone())
                            .map_err(MailcalError::Connect)?;
                        let service = AccountService::new(allodia_license::host());
                        service
                            .delete_account(&transport, &token, &state.id, state.version)
                            .map_err(|err| self.report_allodia(err))?;
                        // Only once the service has agreed. Forgetting first would leave a record
                        // nothing here knows about, offering the account back at the next pass.
                        bookkeeping
                            .forget(account_id)
                            .map_err(|err| MailcalError::Connect(err.to_string()))?;
                    }
                    // After `forget`, which clears it: without this the next pass would upload the
                    // account straight back, and the position would undo itself within seconds.
                    bookkeeping
                        .set_excluded(account_id, true)
                        .map_err(|err| MailcalError::Connect(err.to_string()))?;
                    log::info!("allodia: [{handle}] taken off the person's other devices");
                }
            }
            Ok(())
        }

        /// Tell the service an account is gone, on the way out of
        /// [`MailcalApp::remove_account`](crate::MailcalApp::remove_account).
        ///
        /// Nothing is returned, because nothing here may fail a removal the person has already
        /// made. What every failure costs is the same and is logged where it happens: the record
        /// outlives the account and comes back as an offer.
        pub(crate) fn forget_allodia_record(&self, account_id: &str) {
            self.allodia_sync.forget_account(account_id);
            let _writing = self
                .allodia_sync
                .writing
                .lock()
                .expect("allodia write lock");
            let Ok(bookkeeping) = self.allodia_bookkeeping() else {
                return;
            };
            let Some(state) = bookkeeping.get(account_id) else {
                return;
            };
            let Ok(token) = self.allodia_access_token("forgetting an account") else {
                return;
            };
            let Ok(transport) = HttpsTransport::new(self.runtime.handle().clone()) else {
                return;
            };
            let service = AccountService::new(allodia_license::host());
            forget_at_service(
                &Pass {
                    service: &service,
                    transport: &transport,
                    token: &token,
                    bookkeeping: &bookkeeping,
                },
                account_id,
                &state,
            );
            // The entry goes whether or not the service was told. Keeping it would leave this
            // device holding a claim on a record for an account it no longer has, and the entry is
            // what a later pass would use to push settings that are gone.
            if let Err(error) = bookkeeping.forget(account_id) {
                log::warn!(
                    "allodia: [{}] the account is removed, but its sync note could not be \
                     cleared; {error}",
                    mailcal_account::account_log_handle(account_id),
                );
            }
        }
    }
}
