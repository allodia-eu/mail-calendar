//! The editor's request channel (`docs/composer-security.md`, Gate 2): one script-message handler
//! on the composer's and the signature editor's web views. A message is parsed by the core
//! (`parse_composer_host_request`), which knows the whole vocabulary, so anything acted on here is
//! a request the product has a type for; an answer goes back through the script the core builds.

use adw::prelude::*;
use gtk::{gio, glib};
use mailcal_bindings::{
    ComposerHostRequest, ComposerLinkAnswer, composer_host_channel, composer_host_requests_script,
    composer_link_address, composer_link_answer_script, parse_composer_host_request,
};
use webkit6::{WebView, prelude::*};

use crate::l10n;

const RESPONSE_CANCEL: &str = "cancel";
const RESPONSE_REMOVE: &str = "remove";
const RESPONSE_APPLY: &str = "apply";

/// Registers the channel on `view`, before its page loads. The editor asks for nothing until it
/// runs `announce_script`.
pub(crate) fn install(view: &WebView) {
    let Some(manager) = view.user_content_manager() else {
        return;
    };
    let channel = composer_host_channel();
    if !manager.register_script_message_handler(&channel, None) {
        return;
    }
    // Weak: the view owns the manager this handler is attached to.
    let view = view.downgrade();
    manager.connect_script_message_received(Some(channel.as_str()), move |_, value| {
        let Some(view) = view.upgrade() else {
            return;
        };
        let Some(request) = parse_composer_host_request(value.to_str().to_string()) else {
            return;
        };
        // Exhaustive, so a request added to the core's vocabulary fails this build until it is
        // answered here.
        match request {
            ComposerHostRequest::Link {
                id,
                text,
                address,
                removable,
            } => {
                ask_for_link(&view, id, &text, &address, removable);
            }
        }
    });
}

/// The script that tells the editor which requests this host answers, for the seed script.
pub(crate) fn announce_script() -> String {
    composer_host_requests_script()
}

/// The link dialog: the words a link is shown as and where it points. The address is completed and
/// checked by the core as it is typed, the rule the answer is held to, so Apply is enabled exactly
/// when the link can be made.
fn ask_for_link(view: &WebView, id: u64, text: &str, address: &str, removable: bool) {
    let title = if address.is_empty() {
        l10n::editor_link_dialog_insert()
    } else {
        l10n::editor_link_dialog_edit()
    };
    let dialog = adw::AlertDialog::new(Some(title), None);
    let text_row = entry_row(l10n::editor_link_text(), text);
    let address_row = entry_row(l10n::editor_link_address(), address);
    address_row.set_input_purpose(gtk::InputPurpose::Url);
    address_row.set_activates_default(true);
    let fields = gtk::ListBox::new();
    fields.add_css_class("boxed-list");
    fields.set_selection_mode(gtk::SelectionMode::None);
    fields.append(&text_row);
    fields.append(&address_row);
    let hint = gtk::Label::new(Some(l10n::editor_link_invalid()));
    hint.add_css_class("error");
    hint.set_xalign(0.0);
    hint.set_wrap(true);
    let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
    content.append(&fields);
    content.append(&hint);
    dialog.set_extra_child(Some(&content));

    dialog.add_response(RESPONSE_CANCEL, l10n::action_cancel());
    if removable {
        dialog.add_response(RESPONSE_REMOVE, l10n::editor_link_remove());
        dialog.set_response_appearance(RESPONSE_REMOVE, adw::ResponseAppearance::Destructive);
    }
    dialog.add_response(RESPONSE_APPLY, l10n::editor_link_apply());
    dialog.set_response_appearance(RESPONSE_APPLY, adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some(RESPONSE_APPLY));
    dialog.set_close_response(RESPONSE_CANCEL);

    let refresh = {
        let dialog = dialog.clone();
        move |row: &adw::EntryRow| {
            let typed = row.text();
            let valid = composer_link_address(typed.to_string()).is_some();
            dialog.set_response_enabled(RESPONSE_APPLY, valid);
            // Said only once something has been typed: an empty field is unfinished, not wrong.
            let wrong = !typed.trim().is_empty() && !valid;
            hint.set_visible(wrong);
            if wrong {
                row.add_css_class("error");
            } else {
                row.remove_css_class("error");
            }
        }
    };
    refresh(&address_row);
    address_row.connect_changed(refresh);

    let view = view.clone();
    glib::MainContext::default().spawn_local(async move {
        let response = dialog.choose_future(Some(&view)).await;
        let answer = answer_for(&response, &text_row.text(), &address_row.text());
        view.grab_focus();
        view.evaluate_javascript(
            &composer_link_answer_script(id, answer),
            None,
            None,
            None::<&gio::Cancellable>,
            |_| {},
        );
    });
}

/// One labelled field. Markup is off in the title: a translation holding a bare `&` would
/// otherwise render as an unterminated entity, which is a blank label.
fn entry_row(title: &str, value: &str) -> adw::EntryRow {
    let row = adw::EntryRow::new();
    row.set_title(title);
    row.set_use_markup(false);
    row.set_text(value);
    row
}

/// What the dialog's response means for the editor.
fn answer_for(response: &str, text: &str, address: &str) -> ComposerLinkAnswer {
    match response {
        RESPONSE_APPLY => ComposerLinkAnswer::Apply {
            text: text.to_owned(),
            address: address.to_owned(),
        },
        RESPONSE_REMOVE => ComposerLinkAnswer::Remove,
        _ => ComposerLinkAnswer::Cancel,
    }
}

#[cfg(test)]
mod tests {
    use mailcal_bindings::ComposerLinkAnswer;

    use super::{RESPONSE_APPLY, RESPONSE_CANCEL, RESPONSE_REMOVE, answer_for};

    #[test]
    fn each_way_out_of_the_dialog_answers_the_editor() {
        assert_eq!(
            answer_for(RESPONSE_APPLY, "docs", "example.com"),
            ComposerLinkAnswer::Apply {
                text: "docs".to_owned(),
                address: "example.com".to_owned()
            }
        );
        assert_eq!(
            answer_for(RESPONSE_REMOVE, "", ""),
            ComposerLinkAnswer::Remove
        );
        assert_eq!(
            answer_for(RESPONSE_CANCEL, "docs", "example.com"),
            ComposerLinkAnswer::Cancel
        );
        // Escape and a click outside close with the close response, never as an apply.
        assert_eq!(
            answer_for("", "docs", "example.com"),
            ComposerLinkAnswer::Cancel
        );
    }
}
