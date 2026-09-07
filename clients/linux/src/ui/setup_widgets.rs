//! Widget builders shared by every account-setup pane, and the one step that comes *after* a
//! connect rather than before it: the "your name" question (`docs/sending.md`).
//!
//! That step is here because it is setup, even though it is raised over the running app: the
//! screen that adds an account is the address field and nothing else (`docs/onboarding.md`), so
//! the name is asked once the account exists and its provider can be asked what it already calls
//! this person.

use adw::prelude::*;
use mailcal_bindings::MailcalApp;

use super::{AppInput, modal};
use crate::l10n;

/// The padded vertical box every setup step is built into.
pub(super) fn page() -> gtk::Box {
    let content = gtk::Box::new(gtk::Orientation::Vertical, 14);
    content.set_margin_start(24);
    content.set_margin_end(24);
    content.set_margin_top(24);
    content.set_margin_bottom(24);
    content
}

pub(super) fn heading(text: &str) -> gtk::Label {
    let label = body(text);
    label.add_css_class("title-1");
    label
}

/// A wrapping, left-aligned paragraph. A `gtk::Label` does not parse markup unless asked, so
/// server names and provider text render as themselves.
pub(super) fn body(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.set_xalign(0.0);
    label.set_wrap(true);
    label
}

pub(super) fn caption(text: &str) -> gtk::Label {
    let label = body(text);
    label.add_css_class("caption");
    label.add_css_class("dim-label");
    label
}

pub(super) fn section(text: &str) -> gtk::Label {
    let label = body(text);
    label.add_css_class("heading");
    label
}

pub(super) fn entry(placeholder: &str, value: &str, secret: bool) -> gtk::Entry {
    let field = gtk::Entry::new();
    field.set_placeholder_text(Some(placeholder));
    field.set_text(value);
    field.set_visibility(!secret);
    field
}

pub(super) fn show_error(content: &gtk::Box, error: Option<&str>) {
    if let Some(error) = error {
        let message = body(error);
        message.add_css_class("error");
        content.append(&message);
    }
}

/// One discovered server, as a labelled row rather than an editable field: detection already
/// found it, and the user's job here is to recognise it, not to retype it.
pub(super) fn detected_row(protocol: &str, detail: &str) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let name = body(protocol);
    name.add_css_class("caption-heading");
    name.add_css_class("dim-label");
    name.set_width_chars(6);
    row.append(&name);
    row.append(&body(detail));
    row
}

/// The untrusted-settings approval: hidden and pre-approved for a TLS-sourced recommendation,
/// an explicit checkbox otherwise. Returned so the submit path can read it
/// ([`trust_approved`]): every client gates credentials on this the same way
/// (`docs/account-autodetect.md` rule 3).
pub(super) fn trust_gate(content: &gtk::Box, trusted: bool) -> gtk::CheckButton {
    let confirm = gtk::CheckButton::with_label(l10n::setup_detect_trust_confirm());
    if trusted {
        confirm.set_active(true);
        confirm.set_visible(false);
    } else {
        let warning = body(l10n::setup_detect_untrusted_warning());
        warning.add_css_class("warning");
        content.append(&warning);
        content.append(&confirm);
    }
    confirm
}

pub(super) const fn trust_approved(detected_trusted: bool, user_approved: bool) -> bool {
    detected_trusted || user_approved
}

/// Holds an untrusted recommendation's Connect closed until the box is ticked. The submit path
/// re-checks [`trust_approved`]: this is the affordance, not the gate: because a button that
/// silently does nothing reads as a broken app rather than as a question waiting for an answer.
pub(super) fn gate_on_trust(trust: &gtk::CheckButton, button: &gtk::Button, trusted: bool) {
    if trusted {
        return;
    }
    button.set_sensitive(false);
    let gated = button.clone();
    trust.connect_toggled(move |choice| gated.set_sensitive(choice.is_active()));
}

/// The trailing button row every pane ends with. Cancel (when the flow is dismissable) and Back
/// are the same everywhere; the caller appends whichever primary action its pane offers.
pub(super) fn actions(
    window: &gtk::Window,
    required: bool,
    sender: &relm4::Sender<AppInput>,
) -> gtk::Box {
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
    let back = gtk::Button::with_label(l10n::a11y_back());
    let input = sender.clone();
    back.connect_clicked(move |_| input.emit(AppInput::RestartAccountSetup));
    actions.append(&back);
    actions
}

/// "Set up manually" from a detected card: the same escape the email step offers, carrying the
/// detection into the form so the user edits it rather than retyping it.
pub(super) fn edit_manually_button(sender: &relm4::Sender<AppInput>) -> gtk::Button {
    let manual = gtk::Button::with_label(l10n::setup_detect_manual());
    let input = sender.clone();
    manual.connect_clicked(move |_| input.emit(AppInput::EditDetectedManually));
    manual
}

pub(super) fn primary(label: &str, window: &gtk::Window) -> gtk::Button {
    let button = gtk::Button::with_label(label);
    button.add_css_class("suggested-action");
    window.set_default_widget(Some(&button));
    button
}

/// A spinner step, with a Cancel that abandons whatever flow is running. The input is a plain
/// function pointer because the button's handler outlives the call.
pub(super) fn waiting(
    message: &str,
    cancel: fn() -> AppInput,
    sender: &relm4::Sender<AppInput>,
) -> gtk::Box {
    let content = page();
    let spinner = gtk::Spinner::new();
    spinner.set_spinning(true);
    spinner.set_size_request(48, 48);
    content.append(&spinner);
    content.append(&body(message));
    let button = gtk::Button::with_label(l10n::action_cancel());
    button.set_halign(gtk::Align::End);
    let input = sender.clone();
    button.connect_clicked(move |_| input.emit(cancel()));
    content.append(&button);
    content
}

/// A progress step with nothing to cancel; detection and the connect itself.
pub(super) fn progress(message: &str) -> gtk::Box {
    let content = page();
    let spinner = gtk::Spinner::new();
    spinner.set_spinning(true);
    spinner.set_size_request(48, 48);
    content.append(&spinner);
    content.append(&body(message));
    content
}

/// Which account the step is asking about, and the name the provider suggested for it.
///
/// The account is fixed for the life of the step: asking "your name" without knowing whose
/// would write the answer onto whichever account happened to be first, so a route that cannot
/// name the account it added raises no step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SenderNameAsk {
    /// The account the question is about.
    pub(super) account: String,
    /// What the provider already calls this person, empty when it holds nothing.
    pub(super) suggestion: String,
}

/// Reads the provider's suggestion for `account`.
///
/// Blocking: it is a provider round trip, so every caller runs it on a worker thread, never on
/// the main loop. An account with no server-side name simply answers empty, which is the
/// ordinary IMAP case and means *ask*.
pub(super) fn sender_name_suggestion(app: &MailcalApp, account: &str) -> String {
    app.suggested_sender_name(account.to_owned())
}

/// The open step, if one is.
#[derive(Default)]
pub(super) struct SenderNamePrompt {
    window: Option<gtk::Window>,
    /// What the open window is asking about, so an unchanged model re-render does not rebuild
    /// it under the user's cursor.
    showing: Option<SenderNameAsk>,
}

impl SenderNamePrompt {
    /// Presents the step for `ask`, or closes the open one when there is nothing to ask.
    pub(super) fn render(
        &mut self,
        ask: Option<&SenderNameAsk>,
        parent: &impl IsA<gtk::Window>,
        sender: &relm4::Sender<AppInput>,
    ) {
        if self.showing.as_ref() == ask {
            return;
        }
        if let Some(window) = self.window.take() {
            window.destroy();
        }
        self.showing = ask.cloned();
        let Some(ask) = ask else {
            return;
        };

        let (window, _) = modal::new(parent, l10n::setup_sender_name_title(), 460, None);
        let content = gtk::Box::new(gtk::Orientation::Vertical, 16);
        content.set_margin_start(24);
        content.set_margin_end(24);
        content.set_margin_top(24);
        content.set_margin_bottom(24);

        let message = gtk::Label::new(Some(l10n::setup_sender_name_description()));
        message.set_wrap(true);
        message.set_xalign(0.0);
        content.append(&message);

        let entry = gtk::Entry::new();
        entry.set_placeholder_text(Some(l10n::setup_sender_name_field()));
        entry.set_text(&ask.suggestion);
        entry.set_activates_default(true);
        content.append(&entry);

        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        actions.set_halign(gtk::Align::End);
        let skip = gtk::Button::with_label(l10n::setup_sender_name_skip());
        let input = sender.clone();
        skip.connect_clicked(move |_| input.emit(AppInput::DismissSenderNamePrompt));
        actions.append(&skip);

        let save = gtk::Button::with_label(l10n::setup_sender_name_continue());
        save.add_css_class("suggested-action");
        let input = sender.clone();
        let account = ask.account.clone();
        let typed = entry.clone();
        save.connect_clicked(move |_| {
            input.emit(AppInput::SetAccountSenderName {
                account: account.clone(),
                name: typed.text().to_string(),
            });
        });
        actions.append(&save);
        content.append(&actions);
        // Return commits. `set_activates_default` on the entry does nothing without this:
        // the window has to name which button the default is.
        window.set_default_widget(Some(&save));

        // Closing the window is skipping: the account keeps sending as a bare address, which is
        // a state the app has to be correct in, so it needs no confirmation of its own.
        let input = sender.clone();
        window.connect_close_request(move |_| {
            input.emit(AppInput::DismissSenderNamePrompt);
            gtk::glib::Propagation::Proceed
        });
        window.set_child(Some(&content));
        window.present();
        entry.grab_focus();
        self.window = Some(window);
    }
}

#[cfg(test)]
mod tests {
    use super::trust_approved;

    #[test]
    fn untrusted_detection_requires_an_explicit_choice() {
        assert!(trust_approved(true, false));
        assert!(!trust_approved(false, false));
        assert!(trust_approved(false, true));
    }
}

#[cfg(test)]
#[path = "setup_sender_name_tests.rs"]
pub(crate) mod sender_name_tests;
