//! Signing in to an **Allodia account**, which the core runs (`docs/waiting.md`): a press sends
//! the loopback this process listens on, the browser driver opens the sign-in and hands back what
//! returns as it does an account's ([`super::browser_sign_in`]), and both cards are drawn from the
//! core's `allodia_card`. What is left here is the core's signal and the service's own pages: the
//! one a sign-out names and Manage account.
//!
//! An Allodia account is not a mail account: it carries no mailbox, appears in no switcher, and a
//! token issued for it cannot touch anyone's mail. Its screen is Settings → Allodia account, and
//! the first-run card offers it too.

use mailcal_bindings::{AllodiaCardState, AllodiaIntent, AllodiaOnboarding, Intent};

use super::{AppModel, oauth_loopback, settings, setup_onboarding::Progress};

impl AppModel {
    /// Signs in, or with `create` creates an account first, from wherever the person pressed.
    pub(super) fn start_allodia(&mut self, create: bool) {
        let redirect_uri = self.loopback_redirect();
        self.dispatch(Intent::Allodia {
            intent: AllodiaIntent::SignIn {
                create,
                redirect_uri,
            },
        });
    }

    /// Stops the sign-in out, which then says nothing.
    pub(super) fn cancel_allodia_sign_in(&mut self) {
        let Some(attempt) = self
            .app
            .as_ref()
            .and_then(|app| app.allodia_card().browser)
            .map(|browser| browser.attempt)
        else {
            return;
        };
        self.dispatch(Intent::Allodia {
            intent: AllodiaIntent::CancelSignIn { attempt },
        });
    }

    /// Signs out: the core erases the stored grant on a thread of its own, and the card names
    /// the service's page that ends its session.
    pub(super) fn sign_out_of_allodia(&mut self) {
        self.dispatch(Intent::Allodia {
            intent: AllodiaIntent::SignOut,
        });
    }

    /// The core's Allodia signal: the cards read the card again, the service's sign-out page
    /// opens once, and the subscription goes with the account it was read for.
    pub(super) fn allodia_signalled(&mut self) {
        let Some(app) = self.app.clone() else {
            return;
        };
        let card = app.allodia_card();
        if let Some(end) = card
            .end_session
            .filter(|end| end.id > self.settings.allodia_end_session)
        {
            self.settings.allodia_end_session = end.id;
            // Best-effort and unreported: this device is signed out whatever the browser does.
            // What it buys is the next sign-in asking who you are rather than completing silently
            // against a session someone thought they left.
            oauth_loopback::launch_browser(&end.url, || {});
        }
        let seen = match card.state {
            AllodiaCardState::SignedIn { account } | AllodiaCardState::Ended { account } => {
                Some(account.email)
            }
            AllodiaCardState::SignedOut => None,
        };
        let signing_in = card.browser.is_some();
        let signed_in_again = self.settings.allodia_signing_in && !signing_in;
        self.settings.allodia_signing_in = signing_in;
        // What was read was about another account, or about none, or about the grant a sign-in
        // just replaced, which may have been refused or too narrow.
        if seen != self.settings.allodia_seen || signed_in_again {
            self.settings.allodia_seen = seen;
            self.forget_allodia_subscription();
        }
        self.settings
            .refresh_if_showing(settings::Category::Allodia);
        self.refresh_onboarding_card();
    }

    /// Opens the service's own account page, where someone changes their details or deletes the
    /// account. A page, not a flow: nothing is pending and nothing comes back.
    pub(super) fn manage_allodia_account(&mut self) {
        let Some(app) = self.app.clone() else {
            return;
        };
        let Some(url) = app.allodia_account_url() else {
            return;
        };
        oauth_loopback::launch_browser(&url, || {});
    }

    /// Pushes what the first-run card should show into the setup window.
    ///
    /// Read from the core each time rather than mirrored: `allodia_account()` is a local read that
    /// never asks the service, and one copy of the answer cannot go stale against another.
    pub(super) fn refresh_onboarding_card(&mut self) {
        let Some(app) = self.app.clone() else {
            return;
        };
        let card = app.allodia_card();
        let devices = app.allodia_devices();
        let progress = match card.onboarding {
            AllodiaOnboarding::Offering => Progress::Offering,
            AllodiaOnboarding::SigningIn { escape } => Progress::SigningIn { escapable: escape },
            AllodiaOnboarding::Checking => Progress::Checking,
            AllodiaOnboarding::SignedIn => Progress::SignedIn,
        };
        self.setup
            .set_onboarding(super::setup_onboarding::Onboarding {
                offered: mailcal_bindings::allodia_sign_in_available(),
                progress,
                failure: card.failure.as_ref().map(settings::allodia_failure_said),
                // `None` until a pass has answered, which is not the same as a pass that answered
                // with nothing: only the second one may be reported as an empty account.
                offers: devices
                    .filter(|devices| devices.nothing_found || !devices.offers.is_empty())
                    .map(|devices| devices.offers),
            });
    }
}
