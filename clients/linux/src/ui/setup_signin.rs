//! What each setup pane asks the user for, once its server has answered.
//!
//! Split from [`super::setup`], which had reached the size limit, along the seam the two
//! sides already had: that module owns the window (which phase, which form, which error),
//! this one owns the pre-flights' answers and what they change on screen.
//!
//! One rule is shared by both protocols and is the reason this is worth its own file: only
//! the **first** answer for a given account counts. A deadline races each probe, so a late
//! real answer arrives at a card the user may already be acting on, and applying it would
//! rebuild the pane under them, taking a half-typed secret with it.

use super::{Phase, SetupState};
use crate::ui::setup_model::{AccountKind, DetectedForm, ImapSignIn, JmapSignIn, SetupForm};

impl SetupState {
    /// The pre-flight's answer, applied to whichever pane asked for it; the detected card or
    /// the manual form. Returns whether it belonged to what is on screen.
    ///
    /// Only the **first** answer for an address counts, which is what lets a deadline race the
    /// probe: whichever arrives first decides, and the loser finds a state that is no longer
    /// `Checking` and is dropped.
    pub(in crate::ui) fn jmap_oauth_available(
        &mut self,
        email: &str,
        server_url: &str,
        available: bool,
    ) -> bool {
        if self.phase != Phase::Form {
            return false;
        }
        let answer = if available {
            JmapSignIn::Offered
        } else {
            JmapSignIn::Unavailable
        };
        let rebuild = match self.form.as_mut() {
            Some(SetupForm::Detected(DetectedForm::Jmap(form)))
                if form.email == email
                    && form.server_url == server_url
                    && form.sign_in == JmapSignIn::Checking =>
            {
                form.sign_in = answer;
                // The card shows neither the offer nor a secret field while it asks, so both
                // answers change what is on screen.
                true
            }
            Some(SetupForm::Manual(form))
                if form.kind == AccountKind::Jmap
                    && form.email == email
                    && form.jmap_server == server_url
                    && form.sign_in == JmapSignIn::Checking =>
            {
                form.sign_in = answer;
                // The manual pane's secret field is already there and stays either way; only an
                // offer is new. Rebuilding on a negative answer would erase a secret being typed
                // to say nothing.
                available
            }
            _ => return false,
        };
        if rebuild {
            self.bump();
        }
        true
    }

    pub(in crate::ui) fn jmap_sign_in_failed(&mut self) {
        if let Some(sign_in) = self.any_jmap_sign_in() {
            *sign_in = JmapSignIn::Failed;
            self.phase = Phase::Form;
            self.error = None;
            self.bump();
        }
    }

    fn any_jmap_sign_in(&mut self) -> Option<&mut JmapSignIn> {
        match self.form.as_mut()? {
            SetupForm::Detected(DetectedForm::Jmap(form)) => Some(&mut form.sign_in),
            SetupForm::Manual(form) if form.kind == AccountKind::Jmap => Some(&mut form.sign_in),
            _ => None,
        }
    }

    /// The IMAP pre-flight's answer, applied to whichever pane asked for it. Returns whether
    /// it belonged to what is on screen.
    ///
    /// Only the **first** answer for a server counts, which is what lets the deadline race the
    /// probe: whichever arrives first decides, and the loser finds a state that is no longer
    /// `Checking` and is dropped.
    pub(in crate::ui) fn imap_auth_answered(
        &mut self,
        email: &str,
        imap_host: &str,
        offer: mailcal_bindings::ImapAuthOffer,
    ) -> bool {
        if self.phase != Phase::Form {
            return false;
        }
        let answer = ImapSignIn::from(offer);
        let rebuild = match self.form.as_mut() {
            Some(SetupForm::Detected(DetectedForm::Imap(form)))
                if form.email == email
                    && form.imap_host == imap_host
                    && form.sign_in == ImapSignIn::Checking =>
            {
                form.sign_in = answer;
                // The card shows neither a button nor a password field while it asks, so
                // every answer changes what is on screen.
                true
            }
            Some(SetupForm::Manual(form))
                if form.kind == AccountKind::Imap
                    && form.email == email
                    && form.imap_host == imap_host
                    && form.imap_sign_in == ImapSignIn::Checking =>
            {
                let offers = answer.show_offer() || answer.explains_registration();
                form.imap_sign_in = answer;
                // The manual pane's password field is already there and stays either way, so
                // only something *new* is worth a rebuild. Rebuilding on "a password, as
                // before" would erase a password being typed to say nothing.
                offers
            }
            _ => return false,
        };
        if rebuild {
            self.bump();
        }
        true
    }

    pub(in crate::ui) fn imap_sign_in_failed(&mut self) {
        if let Some(sign_in) = self.any_imap_sign_in() {
            *sign_in = ImapSignIn::Failed;
            self.phase = Phase::Form;
            self.error = None;
            self.bump();
        }
    }

    fn any_imap_sign_in(&mut self) -> Option<&mut ImapSignIn> {
        match self.form.as_mut()? {
            SetupForm::Detected(DetectedForm::Imap(form)) => Some(&mut form.sign_in),
            SetupForm::Manual(form) if form.kind == AccountKind::Imap => {
                Some(&mut form.imap_sign_in)
            }
            _ => None,
        }
    }
}
