//! The Allodia account's card, kept by the core (`entitlement.md`, "Signing in";
//! `docs/onboarding.md`): who is signed in, whether the service refused that sign-in, the sign-in
//! out in the browser, and what the last sign-in or sign-out came to. Settings → Allodia account
//! and the first-run card both draw it, read with `MailcalApp::allodia_card` after every
//! `Surface::Allodia` signal; a client words it and keeps no step of its own.

use std::sync::Mutex;

use crate::{AccountChangeRefusal, AllodiaAccount, BrowserSignIn, ControlState};

/// What the Allodia account's card draws.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AllodiaCard {
    pub state: AllodiaCardState,
    /// The sign-in out, drawn as signing in with its Cancel (`AllodiaIntent::CancelSignIn`) in
    /// place of the card's buttons. The client's browser driver opens it as it opens an
    /// account's, from `MailcalApp::browser_sign_in`.
    pub browser: Option<BrowserSignIn>,
    /// Sign in, and Sign in again wherever a surface offers it: running while the sign-in it
    /// started is being started or completed.
    pub sign_in: ControlState,
    /// Create an account.
    pub create: ControlState,
    /// Sign out: running while the grant is erased, held while a sign-in is out.
    pub sign_out: ControlState,
    /// What the last sign-in or sign-out came to, when it did not work. A sign-in the person
    /// cancelled or whose browser they closed says nothing.
    pub failure: Option<AllodiaFailure>,
    /// Where to send the browser after the last sign-out, to end the service's own session:
    /// opened once for each `id`, best-effort.
    pub end_session: Option<AllodiaEndSession>,
    /// What the first-run card draws.
    pub onboarding: AllodiaOnboarding,
}

/// Whether someone is signed in.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum AllodiaCardState {
    SignedOut,
    SignedIn {
        account: AllodiaAccount,
    },
    /// The service refused the stored sign-in, as the `AllodiaSignInEnded` notice says: drawn
    /// as "your sign-in as … has ended" with Sign in again and Sign out, and no Manage account.
    Ended {
        account: AllodiaAccount,
    },
}

/// Why the last sign-in or sign-out did not work.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum AllodiaFailure {
    /// The sign-in did not complete.
    SignIn { refusal: AccountChangeRefusal },
    /// The stored sign-in could not be erased. This device is signed out all the same.
    SignOut { detail: String },
}

/// The service's own sign-out page, to open once.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AllodiaEndSession {
    pub id: u64,
    pub url: String,
}

/// How far the first-run card has got (`docs/onboarding.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum AllodiaOnboarding {
    /// Nobody is signed in: the recommendation.
    Offering,
    /// A sign-in is out. `escape` once it has run for eight seconds, when the card draws its
    /// way back (`AllodiaIntent::CancelSignIn`).
    SigningIn { escape: bool },
    /// Signed in, and the pass over the other devices that follows is running.
    Checking,
    /// Signed in: what the other devices hold is `MailcalApp::allodia_devices`.
    SignedIn,
}

/// What the card keeps between signals.
#[derive(Debug, Default)]
pub(crate) struct AllodiaWork {
    inner: Mutex<Inner>,
}

#[derive(Debug, Default)]
struct Inner {
    failure: Option<AllodiaFailure>,
    end_session: Option<AllodiaEndSession>,
    signing_out: bool,
    /// The sign-in out when the card was last signalled, and its stage.
    signalled: Option<(u64, u8)>,
}

impl AllodiaWork {
    fn inner(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().expect("allodia card mutex poisoned")
    }

    pub(crate) fn failure(&self) -> Option<AllodiaFailure> {
        self.inner().failure.clone()
    }

    /// What the last sign-in came to: `None` for one that worked or was started again.
    pub(crate) fn say(&self, failure: Option<AllodiaFailure>) {
        self.inner().failure = failure;
    }

    pub(crate) fn end_session(&self) -> Option<AllodiaEndSession> {
        self.inner().end_session.clone()
    }

    pub(crate) fn signing_out(&self) -> bool {
        self.inner().signing_out
    }

    /// Starts a sign-out; `false` while one runs.
    pub(crate) fn sign_out(&self) -> bool {
        let mut inner = self.inner();
        let started = !inner.signing_out;
        inner.signing_out = true;
        started
    }

    /// A sign-out ended: the service's page to open, or why the grant stayed stored.
    pub(crate) fn signed_out(&self, answer: Result<Option<String>, String>) {
        let mut inner = self.inner();
        inner.signing_out = false;
        match answer {
            Ok(url) => {
                inner.failure = None;
                let id = inner.end_session.as_ref().map_or(1, |last| last.id + 1);
                if let Some(url) = url {
                    inner.end_session = Some(AllodiaEndSession { id, url });
                }
            }
            Err(detail) => inner.failure = Some(AllodiaFailure::SignOut { detail }),
        }
    }

    /// Whether the sign-in out moved since the card was last signalled, noting `now`.
    pub(crate) fn moved(&self, now: Option<(u64, u8)>) -> bool {
        let mut inner = self.inner();
        let moved = inner.signalled != now;
        inner.signalled = now;
        moved
    }
}
