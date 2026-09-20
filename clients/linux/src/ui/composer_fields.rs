//! What the composer's Send and Save read the message out of, and the one route both take.
//!
//! Split from [`super::composer`], which is at the 500-line limit. The two actions differ in the
//! message they emit and in what they do when the editor cannot be read: a send says so and gives
//! its button back, a save is silent, because saving is never something the user waits for
//! (`docs/drafts.md`).

use std::{cell::RefCell, rc::Rc};

use adw::prelude::*;
use gtk::gio;
use webkit6::prelude::WebViewExt;

use super::{
    AppInput,
    composer_model::{ComposeContext, ComposerSubmission, PickedFile},
    recipients::RecipientField,
};
use crate::l10n;

/// The composer's live widgets, as Send, Save and the autosave timer read them.
#[derive(Clone)]
pub(super) struct ComposerFields {
    pub(super) request: ComposeContext,
    /// The From picker's items, in its own order, so the selected index names an account.
    pub(super) accounts: Vec<(String, String)>,
    pub(super) from: gtk::DropDown,
    pub(super) to: Rc<RecipientField>,
    pub(super) cc: Rc<RecipientField>,
    pub(super) bcc: Rc<RecipientField>,
    pub(super) subject: gtk::Entry,
    pub(super) files: Rc<RefCell<Vec<PickedFile>>>,
}

/// The header as one value, so one comparison covers every field rather than one per field.
pub(super) type HeaderFingerprint = (String, String, String, String, usize);

impl ComposerFields {
    /// What the composer's header holds, for the autosave timer to compare against the last tick.
    ///
    /// Sampled rather than signalled: [`RecipientField`] keeps **one** change callback, and the
    /// Send button already owns it, so a second registration would replace it and leave Send
    /// stuck on whatever it had last decided.
    pub(super) fn fingerprint(&self) -> HeaderFingerprint {
        (
            self.to.text(),
            self.cc.text(),
            self.bcc.text(),
            self.subject.text().to_string(),
            self.files.borrow().len(),
        )
    }

    /// The account the From picker is showing, which is the one the send goes out as.
    fn sender_account(&self) -> Option<String> {
        usize::try_from(self.from.selected())
            .ok()
            .and_then(|index| self.accounts.get(index))
            .map(|(id, _)| id.clone())
    }
}

/// Reads the editor document and hands the whole submission to `deliver`.
///
/// `on_failure` runs instead when the document cannot be read.
pub(super) fn read_document(
    editor: &webkit6::WebView,
    fields: &ComposerFields,
    sender: &relm4::Sender<AppInput>,
    deliver: fn(Box<ComposerSubmission>) -> AppInput,
    on_failure: impl Fn() + 'static,
) {
    let request = fields.request.clone();
    let to = fields.to.text();
    let cc = fields.cc.text();
    let bcc = fields.bcc.text();
    let subject = fields.subject.text().to_string();
    let files = fields.files.borrow().clone();
    let from = fields.sender_account();
    let sender = sender.clone();
    editor.evaluate_javascript(
        "composerDocument()",
        None,
        None,
        None::<&gio::Cancellable>,
        move |result| match result {
            Ok(value) => sender.emit(deliver(Box::new(ComposerSubmission {
                request,
                to,
                cc,
                bcc,
                subject,
                document_json: value.to_str().to_string(),
                files,
                from,
            }))),
            Err(_) => on_failure(),
        },
    );
}

/// Send: the document is read, the message goes, and a failure says which failure it was.
pub(super) fn connect_send(
    button: &gtk::Button,
    editor: &webkit6::WebView,
    fields: &ComposerFields,
    error: gtk::Label,
    sender: relm4::Sender<AppInput>,
) {
    let editor = editor.clone();
    let fields = fields.clone();
    let pressed = button.clone();
    button.connect_clicked(move |_| {
        pressed.set_sensitive(false);
        error.set_visible(false);
        let error = error.clone();
        let button = pressed.clone();
        read_document(
            &editor,
            &fields,
            &sender,
            AppInput::SubmitComposer,
            move || {
                // The label is shared with the dropped-picture failure, so re-state which
                // failure this is rather than leaving the last message standing.
                error.set_text(l10n::compose_prepare_error());
                error.set_visible(true);
                button.set_sensitive(true);
            },
        );
    });
}

/// Save as draft: the same read, and silence when it fails.
///
/// The button is never disabled. A draft is unfinished by definition, so there is no state a
/// composer can be in that this refuses, and pressing it on an unchanged message reaches no
/// server.
pub(super) fn connect_save_draft(
    button: &gtk::Button,
    editor: &webkit6::WebView,
    fields: &ComposerFields,
    sender: relm4::Sender<AppInput>,
) {
    let editor = editor.clone();
    let fields = fields.clone();
    button.connect_clicked(move |_| {
        read_document(
            &editor,
            &fields,
            &sender,
            AppInput::SaveComposerDraft,
            || {},
        );
    });
}
