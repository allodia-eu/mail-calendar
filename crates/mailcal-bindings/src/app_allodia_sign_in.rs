//! The Allodia account's sign-in and sign-out, run by the core from the press to the grant it
//! stores or erases (`entitlement.md`, "Signing in"; `docs/waiting.md`). The sign-in is the one
//! browser sign-in the core holds, so a fresh press of any sign-in supersedes it and the client's
//! browser driver opens it and hands back its redirect as it does an account's; it is completed
//! on a thread of its own rather than as a change on a page, since it changes no account's page.

use std::sync::Arc;

use super::{Launch, Way};
use crate::{
    AccountChangeRefusal, AllodiaCard, AllodiaCardState, AllodiaFailure, AllodiaIntent,
    AllodiaOnboarding, ControlState, MailcalApp, MailcalError, NoticeAction,
    account_page::browser::{Began, Completion, Pending, Target},
    allodia::ACCOUNT_ID,
};

#[uniffi::export]
impl MailcalApp {
    /// The Allodia account's card: read again after every `Surface::Allodia` signal, and for the
    /// first-run card's progress after every `Surface::OtherDevices` one. Local and cheap.
    #[must_use]
    pub fn allodia_card(&self) -> AllodiaCard {
        let out = self.account_work.browser.allodia();
        let state = match self.allodia_account() {
            None => AllodiaCardState::SignedOut,
            Some(account) if self.allodia_sign_in_ended().is_some() => {
                AllodiaCardState::Ended { account }
            }
            Some(account) => AllodiaCardState::SignedIn { account },
        };
        let signing_out = self.account_work.allodia.signing_out();
        let working = out.as_ref().filter(|out| out.working);
        let control = |create: bool| ControlState {
            enabled: !signing_out && working.is_none(),
            running: working.is_some_and(|out| out.create == create),
        };
        let onboarding = match (&out, &state) {
            (Some(out), _) => AllodiaOnboarding::SigningIn { escape: out.escape },
            (None, AllodiaCardState::SignedOut) => AllodiaOnboarding::Offering,
            (None, _)
                if self
                    .allodia_devices()
                    .is_some_and(|devices| devices.checking) =>
            {
                AllodiaOnboarding::Checking
            }
            (None, _) => AllodiaOnboarding::SignedIn,
        };
        AllodiaCard {
            sign_in: control(false),
            create: control(true),
            sign_out: ControlState {
                enabled: out.is_none() && !signing_out,
                running: signing_out,
            },
            browser: out.map(|out| out.record),
            state,
            failure: self.account_work.allodia.failure(),
            end_session: self.account_work.allodia.end_session(),
            onboarding,
        }
    }
}

impl MailcalApp {
    /// Handles a change to the Allodia account's sign-in, which returns at once.
    pub(crate) fn dispatch_allodia(&self, intent: AllodiaIntent) {
        let Some(app) = self.rediscovery.app() else {
            log::warn!("allodia: a change was asked for while the app shuts down");
            return;
        };
        match intent {
            AllodiaIntent::SignIn {
                create,
                redirect_uri,
            } => {
                let sign_in = Launch {
                    account: ACCOUNT_ID.to_owned(),
                    target: Target::Allodia { create },
                    way: Way::Allodia { create },
                    adding: Vec::new(),
                    switching: false,
                };
                app.launch(sign_in, redirect_uri);
                self.signal_sign_in();
            }
            AllodiaIntent::CancelSignIn { attempt } => {
                if self.account_work.browser.is_allodia(attempt) {
                    self.account_work.browser.end(attempt);
                    self.signal_sign_in();
                }
            }
            AllodiaIntent::SignOut => app.sign_out(),
        }
    }

    /// Asks the service to start the Allodia sign-in, returning to `redirect_uri`.
    pub(super) fn begin_allodia_way(
        &self,
        redirect_uri: String,
        create: bool,
    ) -> Result<Began, MailcalError> {
        self.begin_allodia(redirect_uri.clone(), create)
            .map(|start| Began {
                pending: Pending::Allodia(start.pending),
                url: start.authorization_url,
                redirect_uri: Some(redirect_uri),
                unreached: Vec::new(),
            })
    }

    /// `attempt` has started: what the last one said goes, and the first-run card draws its way
    /// back once it has run long enough to need one.
    pub(super) fn allodia_began(app: &Arc<Self>, attempt: u64) {
        app.account_work.allodia.say(None);
        let app = Arc::clone(app);
        std::thread::spawn(move || {
            std::thread::sleep(app.account_work.browser.escape_after());
            if app.account_work.browser.is_allodia(attempt) {
                app.app.allodia_changed();
            }
        });
    }

    /// Completes `attempt` with what its browser returned: the grant is stored, or the card says
    /// why not.
    pub(super) fn complete_allodia(&self, attempt: u64) {
        let browser = &self.account_work.browser;
        let Some(run) = browser.take(attempt) else {
            return;
        };
        let answer = match run.completion {
            Completion::Allodia {
                pending,
                callback_url,
            } => self.complete_allodia_sign_in(pending, callback_url),
            _ => Err(MailcalError::Config("not an Allodia sign-in".to_owned())),
        };
        browser.completed(attempt, None);
        if let Err(error) = answer {
            log::info!("allodia: the browser sign-in did not complete");
            self.allodia_ended(AccountChangeRefusal::from(&error));
        }
        self.signal_sign_in();
    }

    /// Says on the card why the Allodia sign-in did not complete, and on the notice whose remedy
    /// it was.
    pub(super) fn allodia_ended(&self, refusal: AccountChangeRefusal) {
        self.account_work
            .allodia
            .say(Some(AllodiaFailure::SignIn { refusal }));
        self.remedy_failed(NoticeAction::SignInToAllodia, None);
    }

    /// Signs out on a thread of its own: the platform store is written. Held while a sign-in is
    /// out, and pressed twice runs once.
    fn sign_out(self: Arc<Self>) {
        if self.account_work.browser.allodia().is_some() || !self.account_work.allodia.sign_out() {
            return;
        }
        self.app.allodia_changed();
        std::thread::spawn(move || {
            let answer = self
                .sign_out_of_allodia()
                .map_err(|error| error.to_string());
            self.account_work.allodia.signed_out(answer);
            self.app.allodia_changed();
        });
    }
}

#[cfg(test)]
#[path = "app_allodia_sign_in_tests.rs"]
mod tests;
