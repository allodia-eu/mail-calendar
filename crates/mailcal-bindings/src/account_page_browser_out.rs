//! What a client draws of the browser sign-in out.

use std::time::Duration;

use super::{Attempt, Stage, Target};
use crate::{AccountCapability, BrowserSignIn, BrowserSignInStage, ControlState};

/// The sign-in out for one account, as its page draws it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Out {
    pub(crate) record: BrowserSignIn,
    /// What it signs in, and so which section draws it.
    pub(crate) target: Target,
    /// The uses it asks for besides what the account is used for.
    pub(crate) adding: Vec<AccountCapability>,
    /// The control that started it is running: the server starts it, the browser is still
    /// coming up, or it completes.
    pub(crate) working: bool,
}

pub(super) fn working(attempt: &Attempt, opening: Duration) -> bool {
    match &attempt.stage {
        Stage::Starting | Stage::Completing(_) => true,
        Stage::Asking { .. } => false,
        Stage::InBrowser { since, .. } => since.elapsed() < opening,
    }
}

pub(super) fn record(attempt: &Attempt) -> BrowserSignIn {
    let stage = match &attempt.stage {
        Stage::Starting => BrowserSignInStage::Starting,
        Stage::Asking { began } => BrowserSignInStage::Asking {
            unreached: began.unreached.clone(),
        },
        Stage::InBrowser { began, .. } => BrowserSignInStage::InBrowser {
            url: began.url.clone(),
            redirect_uri: began.redirect_uri.clone(),
        },
        Stage::Completing(_) => BrowserSignInStage::Completing,
    };
    let completing = matches!(stage, BrowserSignInStage::Completing);
    BrowserSignIn {
        attempt: attempt.id,
        account: attempt.account.clone(),
        stage,
        cancel: ControlState {
            enabled: !completing,
            running: false,
        },
    }
}

/// How long an Allodia sign-in runs before the first-run card owes the person a way back
/// (`docs/onboarding.md`): long enough that an ordinary hop, which puts the browser in front of
/// them within a second, never draws a button they had no reason to read.
pub(crate) const ESCAPE_AFTER: Duration = Duration::from_secs(8);

/// The Allodia account's sign-in out, as its card draws it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AllodiaOut {
    pub(crate) record: BrowserSignIn,
    /// It asks for the service's sign-up page.
    pub(crate) create: bool,
    /// The control that started it is running, as an account's is.
    pub(crate) working: bool,
    /// It has run for `ESCAPE_AFTER`.
    pub(crate) escape: bool,
}

impl super::BrowserSignIns {
    /// The Allodia account's sign-in, when it is the one out.
    pub(crate) fn allodia(&self) -> Option<AllodiaOut> {
        let slot = self.slot();
        let (opening, escape) = (
            slot.opening.unwrap_or(super::OPENING),
            slot.escape.unwrap_or(ESCAPE_AFTER),
        );
        let current = slot.current.as_ref()?;
        let Target::Allodia { create } = current.target else {
            return None;
        };
        Some(AllodiaOut {
            record: record(current),
            create,
            working: working(current, opening),
            escape: current.started.elapsed() >= escape,
        })
    }

    /// Which Allodia sign-in is out and at which stage, to tell whether its card moved.
    pub(crate) fn allodia_key(&self) -> Option<(u64, u8)> {
        let slot = self.slot();
        let current = slot
            .current
            .as_ref()
            .filter(|current| matches!(current.target, Target::Allodia { .. }))?;
        let stage = match current.stage {
            Stage::Starting => 0,
            Stage::Asking { .. } => 1,
            Stage::InBrowser { .. } => 2,
            Stage::Completing(_) => 3,
        };
        Some((current.id, stage))
    }

    /// Whether `attempt` is the Allodia account's sign-in out.
    pub(crate) fn is_allodia(&self, attempt: u64) -> bool {
        self.slot().current.as_ref().is_some_and(|current| {
            current.id == attempt && matches!(current.target, Target::Allodia { .. })
        })
    }

    /// How long an Allodia sign-in runs before the first-run card draws its way back.
    pub(crate) fn escape_after(&self) -> Duration {
        self.slot().escape.unwrap_or(ESCAPE_AFTER)
    }

    #[cfg(test)]
    pub(crate) fn set_escape_after(&self, escape: Duration) {
        self.slot().escape = Some(escape);
    }
}
