//! Host work that must not run on the GTK main thread, and the state tracking what is in
//! flight.
//!
//! Account detection, connecting and removal, the provider sign-ins and the background new-mail
//! scan each block on the network, so each is handed to a thread and reports back through an
//! [`AppInput`]. What the state guards is the second launch of one of them: an OAuth attempt
//! carries an id and a cancel flag so a stale completion cannot publish over a newer one, and
//! the background scan collapses repeats into a single follow-up rather than queueing a scan
//! per mailbox change. The sign-ins themselves are [`super::oauth_actions`] and
//! [`super::jmap_actions`].
//!
//! It also carries the two first-run flags, because the setup task in this file is what
//! completes them: the welcome screen hands over to setup, and a connected account closes both.

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use super::{
    AppInput, AppModel, dns, notifications, oauth_loopback::OAuthLoopback,
    setup_model::AccountSubmission,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum BackgroundScan {
    #[default]
    Idle,
    InFlight,
    Pending,
}

#[derive(Debug)]
pub(super) struct HostTasks {
    pub(super) welcome_pending: bool,
    pub(super) setup_after_welcome: bool,
    background: BackgroundScan,
    /// One slot per browser flow. They are independent; a JMAP pre-flight running while a
    /// Microsoft sign-in is open must not cancel it; but each behaves identically, so they
    /// share [`AttemptSlot`] rather than a third copy of the same four methods.
    pub(super) google: AttemptSlot,
    pub(super) microsoft: AttemptSlot,
    pub(super) jmap: AttemptSlot,
    pub(super) allodia: AttemptSlot,
    jmap_loopback: Option<OAuthLoopback>,
}

/// The one in-flight browser sign-in of a given kind: an id, so a stale completion cannot
/// publish over a newer one, and a flag the waiting worker polls.
#[derive(Debug)]
pub(super) struct AttemptSlot {
    next: u64,
    current: Option<OAuthAttempt>,
}

#[derive(Debug)]
struct OAuthAttempt {
    id: u64,
    cancel: Arc<AtomicBool>,
}

impl AttemptSlot {
    const fn empty() -> Self {
        Self {
            next: 0,
            current: None,
        }
    }

    pub(super) fn start(&mut self) -> (u64, Arc<AtomicBool>) {
        if let Some(attempt) = self.current.take() {
            attempt.cancel.store(true, Ordering::Release);
        }
        self.next = self.next.wrapping_add(1);
        let cancel = Arc::new(AtomicBool::new(false));
        self.current = Some(OAuthAttempt {
            id: self.next,
            cancel: Arc::clone(&cancel),
        });
        (self.next, cancel)
    }

    /// Releases the slot as well as raising the flag: holding it until the abandoned worker
    /// reports back would leave the sign-in button a silent no-op for as long as the cancelled
    /// exchange takes (`docs/provider-oauth.md` rule 13).
    pub(super) fn cancel(&mut self) -> bool {
        let Some(attempt) = self.current.take() else {
            return false;
        };
        attempt.cancel.store(true, Ordering::Release);
        true
    }

    pub(super) fn holds(&self, id: u64) -> bool {
        self.current
            .as_ref()
            .is_some_and(|attempt| attempt.id == id)
    }

    pub(super) fn cancel_token(&self, id: u64) -> Option<Arc<AtomicBool>> {
        self.current
            .as_ref()
            .filter(|attempt| attempt.id == id)
            .map(|attempt| Arc::clone(&attempt.cancel))
    }

    pub(super) fn finish(&mut self, id: u64) -> bool {
        if !self.holds(id) {
            return false;
        }
        self.current = None;
        true
    }
}

impl HostTasks {
    pub(super) const fn new(welcome_pending: bool, setup_after_welcome: bool) -> Self {
        Self {
            welcome_pending,
            setup_after_welcome,
            background: BackgroundScan::Idle,
            google: AttemptSlot::empty(),
            microsoft: AttemptSlot::empty(),
            jmap: AttemptSlot::empty(),
            allodia: AttemptSlot::empty(),
            jmap_loopback: None,
        }
    }

    /// One redirect URI for every JMAP attempt in this process, so a retry reuses the core's
    /// dynamic-registration cache instead of registering this install again.
    pub(super) fn jmap_loopback(&mut self) -> std::io::Result<OAuthLoopback> {
        if self.jmap_loopback.is_none() {
            self.jmap_loopback = Some(OAuthLoopback::bind()?);
        }
        self.jmap_loopback
            .as_ref()
            .expect("JMAP loopback was initialized")
            .try_clone()
    }

    /// Returns the listener named by an existing JMAP grant. A cold launch has to rebind its
    /// registered port; a same-process repair reuses the listener already retaining that port.
    pub(super) fn jmap_loopback_for_redirect(
        &mut self,
        redirect_uri: &str,
    ) -> std::io::Result<OAuthLoopback> {
        if self
            .jmap_loopback
            .as_ref()
            .is_some_and(|loopback| loopback.redirect_uri() == redirect_uri)
        {
            return self
                .jmap_loopback
                .as_ref()
                .expect("matching JMAP loopback exists")
                .try_clone();
        }
        self.jmap_loopback = None;
        self.jmap_loopback = Some(OAuthLoopback::bind_redirect_uri(redirect_uri)?);
        self.jmap_loopback
            .as_ref()
            .expect("JMAP re-authentication loopback was initialized")
            .try_clone()
    }

    fn start_background(&mut self) -> bool {
        match self.background {
            BackgroundScan::Idle => {
                self.background = BackgroundScan::InFlight;
                true
            }
            BackgroundScan::InFlight => {
                self.background = BackgroundScan::Pending;
                false
            }
            BackgroundScan::Pending => false,
        }
    }

    fn finish_background(&mut self) -> bool {
        let repeat = self.background == BackgroundScan::Pending;
        self.background = BackgroundScan::Idle;
        repeat
    }
}

impl AppModel {
    pub(super) fn detect_account(&mut self, email: String, sender: relm4::Sender<AppInput>) {
        let Some(app) = self.app.clone() else {
            return;
        };
        self.setup.detecting();
        std::thread::spawn(move || {
            let recommendation =
                app.detect_account_settings(email.clone(), Some(Box::new(dns::NativeResolver)));
            sender.emit(AppInput::AccountDetected(email, Box::new(recommendation)));
        });
    }

    pub(super) fn submit_account(
        &mut self,
        submission: AccountSubmission,
        sender: relm4::Sender<AppInput>,
    ) {
        let Some(app) = self.app.clone() else {
            return;
        };
        self.setup.connecting();
        std::thread::spawn(move || {
            let result = submission.config_toml().and_then(|config| {
                // The core persists the credential through the host's `AccountCredentialStore`
                // and rolls the add back itself when that write fails.
                app.add_account(config)
                    .map(|_| ())
                    .map_err(|error| error.to_string())
            });
            sender.emit(AppInput::AccountAdded(result));
        });
    }

    pub(super) fn account_added(&mut self, result: Result<(), String>) {
        match result {
            Ok(()) => {
                self.setup.complete();
                if let Some(app) = &self.app {
                    self.snapshot = app.mailbox_list();
                }
                self.try_open_pending_mailto();
            }
            Err(error) => self
                .setup
                .failed(crate::l10n::status_connect_failed(&error)),
        }
    }

    pub(super) fn remove_account(&mut self, id: String, sender: relm4::Sender<AppInput>) {
        let Some(app) = self.app.clone() else {
            return;
        };
        std::thread::spawn(move || {
            // The core erases the stored credential through the host's `AccountCredentialStore`,
            // so removing the Secret Service item here as well would be a second, racing delete.
            let result = app.remove_account(id).map_err(|error| error.to_string());
            sender.emit(AppInput::AccountRemoved(result));
        });
    }

    pub(super) fn collect_new_mail(&mut self, sender: relm4::Sender<AppInput>) {
        let Some(app) = self.app.clone() else {
            return;
        };
        if self.snapshot.accounts.is_empty() {
            return;
        }
        if !self.host_tasks.start_background() {
            return;
        }
        let enabled = self.preferences.notifications_enabled();
        std::thread::spawn(move || {
            let outcome = app.collect_cached_new_mail();
            if enabled {
                notifications::post(outcome);
            }
            sender.emit(AppInput::BackgroundFinished);
        });
    }

    pub(super) fn background_finished(&mut self, sender: &relm4::Sender<AppInput>) {
        if self.host_tasks.finish_background() {
            sender.emit(AppInput::CollectNewMail);
        }
    }

    pub(super) fn replace_account_secret(
        &mut self,
        account: String,
        secret: String,
        sender: relm4::Sender<AppInput>,
    ) {
        let Some(app) = self.app.clone() else {
            return;
        };
        self.credential_repair_failed = None;
        std::thread::spawn(move || {
            let repaired = account.clone();
            let success = app.replace_account_secret(account, secret).is_ok();
            sender.emit(AppInput::AccountSecretReplaced {
                account: repaired,
                success,
            });
        });
    }

    pub(super) fn account_secret_replaced(&mut self, account: String, success: bool) {
        self.settings
            .open(Some(super::settings::Category::Accounts));
        if success {
            self.credential_repair_failed = None;
            self.notice = None;
            if let Some(app) = &self.app {
                self.snapshot = app.mailbox_list();
                self.connectivity =
                    super::connectivity::ConnectivityState::pull(app, &self.snapshot.accounts);
            }
        } else {
            self.credential_repair_failed = Some(account);
            self.notice = Some(crate::l10n::signin_expired_failed().to_owned());
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;

    use super::HostTasks;

    #[test]
    fn a_mailbox_change_during_a_scan_queues_exactly_one_follow_up() {
        let mut state = HostTasks::new(false, false);
        assert!(state.start_background());
        assert!(!state.start_background());
        assert!(!state.start_background());
        assert!(state.finish_background());
        assert!(state.start_background());
        assert!(!state.finish_background());
    }

    #[test]
    fn an_attempt_cancels_once_and_ignores_a_stale_completion() {
        let mut state = HostTasks::new(false, false);
        let (first, cancelled) = state.google.start();
        let (second, _) = state.google.start();
        assert!(cancelled.load(Ordering::Acquire));
        // Replacing frees the slot at once, so a retry never waits on the abandoned worker.
        assert!(!state.google.holds(first));
        assert_ne!(first, second);
        assert!(!state.google.finish(first));
        assert!(state.google.holds(second));
        assert!(state.google.cancel());
        assert!(!state.google.cancel());
        let (third, _) = state.google.start();
        assert_ne!(second, third);
        assert!(state.google.finish(third));
    }

    #[test]
    fn the_three_sign_in_slots_do_not_interfere() {
        let mut state = HostTasks::new(false, false);
        let (google, _) = state.google.start();
        let (microsoft, _) = state.microsoft.start();
        let (jmap, _) = state.jmap.start();

        // Each slot answers only for its own flow: ids collide across slots (all start at 1),
        // so a shared one would let a finished Microsoft sign-in retire a live Google one.
        assert!(state.google.cancel());
        assert!(state.microsoft.holds(microsoft));
        assert!(state.jmap.holds(jmap));
        assert!(!state.google.holds(google));
        assert!(state.microsoft.finish(microsoft));
        assert!(state.jmap.finish(jmap));
    }

    #[test]
    fn jmap_retries_keep_one_redirect_for_dynamic_registration_cache_reuse() {
        let mut state = HostTasks::new(false, false);
        let first = state.jmap_loopback().expect("first listener");
        let second = state.jmap_loopback().expect("cloned listener");

        assert_eq!(first.redirect_uri(), second.redirect_uri());
    }

    #[test]
    fn jmap_reauthentication_reuses_the_registered_redirect() {
        let mut state = HostTasks::new(false, false);
        let original = state.jmap_loopback().expect("original listener");
        let rebound = state
            .jmap_loopback_for_redirect(&original.redirect_uri())
            .expect("reuse registered listener");

        assert_eq!(original.redirect_uri(), rebound.redirect_uri());
    }
}
