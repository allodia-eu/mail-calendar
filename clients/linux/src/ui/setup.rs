//! Email-first account-setup window: the phases, and the state each pane renders from.

use adw::prelude::*;

use super::{
    AppInput, setup_google, setup_imap, setup_jmap, setup_manual, setup_microsoft,
    setup_model::{AccountKind, DetectedForm, JmapSignIn, ManualForm, SetupForm, edit_manually},
    setup_onboarding::{self, Onboarding},
    setup_widgets::{actions, body, entry, heading, page, progress},
};
use crate::l10n;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Email,
    Detecting,
    Form,
    GoogleSigningIn,
    MicrosoftSigningIn,
    JmapSigningIn,
    Connecting,
}

pub(super) struct SetupState {
    visible: bool,
    required: bool,
    generation: u64,
    phase: Phase,
    form: Option<SetupForm>,
    error: Option<String>,
    /// The address an account offered by one of the person's other devices is for. It only fills
    /// the field: which route the flow takes is still decided by detection, so an offer whose
    /// settings have since moved is corrected rather than believed.
    start_email: String,
    /// The first-run Allodia recommendation's state ([`super::setup_onboarding`]). Held here
    /// because a sign-in outlives several window rebuilds.
    onboarding: Onboarding,
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
        self.start_email = start_email;
        self.bump();
    }

    pub(super) fn detecting(&mut self) {
        self.phase = Phase::Detecting;
        self.error = None;
        self.bump();
    }

    pub(super) fn show_form(&mut self, form: SetupForm) {
        self.phase = Phase::Form;
        self.form = Some(form);
        self.error = None;
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

    pub(super) fn connecting(&mut self) {
        self.phase = Phase::Connecting;
        self.error = None;
        self.bump();
    }

    pub(super) fn google_signing_in(&mut self) {
        self.phase = Phase::GoogleSigningIn;
        self.error = None;
        self.bump();
    }

    pub(super) fn microsoft_signing_in(&mut self) {
        self.phase = Phase::MicrosoftSigningIn;
        self.error = None;
        self.bump();
    }

    pub(super) fn jmap_signing_in(&mut self) {
        self.phase = Phase::JmapSigningIn;
        self.error = None;
        self.bump();
    }

    /// The pre-flight's answer, applied to whichever pane asked for it; the detected card or
    /// the manual form. Returns whether it belonged to what is on screen.
    ///
    /// Only the **first** answer for an address counts, which is what lets a deadline race the
    /// probe: whichever arrives first decides, and the loser finds a state that is no longer
    /// `Checking` and is dropped.
    pub(super) fn jmap_oauth_available(
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

    pub(super) fn jmap_sign_in_failed(&mut self) {
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

    pub(super) fn retry_form(&mut self) {
        self.phase = Phase::Form;
        self.error = None;
        self.bump();
    }

    pub(super) fn failed(&mut self, error: String) {
        self.phase = Phase::Form;
        self.error = Some(error);
        self.bump();
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
    }
}

/// The pre-flight a freshly shown manual form owes, if any.
fn manual_probe(form: &SetupForm) -> Option<ManualForm> {
    match form {
        SetupForm::Manual(manual) if manual.probes_jmap_sign_in() => Some(manual.clone()),
        _ => None,
    }
}

#[derive(Debug, Default)]
pub(super) struct SetupWindow {
    window: Option<gtk::Window>,
    rendered_generation: u64,
    rendered_required: bool,
}

impl SetupWindow {
    pub(super) fn render(
        &mut self,
        state: &SetupState,
        parent: &adw::ApplicationWindow,
        sender: &relm4::Sender<AppInput>,
    ) {
        if !state.visible {
            if let Some(window) = self.window.take() {
                // Required setup rejects `close-request`; use the host-controlled destruction
                // path after success while keeping the user-close guard intact.
                window.destroy();
            }
            return;
        }
        // The close-request guard captures `required`, so only a change in that has to build a
        // new window. Every other step swaps the child of the one already on screen; a window
        // per phase would stack them, since `close()` is vetoed while setup is required.
        let reusable = self.rendered_required == state.required;
        let window = match self.window.take() {
            Some(window) if reusable => {
                if self.rendered_generation == state.generation {
                    self.window = Some(window);
                    return;
                }
                window
            }
            Some(previous) => {
                previous.destroy();
                modal(parent, state.required, sender)
            }
            None => modal(parent, state.required, sender),
        };
        // A default action belongs to one phase's child. Clear it before building the next phase,
        // whose builder may install a different default while the old child still exists.
        window.set_default_widget(None::<&gtk::Widget>);
        let content = match state.phase {
            Phase::Email => email_step(&window, state, sender),
            Phase::Detecting => progress(l10n::setup_detect_looking()),
            Phase::Form => form_step(&window, state, sender),
            Phase::GoogleSigningIn => setup_google::signing_in(sender),
            Phase::MicrosoftSigningIn => setup_microsoft::signing_in(sender),
            Phase::JmapSigningIn => setup_jmap::signing_in(sender),
            Phase::Connecting => progress(l10n::status_connecting()),
        };
        window.set_child(Some(&content));
        window.present();
        self.rendered_generation = state.generation;
        self.rendered_required = state.required;
        self.window = Some(window);
    }

    #[cfg(test)]
    pub(super) fn current_window(&self) -> Option<gtk::Window> {
        self.window.clone()
    }
}

fn modal(
    parent: &adw::ApplicationWindow,
    required: bool,
    sender: &relm4::Sender<AppInput>,
) -> gtk::Window {
    let (window, _) = crate::ui::modal::new(parent, l10n::setup_detect_title(), 520, Some(520));
    let input = sender.clone();
    window.connect_close_request(move |_| {
        if required {
            gtk::glib::Propagation::Stop
        } else {
            // Dismissing with the window controls has to end the flow, exactly like the Cancel
            // button. Without this the state stays `visible`, so a late `bump()`: a slow JMAP
            // OAuth pre-flight, a detection result, a connect failure; re-presents the modal
            // the user just closed.
            input.emit(AppInput::CancelAccountSetup);
            gtk::glib::Propagation::Proceed
        }
    });
    window
}

fn email_step(
    window: &gtk::Window,
    state: &SetupState,
    sender: &relm4::Sender<AppInput>,
) -> gtk::Box {
    let required = state.required;
    let content = page();
    // The recommendation, the way back for someone who already has an account, and the divider
    // that names what follows: above the address field, in that order (`docs/onboarding.md`).
    // Nothing at all in a build with no registration. On a later add the card is gone and the
    // offers are not: `required` is what tells the two apart.
    setup_onboarding::append(&content, &state.onboarding, sender, required);
    content.append(&body(l10n::setup_detect_description()));
    let email = entry(
        l10n::setup_detect_email_placeholder(),
        &state.start_email,
        false,
    );
    email.set_input_purpose(gtk::InputPurpose::Email);
    email.set_activates_default(true);
    content.append(&email);

    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    actions.set_halign(gtk::Align::End);
    if !required {
        let cancel = gtk::Button::with_label(l10n::action_cancel());
        let input = sender.clone();
        let dialog = window.clone();
        cancel.connect_clicked(move |_| {
            input.emit(AppInput::CancelAccountSetup);
            dialog.close();
        });
        actions.append(&cancel);
    }
    let manual = gtk::Button::with_label(l10n::setup_detect_manual());
    let manual_email = email.clone();
    let input = sender.clone();
    manual.connect_clicked(move |_| {
        input.emit(AppInput::ManualAccountSetup(
            manual_email.text().trim().to_owned(),
        ));
    });
    actions.append(&manual);
    let detect = gtk::Button::with_label(l10n::setup_detect_action());
    detect.add_css_class("suggested-action");
    window.set_default_widget(Some(&detect));
    let input = sender.clone();
    detect.connect_clicked(move |_| {
        let value = email.text().trim().to_owned();
        if !value.is_empty() {
            input.emit(AppInput::DetectAccount(value));
        }
    });
    actions.append(&detect);
    content.append(&actions);
    content
}

fn form_step(
    window: &gtk::Window,
    state: &SetupState,
    sender: &relm4::Sender<AppInput>,
) -> gtk::Box {
    let content = page();
    let error = state.error.as_deref();
    let required = state.required;
    let Some(form) = &state.form else {
        content.append(&heading(l10n::setup_detect_found_title()));
        content.append(&body(l10n::setup_detect_reason_nothing()));
        content.append(&actions(window, required, sender));
        return content;
    };
    match form {
        SetupForm::Detected(detected) => {
            content.append(&heading(l10n::setup_detect_found_title()));
            match detected {
                DetectedForm::Imap(form) => {
                    setup_imap::detected_fields(&content, window, form, error, required, sender);
                }
                DetectedForm::Jmap(form) => {
                    setup_jmap::detected_fields(&content, window, form, error, required, sender);
                }
                DetectedForm::Microsoft(form) => {
                    setup_microsoft::detected_fields(
                        &content, window, form, error, required, sender,
                    );
                }
                DetectedForm::Google(form) => {
                    setup_google::detected_fields(&content, window, form, error, required, sender);
                }
            }
        }
        SetupForm::Manual(form) => {
            content.append(&heading(l10n::setup_title()));
            setup_manual::fields(&content, window, form, error, required, sender);
        }
    }
    content
}

#[cfg(test)]
#[path = "setup_tests.rs"]
mod tests;
