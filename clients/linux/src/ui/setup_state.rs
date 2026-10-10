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
    /// The account is added; which of the uses its server offers it is used for is asked.
    Uses,
    /// The account is added; linking it to another is offered.
    Links,
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
    /// The uses step on screen, in [`Phase::Uses`]; `None` there while the choice is stored.
    pub(super) uses: Option<super::setup_signed_in::UsesStep>,
    /// The link step on screen, in [`Phase::Links`].
    pub(super) links: Option<super::setup_links::LinkStep>,
    /// The account whose link step an "Add another account" left, to come back to.
    pub(super) returning_to: Option<String>,
    /// Every account this flow added, whose names are asked once it ends.
    pub(super) added: Vec<String>,
    /// The servers found beside each JMAP account this flow added, until its link step uses them.
    pub(super) beside: Vec<super::setup_links::Beside>,
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
            uses: None,
            links: None,
            returning_to: None,
            added: Vec::new(),
            beside: Vec::new(),
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

    /// A flow of its own, which comes back to no link step and has added nothing yet.
    fn forget_flow(&mut self) {
        self.uses = None;
        self.links = None;
        self.returning_to = None;
        self.added.clear();
        self.beside.clear();
    }

    /// Keeps the servers detection found beside the JMAP form `account` was just added from.
    pub(super) fn remember_beside(&mut self, account: &str) {
        let Some(SetupForm::Detected(super::setup_model::DetectedForm::Jmap(form))) = &self.form
        else {
            return;
        };
        if form.caldav_url.is_some() || form.carddav_url.is_some() {
            self.beside.push(super::setup_links::Beside {
                account: account.to_owned(),
                email: form.email.clone(),
                caldav_url: form.caldav_url.clone(),
                carddav_url: form.carddav_url.clone(),
            });
        }
    }

    /// The servers found beside `account`'s own, while unused.
    pub(super) fn beside_for(&self, account: &str) -> Option<&super::setup_links::Beside> {
        self.beside.iter().find(|beside| beside.account == account)
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
        self.forget_flow();
        self.bump();
    }

    /// Shows the uses step. A link step an "Add another account" left is kept, so what was
    /// picked on it comes back after.
    pub(super) fn show_uses(&mut self, step: super::setup_signed_in::UsesStep) {
        self.phase = Phase::Uses;
        self.uses = Some(step);
        self.error = None;
        self.certificate = None;
        self.bump();
    }

    /// The uses step answered: it waits while the choice is stored. `None` when it is not on
    /// screen.
    pub(super) fn take_uses(&mut self) -> Option<super::setup_signed_in::UsesStep> {
        if !self.visible || self.phase != Phase::Uses {
            return None;
        }
        let step = self.uses.take()?;
        self.bump();
        Some(step)
    }

    /// Whether the uses step is on screen, answered or not.
    pub(super) fn choosing_uses(&self) -> bool {
        self.visible && self.phase == Phase::Uses
    }

    /// Shows the link step.
    pub(super) fn show_links(&mut self, step: super::setup_links::LinkStep) {
        self.phase = Phase::Links;
        self.links = Some(step);
        self.error = None;
        self.certificate = None;
        self.bump();
    }

    /// The same step read again, for a suggestion that arrived after it was drawn: redrawn only
    /// when the person has not changed it.
    pub(super) fn refresh_links(&mut self, fresh: super::setup_links::LinkStep) {
        if self.phase != Phase::Links {
            return;
        }
        let Some(step) = self.links.take() else {
            return;
        };
        // Every Settings signal reads the step again; redrawing an unchanged one would close a
        // dropdown the person has open.
        let redraw =
            !step.touched && (step.pickers != fresh.pickers || step.picked != fresh.picked);
        self.links = Some(step.refreshed(fresh));
        if redraw {
            self.bump();
        }
    }

    /// The account the link step on screen is for.
    pub(super) fn linking(&self) -> Option<&str> {
        (self.visible && self.phase == Phase::Links)
            .then_some(self.links.as_ref())
            .flatten()
            .map(|step| step.account.as_str())
    }

    /// Records a pick. The dropdown already shows it, so nothing is redrawn.
    pub(super) fn pick_link(&mut self, picker: usize, option: Option<usize>) {
        if let Some(step) = self.links.as_mut() {
            step.pick(picker, option);
        }
    }

    /// "Add another account" from the link step: setup again from the address, coming back to
    /// this step when it adds an account or is cancelled.
    pub(super) fn add_linked(&mut self) {
        let Some(step) = self.links.take() else {
            return;
        };
        let (added, beside) = (
            std::mem::take(&mut self.added),
            std::mem::take(&mut self.beside),
        );
        self.open_on(false, String::new());
        self.returning_to = Some(step.account.clone());
        self.added = added;
        self.beside = beside;
        // Kept, so what was picked comes back with the step.
        self.links = Some(step);
    }

    /// The link step's offer of the servers found beside its account: setup again, on those
    /// servers, coming back to the step as "Add another account" does. The offer is used up.
    pub(super) fn add_beside(&mut self) -> Option<super::setup_links::Beside> {
        let account = self.links.as_ref()?.account.clone();
        let index = self
            .beside
            .iter()
            .position(|beside| beside.account == account)?;
        let beside = self.beside.remove(index);
        self.add_linked();
        // Back from the card it opens goes to this address, as from any other.
        self.start_email.clone_from(&beside.email);
        Some(beside)
    }

    /// Shows the link step again, read afresh, with what was picked on it before carried across,
    /// and `added`, an account just added from it, picked where it fits.
    pub(super) fn show_links_again(
        &mut self,
        fresh: super::setup_links::LinkStep,
        added: Option<&str>,
    ) {
        let mut step = match self.links.take() {
            Some(earlier) if earlier.account == fresh.account => earlier.refreshed(fresh),
            _ => fresh,
        };
        if let Some(added) = added {
            step.adopt(added);
        }
        self.show_links(step);
    }

    /// Back from the second step to the address it was reached with, which stays in the field
    /// (`docs/account-autodetect.md` rule 12).
    pub(super) fn back_to_address(&mut self, required: bool) {
        let email = self.form.as_ref().map(SetupForm::email).unwrap_or_default();
        let (returning_to, added, links, beside) = (
            self.returning_to.take(),
            std::mem::take(&mut self.added),
            self.links.take(),
            std::mem::take(&mut self.beside),
        );
        self.open_on(required, email);
        self.returning_to = returning_to;
        self.added = added;
        self.links = links;
        self.beside = beside;
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
