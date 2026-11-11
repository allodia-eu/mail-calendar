//! A sign-in in the person's browser, held by the core from the press that starts it to the
//! account it changes (`docs/waiting.md`, `docs/provider-oauth.md` rule 13). A client opens the
//! browser, listens where it returns and hands back what came; which step the sign-in is at, what
//! it is waiting for and what it came to are the core's.

use crate::{AccountCapability, ControlState};

/// The browser sign-in the core is running: at most one at a time, since a fresh press
/// supersedes the one still out. Read with `MailcalApp::browser_sign_in` after every
/// `Surface::AccountPage` signal.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct BrowserSignIn {
    /// Grows with each sign-in started. A client opens the browser once for each, and names it
    /// when it hands back what the browser did.
    pub attempt: u64,
    /// The account it signs in.
    pub account: String,
    pub stage: BrowserSignInStage,
    /// Its Cancel, sent as `AccountPageIntent::CancelSignIn`: held once the sign-in is being
    /// completed, which no longer waits on the person.
    pub cancel: ControlState,
}

/// Where a browser sign-in is.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum BrowserSignInStage {
    /// The core asks the server to start one: discovery and registration, which can take a few
    /// seconds.
    Starting,
    /// Switching to the provider would cost these uses (`docs/accounts.md` rule 18): the person
    /// answers `AccountPageIntent::AnswerUnreached` before the browser opens.
    Asking { unreached: Vec<AccountCapability> },
    /// Waiting on the person in the browser, which the client opens at `url` once for this
    /// attempt. `redirect_uri` is where the browser returns, which the client listens on and
    /// hands back with `AccountPageIntent::SignInBrowserEnded`; `None` where nothing returns,
    /// because the core asks the server itself whether the person has finished
    /// (`docs/nextcloud-sign-in.md` rule 5).
    InBrowser {
        url: String,
        redirect_uri: Option<String>,
    },
    /// The core is completing it: exchanging what the browser returned and connecting with it,
    /// as a change on the page.
    Completing,
}

/// What became of the browser a client opened for a sign-in.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum SignInBrowserEnd {
    /// It returned to the redirect, with this URL.
    Redirected { callback_url: String },
    /// The person closed it. A sign-in that returns nothing is asked once more whether the
    /// person finished before it ends.
    Closed,
    /// It could not be opened.
    NotOpened,
}
