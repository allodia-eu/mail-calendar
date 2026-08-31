//! Widget builders shared by every account-setup pane.

use adw::prelude::*;

use super::AppInput;
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
