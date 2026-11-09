//! Popping the composer out: the pane's composer moved into a window of its own, holding exactly
//! what it held (`docs/reading-window.md`, "Popping a composer out").
//!
//! The composition does not move; only its view does. The window opens on the composition the
//! pane's composer had, so its saves go on superseding the same stored draft, and the pane lets go
//! of it without leaving: leaving would save and close the composition the window has just opened
//! on.

use std::rc::Rc;

use adw::prelude::*;
use mailcal_bindings::{ComposeOrigin, ComposerFileAttachment, ComposerState, MovedSignature};

use super::{
    AppInput, AppModel,
    composer_draft::DraftGuard,
    composer_fields::read_document,
    composer_model::{ComposeContext, ComposerSubmission},
    composer_open::ComposerTarget,
    composer_signature::SignatureControl,
    icons,
    reader::ComposerHost,
};
use crate::l10n;

/// What the pane's composer held as it was popped out.
#[derive(Debug)]
pub(crate) struct PoppedOut {
    pub(crate) submission: ComposerSubmission,
    /// What the composer was doing with the signature.
    pub(crate) signature: MovedSignature,
    /// Whether anything had been written in it.
    pub(crate) written: bool,
}

/// The pop-out button, at the right end of the composer's header bar, where Outlook puts it: it
/// is about the composer as a whole, not an action on the message. Only the pane's composer has
/// one: a composer already in a window has nowhere further to go.
pub(super) fn pop_out_button(request: &ComposeContext) -> Option<gtk::Button> {
    if request.host != ComposerHost::Pane {
        return None;
    }
    let button = gtk::Button::from_icon_name(icons::POP_OUT);
    button.add_css_class("flat");
    button.set_tooltip_text(Some(l10n::action_open_in_window()));
    button.update_property(&[gtk::accessible::Property::Label(
        l10n::action_open_in_window(),
    )]);
    Some(button)
}

/// Moves the composer into a window when `button` is pressed.
pub(super) fn connect_pop_out(
    button: &gtk::Button,
    request: &ComposeContext,
    guard: &Rc<DraftGuard>,
    signature: Option<&Rc<SignatureControl>>,
    sender: &relm4::Sender<AppInput>,
) {
    let manages = request.manages_signature();
    let guard = Rc::downgrade(guard);
    let signature = signature.map(Rc::downgrade);
    let sender = sender.clone();
    button.connect_clicked(move |_| {
        let Some(guard) = guard.upgrade() else {
            return;
        };
        let control = signature.as_ref().and_then(std::rc::Weak::upgrade);
        let moved = match (manages, control) {
            (false, _) => MovedSignature::Untouched,
            (true, Some(control)) => control.moved(),
            // An empty library offers no control, and nothing to follow but the account.
            (true, None) => MovedSignature::FollowsAccount,
        };
        pop_out(&guard, moved, &sender);
    });
}

/// Reads everything the composer holds and reports it with what it was doing with the signature.
/// A document that cannot be read moves nothing, and the composer stays where it is.
fn pop_out(guard: &DraftGuard, signature: MovedSignature, sender: &relm4::Sender<AppInput>) {
    let editor = guard.editor.clone();
    let fields = guard.fields.clone();
    let sender = sender.clone();
    guard.edited(move |written| {
        let unread = sender.clone();
        read_document(
            &editor,
            &fields,
            &sender,
            move |submission| {
                AppInput::PopOutComposer(Box::new(PoppedOut {
                    submission: *submission,
                    signature,
                    written,
                }))
            },
            move || unread.emit(AppInput::ComposerPopOutFailed),
        );
    });
}

impl AppModel {
    /// Asks the core to open the pane's composer again in a window, on the composition it has.
    /// The pane lets go of it once the answer is in ([`AppModel::composer_opened`]).
    ///
    /// An answer for a composer no longer in the pane is dropped: it was left or sent meanwhile,
    /// and its composition is not the pane's to move.
    pub(super) fn pop_out_composer(&mut self, popped: PoppedOut, sender: &relm4::Sender<AppInput>) {
        let PoppedOut {
            submission,
            signature,
            written,
        } = popped;
        let composition = submission.request.composition.clone();
        if self
            .composer
            .as_ref()
            .is_none_or(|open| open.composition != composition)
        {
            return;
        }
        let state = ComposerState {
            from: submission.from,
            to: submission.to,
            cc: submission.cc,
            bcc: submission.bcc,
            subject: submission.subject,
            document_json: submission.document_json,
            files: submission
                .files
                .into_iter()
                .map(|file| ComposerFileAttachment {
                    path: file.path,
                    file_name: file.file_name,
                    media_type: file.media_type,
                })
                .collect(),
            signature,
            written,
        };
        self.open_composer_into(
            Some(composition),
            ComposeOrigin::Moved { state },
            None,
            ComposerTarget::Window,
            Vec::new(),
            sender,
        );
    }

    /// The pane's composer moved into a window: the pane is the reading pane's again. Its
    /// composition is the window's now, so it is not closed, which is the difference from every
    /// other way a composer leaves the pane.
    pub(super) fn release_popped_out_composer(&mut self, composition: &str) {
        if self
            .composer
            .as_ref()
            .is_some_and(|open| open.composition == composition)
        {
            self.composer = None;
            self.composer_error = None;
        }
    }
}
