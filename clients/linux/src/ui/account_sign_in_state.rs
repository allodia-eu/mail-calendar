//! What the open account's Sign-in section knows about a change: what the server accepts, and
//! where a running change is.

use mailcal_bindings::{AccountCapability, AccountSignInOptions, AccountSignInStart};

use super::Attempt;

/// Where a running change is, which decides what its page offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Stage {
    /// The core is starting or completing it, so every control waits.
    Core,
    /// The browser holds it. The controls stay live, because a fresh press replaces it
    /// (`docs/provider-oauth.md` rule 13), and Cancel drops it.
    Browser,
}

/// What the open account's Sign-in section knows. Here rather than in the window, which is
/// rebuilt on every change while a question to the server and a browser hop each outlive
/// several rebuilds.
#[derive(Debug, Clone, Default)]
pub(crate) struct SignInSwitchState {
    /// What one account's server accepts, and the account it is for.
    answered: Option<(String, AccountSignInOptions)>,
    /// The account whose question is out, so a page opened twice asks once.
    asking: Option<String>,
    /// The account whose sign-in is changing, while it is, and where the change is.
    running: Option<(String, Stage)>,
    /// A switch waiting on what becomes of the calendar and contacts it would not reach, asked
    /// before the browser opens (`docs/accounts.md` rule 18).
    pub(super) question: Option<(Attempt, Box<AccountSignInStart>)>,
}

impl SignInSwitchState {
    /// What `account`'s server accepts, once it has answered.
    pub(crate) fn options(&self, account: &str) -> Option<&AccountSignInOptions> {
        self.answered
            .as_ref()
            .filter(|(id, _)| id == account)
            .map(|(_, options)| options)
    }

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

    /// Whether to ask about `account`: nothing is known or asked about it yet.
    pub(super) fn ask(&mut self, account: &str) -> bool {
        if self.options(account).is_some() || self.asking.as_deref() == Some(account) {
            return false;
        }
        self.asking = Some(account.to_owned());
        true
    }

    pub(super) fn answer(&mut self, account: String, options: AccountSignInOptions) {
        if self.asking.as_deref() == Some(account.as_str()) {
            self.asking = None;
        }
        self.answered = Some((account, options));
    }

    /// Forgets what `account`'s server was said to accept: its way in changed, and with it the
    /// question.
    pub(crate) fn forget(&mut self, account: &str) {
        if self.options(account).is_some() {
            self.answered = None;
        }
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

    fn restage(&mut self, account: &str, stage: Stage) {
        if let Some((id, current)) = self.running.as_mut()
            && id == account
        {
            *current = stage;
        }
    }
}
