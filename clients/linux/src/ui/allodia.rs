//! Signing in to an **Allodia account**: the browser half, and the model actions that drive it.
//!
//! The shared core owns the whole OAuth state machine; discovery, PKCE, the exchange, the identity
//! lookup and the write to the secret store; so this file owns only the four steps every desktop
//! browser sign-in here takes: bind the loopback the redirect comes back to, open the system
//! browser, wait for the callback off the main thread, hand it to the core. The shape of
//! [`super::microsoft`] plus its half of [`super::oauth_actions`], kept in one file because it is
//! one small flow that touches none of the setup wizard's state.
//!
//! An Allodia account is not a mail account: it carries no mailbox, appears in no switcher, and a
//! token issued for it cannot touch anyone's mail. Its screen is Settings → Accounts, and the setup
//! wizard never offers it.
//!
//! Linux claims no URI scheme, so the redirect is a bounded loopback listener; the same shape the
//! Google Desktop and Microsoft flows use here. The account service therefore has to match the
//! **loopback host** rather than an exact port, which is [RFC 8252 §7.3]'s own exemption.
//!
//! [RFC 8252 §7.3]: https://www.rfc-editor.org/rfc/rfc8252#section-7.3

use std::sync::{Arc, atomic::AtomicBool};

use mailcal_bindings::{AllodiaSignInStart, MailcalApp};

use super::{
    AppInput, AppModel,
    oauth_loopback::{self, CallbackOutcome, OAuthLoopback},
    settings,
    setup_onboarding::Progress,
};
use crate::l10n;

/// How a sign-in ended. `Failed` carries text that is **ready to show**: the two host-side causes
/// are whole sentences of their own, and only the core's terse error goes through the "signing in
/// didn't work" wrapper: nesting one inside the other reads as an apology repeated twice.
#[derive(Debug)]
pub(crate) enum AllodiaOutcome {
    SignedIn,
    Cancelled,
    Failed(String),
}

fn begin(
    app: &Arc<MailcalApp>,
    create: bool,
) -> Result<(OAuthLoopback, AllodiaSignInStart), String> {
    let loopback =
        OAuthLoopback::bind().map_err(|_| l10n::settings_allodia_browser_failed().to_owned())?;
    let redirect = loopback.redirect_uri();
    let start = if create {
        app.begin_allodia_registration(redirect)
    } else {
        app.begin_allodia_sign_in(redirect)
    }
    .map_err(|error| l10n::settings_allodia_failed(&error.to_string()))?;
    Ok((loopback, start))
}

/// Waits for the redirect on the port this attempt bound.
///
/// No state matching, unlike the JMAP flow: that one reuses a single port for the whole process, so
/// a stale callback can reach a later attempt. Here every attempt binds its own, and the attempt
/// slot drops a superseded completion anyway.
fn wait(loopback: OAuthLoopback, cancel: &AtomicBool) -> CallbackOutcome {
    loopback.wait(
        cancel,
        l10n::settings_allodia_timeout(),
        l10n::settings_allodia_browser_failed(),
    )
}

fn complete(app: &Arc<MailcalApp>, pending: String, callback_url: String) -> AllodiaOutcome {
    match app.complete_allodia_sign_in(pending, callback_url) {
        // The core stores the grant through the host's `AccountCredentialStore` before it reports
        // success, so there is nothing for the client to persist.
        Ok(_) => AllodiaOutcome::SignedIn,
        Err(error) => AllodiaOutcome::Failed(l10n::settings_allodia_failed(&error.to_string())),
    }
}

impl AppModel {
    pub(super) fn start_allodia_sign_in(&mut self, sender: relm4::Sender<AppInput>) {
        self.start_allodia(sender, false);
    }

    /// Both entry points. `create` asks the service for its sign-up page instead of its sign-in
    /// one; the redirect, the exchange and the store are identical, so nothing downstream needs to
    /// know which started it.
    pub(super) fn start_allodia(&mut self, sender: relm4::Sender<AppInput>, create: bool) {
        let Some(app) = self.app.clone() else {
            return;
        };
        if self.settings.allodia_signing_in {
            return;
        }
        let (attempt, cancel) = self.host_tasks.allodia.start();
        let (loopback, start) = match begin(&app, create) {
            Ok(started) => started,
            Err(error) => {
                self.host_tasks.allodia.finish(attempt);
                self.settings.allodia_failure = Some(error);
                self.refresh_account_settings();
                return;
            }
        };
        self.settings.allodia_failure = None;
        self.settings.allodia_signing_in = true;
        self.settings.allodia_sign_in_slow = false;
        // The first-run card's way back, armed rather than drawn: a hop that comes straight back
        // never puts a button in front of somebody who had no reason to read it.
        let slow = sender.clone();
        gtk::glib::timeout_add_local_once(
            super::setup_onboarding::SIGN_IN_ESCAPE_AFTER,
            move || {
                slow.emit(AppInput::AllodiaSignInSlow(attempt));
            },
        );
        // The launch reports failure through the portal, later and on the main context, so the
        // card goes to its pending state now and is put back by the same input the wait thread
        // would have used. The attempt slot keeps the two from both landing.
        let failed = sender.clone();
        oauth_loopback::launch_browser(&start.authorization_url, move || {
            failed.emit(AppInput::AllodiaSignInFinished(
                attempt,
                AllodiaOutcome::Failed(l10n::settings_allodia_browser_failed().to_owned()),
            ));
        });
        self.refresh_account_settings();
        std::thread::spawn(move || {
            let outcome = match wait(loopback, &cancel) {
                CallbackOutcome::Received(callback_url) => {
                    complete(&app, start.pending, callback_url)
                }
                CallbackOutcome::Cancelled => AllodiaOutcome::Cancelled,
                CallbackOutcome::Failed(error) => AllodiaOutcome::Failed(error),
            };
            sender.emit(AppInput::AllodiaSignInFinished(attempt, outcome));
        });
    }

    pub(super) fn allodia_sign_in_finished(
        &mut self,
        attempt: u64,
        outcome: AllodiaOutcome,
        sender: relm4::Sender<AppInput>,
    ) {
        if !self.host_tasks.allodia.finish(attempt) {
            // A superseded or cancelled attempt. Nothing of its own to publish over the newer one,
            // but a sign-in the user cancelled after the exchange had already run IS stored, so the
            // card is refreshed to show the account rather than still offering a sign-in.
            if matches!(outcome, AllodiaOutcome::SignedIn) {
                self.refresh_account_settings();
            }
            return;
        }
        self.settings.allodia_signing_in = false;
        self.settings.allodia_sign_in_slow = false;
        match outcome {
            AllodiaOutcome::SignedIn => {
                self.settings.allodia_failure = None;
                // The first thing a new sign-in is for: this device's accounts go up, and whatever
                // the person's other devices hold comes back.
                self.sync_allodia_accounts(sender);
            }
            // A dismissed browser is not a failure; say nothing about it.
            AllodiaOutcome::Cancelled => {}
            AllodiaOutcome::Failed(error) => self.settings.allodia_failure = Some(error),
        }
        self.refresh_account_settings();
    }

    pub(super) fn cancel_allodia_sign_in(&mut self) {
        if self.host_tasks.allodia.cancel() {
            self.settings.allodia_signing_in = false;
            self.settings.allodia_sign_in_slow = false;
            self.refresh_account_settings();
        }
    }

    /// The hop has outlasted the card's threshold, so the first-run card gains its way back.
    ///
    /// Keyed on the attempt: the timer outlives the sign-in it was armed for, and a later one
    /// started in the meantime has its own threshold to serve rather than inheriting this one's.
    pub(super) fn allodia_sign_in_slow(&mut self, attempt: u64) {
        if !self.host_tasks.allodia.holds(attempt) {
            return;
        }
        self.settings.allodia_sign_in_slow = true;
        self.refresh_account_settings();
    }

    /// Signs out: the core forgets the account and erases its stored grant. Local only, which is
    /// what removing a mail account is too; the grant stays alive at the service until it expires
    /// or the person revokes it there.
    pub(super) fn sign_out_of_allodia(&mut self) {
        let Some(app) = self.app.clone() else {
            return;
        };
        // The account is forgotten in memory whatever the store does, so the card re-reads from the
        // core rather than being cleared here: a delete that failed leaves the app signed out and
        // says why.
        match app.sign_out_of_allodia() {
            Ok(end_session) => {
                self.settings.allodia_failure = None;
                // Nothing left to say about other devices once this one leaves the account that
                // linked them.
                self.forget_allodia_sync();
                // Best-effort and deliberately unreported: this device is signed out whatever
                // happens to the browser. What it buys is the next sign-in asking who you are
                // rather than completing silently against a session someone thought they left.
                if let Some(url) = end_session {
                    oauth_loopback::launch_browser(&url, || {});
                }
            }
            Err(error) => {
                self.settings.allodia_failure =
                    Some(l10n::settings_allodia_failed(&error.to_string()));
            }
        }
        self.refresh_account_settings();
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

    /// Re-renders the Settings window on the Allodia page, which is how this card changes.
    ///
    /// A **refresh**, never an open: a redirect landing after the user closed Settings must not put
    /// the window back over their mail. Allodia is the page the sign-in was started from, so it is
    /// the page a browser hop coming back belongs on.
    ///
    /// Only for that. A change somebody made on a **different** page; the per-account sharing
    /// control lives on Accounts; takes
    /// [`refresh_settings_in_place`](Self::refresh_settings_in_place) instead, or answering it
    /// would move them off the page they answered it on.
    pub(super) fn refresh_account_settings(&mut self) {
        self.settings.refresh(settings::Category::Allodia);
        // The same state drives the first-run card, which is a different window and is the one on
        // screen when this matters most; the person has no account yet, so Settings is not open.
        self.refresh_onboarding_card();
    }

    /// The same redraw, leaving whichever page is open where it is.
    pub(super) fn refresh_settings_in_place(&mut self) {
        self.settings.refresh_in_place();
        self.refresh_onboarding_card();
    }

    /// Pushes what the first-run card should show into the setup window.
    ///
    /// Read from the core each time rather than mirrored: `allodia_account()` is a local read that
    /// never asks the service, and one copy of the answer cannot go stale against another.
    pub(super) fn refresh_onboarding_card(&mut self) {
        let signed_in = self
            .app
            .as_ref()
            .is_some_and(|app| app.allodia_account().is_some());
        let progress = if self.settings.allodia_signing_in {
            Progress::SigningIn {
                escapable: self.settings.allodia_sign_in_slow,
            }
        } else if !signed_in {
            Progress::Offering
        } else if self.settings.allodia_sync.checking {
            Progress::Checking
        } else {
            Progress::SignedIn
        };
        self.setup
            .set_onboarding(super::setup_onboarding::Onboarding {
                offered: mailcal_bindings::allodia_sign_in_available(),
                progress,
                failure: self.settings.allodia_failure.clone(),
                // `None` until a pass has answered, which is not the same as a pass that answered
                // with nothing: only the second one may be reported as an empty account.
                offers: self
                    .settings
                    .allodia_sync
                    .report
                    .as_ref()
                    .map(|report| report.offers.clone()),
            });
    }
}
