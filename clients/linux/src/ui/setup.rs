//! Email-first account-setup window: the panes, and which one each phase draws. The state
//! they render from is [`super::setup_state`].

use adw::prelude::*;

use super::{
    AppInput, setup_google, setup_imap, setup_jmap, setup_manual, setup_microsoft,
    setup_model::{DetectedForm, SetupForm},
    setup_onboarding,
    setup_pane::ConnectPane,
    setup_state::{Phase, SetupState},
    setup_widgets::{actions, body, entry, heading, page, progress},
};
use crate::l10n;

#[derive(Debug, Default)]
pub(super) struct SetupWindow {
    window: Option<gtk::Window>,
    rendered_generation: u64,
    rendered_required: bool,
    /// The form generation the pane on screen was built for, and the pane itself while one is
    /// mounted. Together they are what lets a connect and its answer be drawn *into* the form
    /// the person is typing in rather than replacing it (`super::setup_pane`).
    rendered_form_generation: u64,
    pane: Option<ConnectPane>,
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
                // A connect, or its answer, against the same form: the pane stays and takes the
                // result. Rebuilding it here is what used to empty the fields under the
                // certificate panel, the secret box included.
                if let Some(pane) = self
                    .pane
                    .as_ref()
                    .filter(|_| self.rendered_form_generation == state.form_generation)
                {
                    pane.set_connecting(state.phase == Phase::Connecting);
                    if state.phase == Phase::Form {
                        pane.show_result(state.error.as_deref(), state.certificate.as_ref());
                    }
                    self.rendered_generation = state.generation;
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
        self.pane = None;
        let content = match state.phase {
            Phase::Email => email_step(&window, state, sender),
            Phase::Detecting => progress(l10n::setup_detect_looking()),
            // A connect whose form is gone (the window was rebuilt under it) has nothing to
            // draw into, so it falls back to the phase spinner.
            Phase::Form | Phase::Connecting => {
                let (content, pane) = form_step(&window, state, sender);
                if let Some(pane) = &pane {
                    pane.set_connecting(state.phase == Phase::Connecting);
                }
                self.pane = pane;
                content
            }
            Phase::GoogleSigningIn => setup_google::signing_in(sender),
            Phase::MicrosoftSigningIn => setup_microsoft::signing_in(sender),
            Phase::JmapSigningIn => setup_jmap::signing_in(sender),
        };
        window.set_child(Some(&content));
        window.present();
        self.rendered_generation = state.generation;
        self.rendered_form_generation = state.form_generation;
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
) -> (gtk::Box, Option<ConnectPane>) {
    let content = page();
    let error = state.error.as_deref();
    // Only an IMAP pane can act on a refused certificate: a JMAP account's stored config
    // carries no exception (`docs/certificate-exceptions.md` → Known gaps), so every other
    // route reports the refusal and offers nothing.
    let certificate = state.certificate.as_ref();
    let required = state.required;
    let Some(form) = &state.form else {
        content.append(&heading(l10n::setup_detect_found_title()));
        content.append(&body(l10n::setup_detect_reason_nothing()));
        content.append(&actions(window, required, sender));
        return (content, None);
    };
    let pane = match form {
        SetupForm::Detected(detected) => {
            content.append(&heading(l10n::setup_detect_found_title()));
            match detected {
                DetectedForm::Imap(form) => Some(setup_imap::detected_fields(
                    &content,
                    window,
                    form,
                    error,
                    certificate,
                    required,
                    sender,
                )),
                DetectedForm::Jmap(form) => {
                    setup_jmap::detected_fields(&content, window, form, error, required, sender)
                }
                DetectedForm::Microsoft(form) => {
                    setup_microsoft::detected_fields(
                        &content, window, form, error, required, sender,
                    );
                    None
                }
                DetectedForm::Google(form) => {
                    setup_google::detected_fields(&content, window, form, error, required, sender);
                    None
                }
            }
        }
        SetupForm::Manual(form) => {
            content.append(&heading(l10n::setup_title()));
            setup_manual::fields(&content, window, form, error, certificate, required, sender)
        }
    };
    (content, pane)
}

#[cfg(test)]
#[path = "setup_tests.rs"]
mod tests;
