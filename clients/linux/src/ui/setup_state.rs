//! What the account-setup window is showing, and what each pane renders from.
//!
//! Split from [`super::setup`], which is the window itself: the state outgrew the file, and the
//! two answer different questions. Nothing here draws.

use super::{
    setup_model::{ImapSignIn, JmapSignIn, ManualForm, SetupForm, edit_manually},
    setup_onboarding::Onboarding,
};

/// The pre-flight a freshly shown manual form owes, if any.
fn manual_probe(form: &SetupForm) -> Option<ManualForm> {
    match form {
        SetupForm::Manual(manual) if manual.probes_jmap_sign_in() => Some(manual.clone()),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Phase {
    Email,
    Detecting,
    Form,
    GoogleSigningIn,
    MicrosoftSigningIn,
    JmapSigningIn,
    ImapSigningIn,
    Connecting,
}

pub(super) struct SetupState {
    pub(super) visible: bool,
    pub(super) required: bool,
    pub(super) generation: u64,
    /// Bumped only when the pane on screen must be *rebuilt*: a different form, a different
    /// account type, a different phase. A connect and its answer leave it alone, because the
    /// person's fields live in that pane's widgets and rebuilding it would throw them away
    /// (`super::setup_pane`).
    pub(super) form_generation: u64,
    pub(super) phase: Phase,
    pub(super) form: Option<SetupForm>,
    pub(super) error: Option<String>,
    /// The certificate the last connect was refused for, when that is why it failed. Cleared
    /// with `error`, so a panel never outlives the failure that raised it
    /// (`docs/certificate-exceptions.md`).
    pub(super) certificate: Option<mailcal_bindings::RejectedCertificate>,
    /// A certificate already accepted during this setup. Kept apart from `certificate`, which
    /// is a question: this is the answer, and it outlives a retry that then fails on the
    /// password so nobody is asked the same thing twice.
    pub(super) accepted_certificate: Option<mailcal_bindings::RejectedCertificate>,
    /// The address an account offered by one of the person's other devices is for. It only fills
    /// the field: which route the flow takes is still decided by detection, so an offer whose
    /// settings have since moved is corrected rather than believed.
    pub(super) start_email: String,
    /// The first-run Allodia recommendation's state ([`super::setup_onboarding`]). Held here
    /// because a sign-in outlives several window rebuilds.
    pub(super) onboarding: Onboarding,
}

impl SetupState {
    pub(super) const fn closed() -> Self {
        Self {
            visible: false,
            required: false,
            generation: 0,
            phase: Phase::Email,
            form: None,
            error: None,
            form_generation: 0,
            certificate: None,
            accepted_certificate: None,
            start_email: String::new(),
            onboarding: Onboarding::new(),
        }
    }

    /// Replaces what the first-run card shows, redrawing the window if it is open.
    ///
    /// Only while the **email step** is on screen: the card belongs above the address field, and a
    /// redraw pushed onto a later step would take a half-filled form away.
    pub(super) fn set_onboarding(&mut self, onboarding: Onboarding) {
        self.onboarding = onboarding;
        if self.visible && self.phase == Phase::Email {
            self.bump();
        }
    }

    pub(super) fn open(&mut self, required: bool) {
        self.open_on(required, String::new());
    }

    /// Opens on an address, for an offer from one of the person's other devices.
    pub(super) fn open_on(&mut self, required: bool, start_email: String) {
        self.visible = true;
        self.required = required;
        self.phase = Phase::Email;
        self.form = None;
        self.error = None;
        self.certificate = None;
        // A fresh flow asks its own questions; nothing an earlier one answered carries over.
        self.accepted_certificate = None;
        self.start_email = start_email;
        self.bump();
    }

    pub(super) fn detecting(&mut self) {
        self.phase = Phase::Detecting;
        self.error = None;
        self.certificate = None;
        self.bump();
    }

    pub(super) fn show_form(&mut self, form: SetupForm) {
        self.phase = Phase::Form;
        self.form = Some(form);
        self.error = None;
        self.certificate = None;
        self.bump();
    }

    /// The detected route the user asked to edit by hand, as its own account type prefilled.
    /// Returns whether the manual pane it opens still owes a JMAP sign-in pre-flight.
    pub(super) fn edit_detected_manually(&mut self) -> Option<ManualForm> {
        let Some(SetupForm::Detected(detected)) = self.form.as_ref() else {
            return None;
        };
        let form = edit_manually(detected);
        let probe = manual_probe(&form);
        self.show_form(form);
        probe
    }

    /// A new account type on the manual form, carrying across whatever was already typed.
    pub(super) fn select_account_kind(&mut self, form: ManualForm) -> Option<ManualForm> {
        let form = SetupForm::Manual(ManualForm {
            // A type the user has just switched to has not been asked about yet.
            sign_in: JmapSignIn::Checking,
            imap_sign_in: ImapSignIn::Checking,
            ..form
        });
        let probe = manual_probe(&form);
        self.show_form(form);
        probe
    }

    /// Records what the manual JMAP pane holds now and answers whether that address still needs
    /// a pre-flight. Deliberately does **not** rebuild: nothing on screen changes when a probe
    /// starts, and a rebuild would take the secret the user may already be typing.
    pub(super) fn adopt_manual_jmap(&mut self, typed: ManualForm) -> Option<ManualForm> {
        if self.phase != Phase::Form {
            return None;
        }
        let Some(SetupForm::Manual(current)) = self.form.as_ref() else {
            return None;
        };
        // Same address, nothing to ask: either the pre-flight is still in flight for it or it
        // has already answered. Leaving the field a second time must not spend another round
        // trip, and a re-probe of an in-flight address would answer twice.
        if current.email == typed.email && current.jmap_server == typed.jmap_server {
            return None;
        }
        let form = ManualForm {
            sign_in: JmapSignIn::Checking,
            ..typed
        };
        let probe = form.probes_jmap_sign_in().then(|| form.clone());
        self.form = Some(SetupForm::Manual(form));
        probe
    }

    /// Records what the manual IMAP pane holds now and answers whether that server still
    /// needs a pre-flight. Deliberately does **not** rebuild: nothing on screen changes when a
    /// probe starts, and a rebuild would take the password the user may already be typing.
    pub(super) fn adopt_manual_imap(&mut self, typed: ManualForm) -> Option<ManualForm> {
        if self.phase != Phase::Form {
            return None;
        }
        let Some(SetupForm::Manual(current)) = self.form.as_ref() else {
            return None;
        };
        // Same account, nothing to ask: either the pre-flight is in flight for it or it has
        // already answered. Leaving a field a second time must not spend another dial.
        if current.email == typed.email && current.imap_host == typed.imap_host {
            return None;
        }
        let form = ManualForm {
            imap_sign_in: ImapSignIn::Checking,
            ..typed
        };
        let probe = form.probes_imap_sign_in().then(|| form.clone());
        self.form = Some(SetupForm::Manual(form));
        probe
    }

    pub(super) fn connecting(&mut self) {
        self.phase = Phase::Connecting;
        self.error = None;
        self.certificate = None;
        self.report();
    }

    pub(super) fn google_signing_in(&mut self) {
        self.phase = Phase::GoogleSigningIn;
        self.error = None;
        self.certificate = None;
        self.bump();
    }

    pub(super) fn microsoft_signing_in(&mut self) {
        self.phase = Phase::MicrosoftSigningIn;
        self.error = None;
        self.certificate = None;
        self.bump();
    }

    pub(super) fn jmap_signing_in(&mut self) {
        self.phase = Phase::JmapSigningIn;
        self.error = None;
        self.certificate = None;
        self.bump();
    }

    pub(super) fn imap_signing_in(&mut self) {
        self.phase = Phase::ImapSigningIn;
        self.error = None;
        self.certificate = None;
        self.bump();
    }

    pub(super) fn retry_form(&mut self) {
        self.phase = Phase::Form;
        self.error = None;
        self.certificate = None;
        self.bump();
    }

    pub(super) fn failed(&mut self, error: String) {
        self.connect_failed(super::setup_model::ConnectFailure {
            message: Some(error),
            certificate: None,
        });
    }

    /// The certificate accepted so far in this setup, if any.
    pub(super) fn accepted_certificate(&self) -> Option<mailcal_bindings::RejectedCertificate> {
        self.accepted_certificate.clone()
    }

    /// Records one, so the rest of this setup carries it without asking again.
    pub(super) fn remember_accepted_certificate(
        &mut self,
        certificate: mailcal_bindings::RejectedCertificate,
    ) {
        self.accepted_certificate = Some(certificate);
    }

    /// The same, for the one path that can also carry the certificate a connect was refused
    /// for, which an IMAP pane offers to accept (`docs/certificate-exceptions.md`).
    pub(super) fn connect_failed(&mut self, failure: super::setup_model::ConnectFailure) {
        self.phase = Phase::Form;
        self.error = failure.message;
        self.certificate = failure.certificate;
        self.report();
    }

    pub(super) fn complete(&mut self) {
        self.visible = false;
        self.required = false;
        self.bump();
    }

    pub(super) fn cancel(&mut self) {
        if !self.required {
            self.visible = false;
            self.bump();
        }
    }

    fn bump(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.form_generation = self.form_generation.wrapping_add(1);
    }

    /// A connect answered. The window re-reads the state without rebuilding the pane, so what
    /// was typed into it stands.
    fn report(&mut self) {
        self.generation = self.generation.wrapping_add(1);
    }
}

/// The pre-flights' answers and what they change on screen. A child module, so it reaches
/// `Phase` and the state's own fields.
#[path = "setup_signin.rs"]
mod signin;
