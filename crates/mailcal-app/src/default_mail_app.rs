//! Whether to offer to become the OS's default mail app, and remembering the answer.
//!
//! The platform half of this feature is one call per platform and cannot be shared: macOS sets
//! the handler, Windows and iOS open their own settings, Linux and Android can do neither
//! (`docs/os-integration.md`). What *is* shared, and lives here, is the decision to ask at all,
//! so no client invents its own idea of when a good moment is.
//!
//! Three conditions, and each exists because its absence produced a bad prompt somewhere in
//! this class of feature:
//!
//! - **Not before there is an account.** On a first launch the app cannot yet send mail, so asking
//!   to be the mail app is asking for a commitment to something the person has not seen.
//! - **Not when it is already true.** A prompt to do what has been done reads as broken.
//! - **Once.** Answered or dismissed, the offer is spent; the way back is Settings → General, which
//!   is always there. An app that asks twice is an app that will ask a third time.
//!
//! Like the sibling settings states ([`crate::swipe_settings`], [`crate::quote_settings`]) the
//! answer lives in the shared preferences file, written read-modify-write so the neighbouring
//! preferences survive. It is a second `impl App` block for the reason they all are: `lib.rs`
//! stays under the 500-line limit.

use engine_api::Provider;
use mailcal_account::{load_preferences, save_preferences};
use mailcal_viewmodel::{DefaultMailAppOutcome, DefaultMailAppSupport};

use crate::{App, Surface};

impl<P: Provider> App<P> {
    /// Whether to put the one-time offer to become the default mail app in front of the user
    /// now.
    ///
    /// `support` is what this build can actually do about it, and `is_default` what the host
    /// was able to find out, with `None` for a platform that cannot tell (a Flatpak has no
    /// host application database to ask). An unknown answer is treated as "not default":
    /// offering where we need not is recoverable, staying silent where we are not the default
    /// is the state this feature exists to change.
    ///
    /// A host asks once, after the account list has loaded. It never has to remember the
    /// answer itself; that is what [`App::record_default_mail_app_offer`] is for.
    pub async fn should_offer_default_mail_app(
        &self,
        support: DefaultMailAppSupport,
        is_default: Option<bool>,
    ) -> bool {
        if support == DefaultMailAppSupport::Unsupported || is_default == Some(true) {
            return false;
        }
        if self.account_ids().await.is_empty() {
            return false;
        }
        self.offer_state().is_none()
    }

    /// Records what came of the offer, so it is never put again.
    ///
    /// Both outcomes end it. A dismissed prompt arrives here as
    /// [`DefaultMailAppOutcome::Declined`]: someone who closed the question without answering
    /// it has still answered it, and asking again is how a prompt becomes nagging.
    // `async` with no inner `await` is intentional: every dispatched command method shares one
    // async shape so `dispatch` and the FFI adapter drive them uniformly.
    #[allow(clippy::unused_async)]
    pub async fn record_default_mail_app_offer(&self, outcome: DefaultMailAppOutcome) {
        let accepted = outcome == DefaultMailAppOutcome::Accepted;
        if let Some(path) = &self.prefs_path {
            let mut prefs = load_preferences(path);
            prefs.default_mail_app_offer = Some(accepted);
            let _ = save_preferences(path, &prefs);
        }
        log::info!("default mail app offer answered: accepted={accepted}");
        self.observer.surface_changed(Surface::Settings);
    }

    /// What came of the offer, or `None` if it has not been put yet. The Settings row reads
    /// this to say where things stand; the action it offers is the same either way.
    #[must_use]
    pub fn default_mail_app_offer(&self) -> Option<bool> {
        self.offer_state()
    }

    /// The persisted answer, read straight from the preferences file.
    ///
    /// Deliberately not cached on [`App`]: it is read at most a few times per launch (once at
    /// boot, once per visit to Settings), so a field and its mutex would cost more than the
    /// read does. The in-memory demo and the tests have no path and behave as never-offered.
    fn offer_state(&self) -> Option<bool> {
        self.prefs_path
            .as_ref()
            .and_then(|path| load_preferences(path).default_mail_app_offer)
    }
}
