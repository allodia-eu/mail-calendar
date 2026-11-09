//! Where a browser sign-in the open account's Sign-in section started is.

use mailcal_bindings::{AccountCapability, AccountSignInStart};

use super::Attempt;
use crate::ui::nextcloud_flight::Grant;

/// Where a running change is, which decides what its page offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Stage {
    /// The core is starting or completing it, so every control waits.
    Core,
    /// The browser holds it. The controls stay live, because a fresh press replaces it
    /// (`docs/provider-oauth.md` rule 13), and Cancel drops it.
    Browser,
}

/// Where a browser sign-in is. Here rather than in the window, which is rebuilt on every change
/// while a browser hop outlives several rebuilds.
#[derive(Debug, Clone, Default)]
pub(crate) struct SignInSwitchState {
    /// The account whose sign-in is changing, while it is, and where the change is.
    running: Option<(String, Stage)>,
    /// A switch waiting on what becomes of the calendar and contacts it would not reach, asked
    /// before the browser opens (`docs/accounts.md` rule 18).
    pub(super) question: Option<(Attempt, Box<AccountSignInStart>)>,
    /// A Nextcloud grant the core could not put on its account yet, kept for the next try
    /// because the server hands it out once (`docs/nextcloud-sign-in.md` rule 6).
    grant: Option<(String, Grant)>,
}

impl SignInSwitchState {
    /// Where `account`'s change is, while one runs.
    pub(crate) fn running(&self, account: &str) -> Option<Stage> {
        self.running
            .as_ref()
            .filter(|(id, _)| id == account)
            .map(|(_, stage)| *stage)
    }

    /// The switch waiting on an answer, by its attempt, and what it would not reach.
    pub(crate) fn question(&self) -> Option<(u64, &[AccountCapability])> {
        self.question
            .as_ref()
            .map(|(attempt, start)| (attempt.id, start.unreached.as_slice()))
    }

    /// A change for `account` started, replacing any other: the core is asked first.
    pub(super) fn begin(&mut self, account: &str) {
        self.running = Some((account.to_owned(), Stage::Core));
    }

    /// The browser opened on `account`'s change.
    pub(super) fn in_browser(&mut self, account: &str) {
        self.restage(account, Stage::Browser);
    }

    /// The browser came back, and the core completes `account`'s change.
    pub(super) fn completing(&mut self, account: &str) {
        self.restage(account, Stage::Core);
    }

    /// Drops `account`'s change while the browser holds it. `false` when there is none to drop,
    /// including one the core is already completing.
    pub(super) fn cancel(&mut self, account: &str) -> bool {
        if self.running(account) != Some(Stage::Browser) {
            return false;
        }
        self.running = None;
        true
    }

    /// `account`'s change ended.
    pub(super) fn end(&mut self, account: &str) {
        if self.running(account).is_some() {
            self.running = None;
        }
    }

    /// The grant kept for `account`, when there is one.
    pub(super) fn held_grant(&self, account: &str) -> Option<Grant> {
        self.grant
            .as_ref()
            .filter(|(id, _)| id == account)
            .map(|(_, grant)| grant.clone())
    }

    pub(super) fn hold_grant(&mut self, account: String, grant: Grant) {
        self.grant = Some((account, grant));
    }

    /// Forgets `account`'s grant: the core used it, or withdrew it.
    pub(super) fn take_grant(&mut self, account: &str) {
        if self.held_grant(account).is_some() {
            self.grant = None;
        }
    }

    /// Takes whichever grant is kept, to withdraw it; never one the core is putting on its
    /// account now.
    pub(super) fn take_any_grant(&mut self) -> Option<Grant> {
        let (account, _) = self.grant.as_ref()?;
        if self.running(account) == Some(Stage::Core) {
            return None;
        }
        self.grant.take().map(|(_, grant)| grant)
    }

    fn restage(&mut self, account: &str, stage: Stage) {
        if let Some((id, current)) = self.running.as_mut()
            && id == account
        {
            *current = stage;
        }
    }
}
