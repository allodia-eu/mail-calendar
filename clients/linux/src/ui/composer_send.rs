//! Send: the editor's document and the header fields, handed to the model as one submission.
//!
//! Split from [`super::composer`], which builds the chrome. A drafted reply's card may ask once
//! first (`super::composer_draft_reply`); it never stops the send.

use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

use adw::prelude::*;
use gtk::gio;
use webkit6::prelude::WebViewExt;

use super::{
    AppInput,
    composer_draft_reply::DraftReplyControl,
    composer_model::{ComposeContext, ComposerSubmission, PickedFile},
    recipients::RecipientField,
};
use crate::l10n;

#[allow(clippy::too_many_arguments)]
pub(super) fn connect_send(
    button: &gtk::Button,
    editor: &webkit6::WebView,
    request: ComposeContext,
    accounts: Vec<(String, String)>,
    from: gtk::DropDown,
    to: Rc<RecipientField>,
    cc: Rc<RecipientField>,
    bcc: Rc<RecipientField>,
    subject: gtk::Entry,
    files: Rc<RefCell<Vec<PickedFile>>>,
    error: gtk::Label,
    sender: relm4::Sender<AppInput>,
    drafted_reply: Option<Weak<DraftReplyControl>>,
) {
    let editor = editor.clone();
    let button_clone = button.clone();
    let submit: Rc<dyn Fn()> = Rc::new(move || {
        button_clone.set_sensitive(false);
        error.set_visible(false);
        let request = request.clone();
        let to_value = to.text();
        let cc_value = cc.text();
        let bcc_value = bcc.text();
        let subject_value = subject.text().to_string();
        let files_value = files.borrow().clone();
        let selected = from.selected();
        let from_value = usize::try_from(selected)
            .ok()
            .and_then(|index| accounts.get(index))
            .map(|(id, _)| id.clone());
        let input_sender = sender.clone();
        let error = error.clone();
        let button = button_clone.clone();
        editor.evaluate_javascript(
            "composerDocument()",
            None,
            None,
            None::<&gio::Cancellable>,
            move |result| {
                if let Ok(value) = result {
                    input_sender.emit(AppInput::SubmitComposer(Box::new(ComposerSubmission {
                        request,
                        to: to_value,
                        cc: cc_value,
                        bcc: bcc_value,
                        subject: subject_value,
                        document_json: value.to_str().to_string(),
                        files: files_value,
                        from: from_value,
                    })));
                } else {
                    // The label is shared with the dropped-picture failure, so re-state which
                    // failure this is rather than leaving the last message standing.
                    error.set_text(l10n::compose_prepare_error());
                    error.set_visible(true);
                    button.set_sensitive(true);
                }
            },
        );
    });
    button.connect_clicked(
        move |_| match drafted_reply.as_ref().and_then(Weak::upgrade) {
            Some(control) => control.before_send(Rc::clone(&submit)),
            None => submit(),
        },
    );
}
