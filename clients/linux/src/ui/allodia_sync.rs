//! Keeping this device's mail-account list in step with the person's other devices.
//!
//! The core does the deciding and the writing; what is here is when to ask it, and what to do with
//! the part it cannot answer alone. The pass **blocks** on the network, so it runs on a thread and
//! comes back as an input; the shape every other network call in this client takes.

use std::sync::Arc;

use mailcal_bindings::{AllodiaAccountSyncMode, AllodiaGrantHealth, AllodiaSyncReport, MailcalApp};

use super::{AppInput, AppModel};

/// How a pass ended.
///
/// `Failed` carries the core's own words. They name endpoints and status codes, never an address
/// or a secret, so showing one stays inside the never-log-content rule.
#[derive(Debug)]
pub(crate) enum AllodiaSyncOutcome {
    Done(Box<AllodiaSyncReport>, AllodiaGrantHealth),
    /// The failure's own words for the log, and the core's typed answer for the screen. Both,
    /// because they are for different readers: one diagnoses, the other decides what a person is
    /// told, and using the first for the second is how an OAuth field name became product copy.
    Failed(String, AllodiaGrantHealth),
}

/// What this launch knows about the person's other devices, apart from the pass itself.
///
/// Together on the model because they are read together and neither survives the launch: whether
/// a pass is worth running at all, and where the last one left each account.
#[derive(Debug, Default)]
pub(crate) struct AllodiaLaunch {
    /// Whether this launch may keep its account list in step with the person's other devices
    /// ([`crate::boot::BootedApp`]).
    pub(crate) syncable: bool,
    /// Which position each account is in, keyed by account id; what the per-account control
    /// draws. A local read; it never asks the service.
    pub(crate) accounts_synced: std::collections::HashMap<String, AllodiaAccountSyncMode>,
}

/// What the Accounts page draws about the person's other devices.
///
/// `report` is `None` until a pass has run, which is not the same as a pass that found nothing:
/// the first has no business drawing a heading, and the second has earned drawing none.
#[derive(Debug, Default, Clone)]
pub(crate) struct AllodiaSyncState {
    pub(crate) checking: bool,
    pub(crate) report: Option<AllodiaSyncReport>,
    pub(crate) failure: Option<String>,
    /// What the core knows about the sign-in itself; what a failure is DRAWN from.
    pub(crate) health: AllodiaGrantHealth,
}

impl AllodiaSyncState {
    /// Whether there is anything at all to put on screen.
    pub(crate) fn has_something_to_say(&self) -> bool {
        self.report.as_ref().is_some_and(|report| {
            !report.offers.is_empty()
                || !report.changed_elsewhere.is_empty()
                || !report.removed_elsewhere.is_empty()
        })
    }
}

impl AppModel {
    /// Runs one pass, if there is any point in running one.
    ///
    /// Nobody signed in, or a launch on canned harness accounts: there is nothing worth syncing,
    /// and asking would only produce an error to draw. A pass already running is left to finish,
    /// two at once would race each other's writes.
    pub(super) fn sync_allodia_accounts(&mut self, sender: relm4::Sender<AppInput>) {
        let Some(app) = self.app.clone() else {
            return;
        };
        if !self.allodia.syncable
            || self.settings.allodia_sync.checking
            || app.allodia_account().is_none()
        {
            return;
        }
        self.settings.allodia_sync.checking = true;
        self.settings.allodia_sync.failure = None;
        self.refresh_settings_in_place();
        std::thread::spawn(move || {
            sender.emit(AppInput::AllodiaSyncFinished(run(&app)));
        });
    }

    pub(super) fn allodia_sync_finished(&mut self, outcome: AllodiaSyncOutcome) {
        self.settings.allodia_sync.checking = false;
        match outcome {
            AllodiaSyncOutcome::Done(report, health) => {
                self.settings.allodia_sync.report = Some(*report);
                self.settings.allodia_sync.failure = None;
                self.settings.allodia_sync.health = health;
            }
            AllodiaSyncOutcome::Failed(error, health) => {
                self.settings.allodia_sync.failure = Some(error);
                self.settings.allodia_sync.health = health;
            }
        }
        self.refresh_settings_in_place();
    }

    /// Moves one account to a sync position.
    ///
    /// The core does everything the position takes; including reaching the service; so it runs
    /// on a thread and comes back as an input. The rows asking about an account changed or removed
    /// elsewhere go as soon as it is answered: a question still on screen afterwards reads as the
    /// answer not having worked.
    pub(super) fn set_allodia_account_sync_mode(
        &mut self,
        account_id: &str,
        mode: AllodiaAccountSyncMode,
        sender: relm4::Sender<AppInput>,
    ) {
        let Some(app) = self.app.clone() else {
            return;
        };
        let account_id = account_id.to_owned();
        std::thread::spawn(move || {
            let failure = app
                .set_allodia_account_sync_mode(account_id.clone(), mode)
                .err()
                .map(|error| error.to_string());
            sender.emit(AppInput::AllodiaSyncModeChanged(account_id, failure));
        });
    }

    /// The position is re-read from the core rather than assumed, so a change the service refused
    /// leaves the control where it was instead of lying about what happened.
    pub(super) fn allodia_sync_mode_changed(&mut self, account_id: &str, failure: Option<String>) {
        if failure.is_none()
            && let Some(report) = self.settings.allodia_sync.report.as_mut()
        {
            report
                .changed_elsewhere
                .retain(|change| change.account_id != account_id);
            report
                .removed_elsewhere
                .retain(|change| change.account_id != account_id);
        }
        self.settings.allodia_sync.failure = failure;
        self.read_accounts_synced();
        self.refresh_settings_in_place();
    }

    /// Re-reads how each account is shared. A local read per account; it never asks the service.
    ///
    /// Empty in a build with no Allodia registration, which is what draws no control at all. The
    /// bookkeeping store is installed either way, so it would otherwise answer for every account.
    pub(super) fn read_accounts_synced(&mut self) {
        let Some(app) = self
            .app
            .clone()
            .filter(|_| mailcal_bindings::allodia_sign_in_available())
        else {
            self.allodia.accounts_synced.clear();
            return;
        };
        self.allodia.accounts_synced = app
            .sync_settings()
            .accounts
            .into_iter()
            .map(|account| {
                let mode = app.allodia_account_sync_mode(account.account_id.clone());
                (account.account_id, mode)
            })
            .collect();
    }

    /// The account list changed, so the person's other devices should hear about it now rather
    /// than at the next launch. A no-op when nobody is signed in.
    pub(super) fn sync_after_account_change(&mut self, sender: relm4::Sender<AppInput>) {
        self.read_accounts_synced();
        self.sync_allodia_accounts(sender);
    }

    /// Sets up an account one of the person's other devices offered, on the route its record
    /// names.
    ///
    /// The ordinary flow with the work already done, which is what syncing an account list is
    /// *for*: the record says which provider and which servers, so this opens the same card
    /// detection would have produced instead of spending a round trip re-learning it; and for a
    /// domain that publishes no autoconfig, re-learning it would have found less and dropped the
    /// person onto the manual form for an account another device set up without trouble.
    ///
    /// The password is still asked for here, because no password travels. A record that names no
    /// server routes to detection rather than to an empty form
    /// ([`mailcal_bindings::setup_from_offer`]). Settings stays open behind it: the setup
    /// window is a modal of its own.
    pub(super) fn set_up_offered_account(
        &mut self,
        offer: mailcal_bindings::AllodiaAccountOffer,
        sender: relm4::Sender<AppInput>,
    ) {
        if !self.can_open_account_setup() {
            return;
        }
        let email = offer.email.clone();
        // `required` is re-derived rather than assumed false: an offer accepted on the FIRST run
        // must leave the window required, or the screen the person cannot skip becomes one they
        // can close into an app with no accounts.
        self.setup
            .open_on(self.snapshot.accounts.is_empty(), email.clone());
        self.account_detected(email, mailcal_bindings::setup_from_offer(offer), sender);
    }

    /// Forgets what the other devices said. Called on sign-out: there is nothing left to say about
    /// them once this device leaves the account that linked them.
    pub(super) fn forget_allodia_sync(&mut self) {
        self.settings.allodia_sync = AllodiaSyncState::default();
    }
}

/// One pass, off the main thread.
fn run(app: &Arc<MailcalApp>) -> AllodiaSyncOutcome {
    match app.sync_allodia_accounts() {
        Ok(report) => AllodiaSyncOutcome::Done(Box::new(report), app.allodia_grant_health()),
        Err(error) => AllodiaSyncOutcome::Failed(error.to_string(), app.allodia_grant_health()),
    }
}
