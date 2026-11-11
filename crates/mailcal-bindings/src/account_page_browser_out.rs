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
