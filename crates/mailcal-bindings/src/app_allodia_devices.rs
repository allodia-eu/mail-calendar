//! The other devices' card and an account's sharing control, as the core runs them
//! ([`crate::allodia_devices`]). A client draws both and starts neither the pass nor the change
//! itself: the pass runs when what it compares may have moved, and a sharing change is a change
//! on the account's page (`AccountPageIntent::SetSharing`).

use crate::{
    AccountSharing, AllodiaAccountSyncMode, AllodiaDevices, ControlState, MailcalApp,
    account_work::{Hold, PageWork},
};

#[uniffi::export]
impl MailcalApp {
    /// What Settings → Accounts draws above the accounts about the person's other devices; `None`
    /// when there is nothing to say, or nobody is signed in to an Allodia account. Read again
    /// after every `Surface::Settings` signal.
    #[must_use]
    pub fn allodia_devices(&self) -> Option<AllodiaDevices> {
        if !self.shares_with_other_devices() {
            return None;
        }
        self.allodia_sync.view(|account| {
            let (hold, _) = self.account_work.of(account);
            ControlState {
                enabled: !hold.busy,
                running: sharing_going(&hold).is_some(),
            }
        })
    }
}

impl MailcalApp {
    /// Someone is signed in, and this device keeps a note of what it has shared.
    fn shares_with_other_devices(&self) -> bool {
        self.allodia_account().is_some() && self.allodia_sync.bookkeeping().is_some()
    }

    /// Runs a pass on a thread of its own, or once more after the one running; nothing while
    /// nobody is signed in. Every route to a pass comes here, so two never run at once.
    pub(crate) fn request_allodia_pass(&self) {
        #[cfg(feature = "allodia-license")]
        {
            if !self.shares_with_other_devices() {
                return;
            }
            let Some(app) = self.rediscovery.app() else {
                return;
            };
            let Some(mut session) = self.allodia_sync.want_pass() else {
                return;
            };
            self.app.accounts_changed();
            std::thread::spawn(move || {
                loop {
                    let outcome = {
                        let _writing = app.allodia_sync.writing.lock().expect("allodia write lock");
                        app.allodia_pass()
                    };
                    let next = app.allodia_sync.pass_ended(session, outcome);
                    app.app.accounts_changed();
                    match next {
                        Some(next) => session = next,
                        None => break,
                    }
                }
            });
        }
    }

    /// One pass, as the card says what it came to.
    #[cfg(feature = "allodia-license")]
    fn allodia_pass(
        &self,
    ) -> Result<crate::allodia_sync::AllodiaSyncReport, crate::AllodiaSyncFailure> {
        #[cfg(test)]
        if let Some(canned) = self
            .allodia_sync
            .canned
            .lock()
            .expect("canned lock")
            .clone()
        {
            return canned;
        }
        self.run_allodia_pass().map_err(|error| {
            log::warn!("allodia: the account list could not be synced; {error}");
            self.allodia_grant_health().into()
        })
    }

    /// The page's sharing control, for an account the other devices can hold.
    pub(crate) fn sharing_section(&self, account: &str, hold: &Hold) -> Option<AccountSharing> {
        #[cfg(not(feature = "allodia-license"))]
        {
            let _ = (account, hold);
            None
        }
        #[cfg(feature = "allodia-license")]
        {
            if !self.shares_with_other_devices() {
                return None;
            }
            allodia_license::to_synced(&self.registry.stored_config(account)?).ok()?;
            let going = sharing_going(hold);
            Some(AccountSharing {
                mode: going.unwrap_or_else(|| self.allodia_account_sync_mode(account)),
                control: ControlState {
                    enabled: !hold.busy,
                    running: going.is_some(),
                },
                failed: self.allodia_sync.sharing_failed(account),
            })
        }
    }

    /// Moves `account` to `mode`, as the page's change; a pass follows one that took, so the card
    /// says what the other devices now hold.
    pub(crate) fn change_sharing(&self, account: &str, mode: AllodiaAccountSyncMode) {
        #[cfg(not(feature = "allodia-license"))]
        let _ = (account, mode);
        #[cfg(feature = "allodia-license")]
        {
            let outcome = {
                let _writing = self
                    .allodia_sync
                    .writing
                    .lock()
                    .expect("allodia write lock");
                self.run_allodia_mode_change(account, mode)
            };
            if let Err(error) = &outcome {
                log::warn!(
                    "allodia: [{}] how the account is shared did not change; {error}",
                    mailcal_account::account_log_handle(account),
                );
            }
            let took = outcome.is_ok();
            self.allodia_sync.sharing_changed(
                account,
                outcome.map_err(|_| self.allodia_grant_health().into()),
            );
            if took {
                self.request_allodia_pass();
            }
        }
    }
}

/// Where a sharing change this page started takes its control.
fn sharing_going(hold: &Hold) -> Option<AllodiaAccountSyncMode> {
    hold.going(|work| match work {
        PageWork::Sharing(mode) => Some(*mode),
        _ => None,
    })
}

#[cfg(all(test, feature = "allodia-license"))]
#[path = "app_allodia_devices_tests.rs"]
mod tests;
