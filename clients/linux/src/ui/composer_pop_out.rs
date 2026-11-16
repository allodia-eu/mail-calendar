//! Popping the composer out: the pane's composer moved into a window of its own, holding exactly
//! what it held (`docs/reading-window.md`, "Popping a composer out").
//!
//! The composition does not move; only its view does. The window opens on the composition the
//! pane's composer had, so its saves go on superseding the same stored draft and its files are the
//! ones the core already holds for it, and the pane lets go of it without leaving: leaving would
//! save and close the composition the window has just opened on.
//!
//! The core's answer comes from a worker, and the pane can be left while it is out. Leaving then
//! hands the composition over rather than saving and closing it, because the window is about to
//! open on it: closed, it would open holding no files, and its saves would store a second copy
//! beside the one the leave saved. [`moved_answer`] decides what the answer does.

use std::rc::Rc;

use adw::prelude::*;
use mailcal_bindings::{ComposeOrigin, ComposerState, MovedSignature};

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
        self.moving_composer = Some(submission.clone());
        let state = ComposerState {
            from: submission.from,
            to: submission.to,
            cc: submission.cc,
            bcc: submission.bcc,
            subject: submission.subject,
            document_json: submission.document_json,
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

    /// Whether `composition` is the pane's composer on its way into a window, so leaving the pane
    /// hands it over rather than saving and closing it.
    pub(super) fn is_moving(&self, composition: &str) -> bool {
        self.moving_composer
            .as_ref()
            .is_some_and(|moving| moving.request.composition == composition)
    }

    /// Settles a pop-out whose answer is in, and says whether its window opens. A Discard while
    /// it was out took the composer away for good, so a window that would open is closed instead;
    /// an answer that failed after the pane was left keeps what was written, in Drafts.
    pub(super) fn settle_moved(&mut self, composition: &str, opened: bool) -> bool {
        let moving = self
            .moving_composer
            .take_if(|moving| moving.request.composition == composition);
        let in_pane = self
            .composer
            .as_ref()
            .is_some_and(|open| open.composition == composition);
        match moved_answer(moving.is_some(), in_pane, opened) {
            MovedAnswer::Open => true,
            MovedAnswer::Nothing => false,
            MovedAnswer::Close => {
                self.close_composition(composition);
                false
            }
            MovedAnswer::SaveAndClose => {
                if let Some(submission) = &moving {
                    self.save_draft_and_close(submission);
                }
                false
            }
        }
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

/// What the core's answer to a pop-out does.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum MovedAnswer {
    /// The window opens on the composition.
    Open,
    /// Nothing more: the pane still has the composer, or it was discarded and the core never
    /// opened it again.
    Nothing,
    /// The core opened a composition a Discard has since taken away; it is closed unseen.
    Close,
    /// The open failed after the pane was left, so nothing holds the composer: what it held when
    /// it was popped out is saved to Drafts and the composition closed, as a leave would have.
    SaveAndClose,
}

/// `moving` is whether the pop-out is still standing (a Discard withdraws it), `in_pane` whether
/// the pane still shows the composer, and `opened` whether the core opened it again.
pub(super) fn moved_answer(moving: bool, in_pane: bool, opened: bool) -> MovedAnswer {
    match (moving, opened) {
        (true, true) => MovedAnswer::Open,
        (true, false) if !in_pane => MovedAnswer::SaveAndClose,
        (false, true) => MovedAnswer::Close,
        _ => MovedAnswer::Nothing,
    }
}

#[cfg(test)]
mod tests {
    use super::{MovedAnswer, moved_answer};

    #[test]
    fn a_pop_out_opens_its_window_whether_or_not_the_pane_was_left_meanwhile() {
        assert_eq!(moved_answer(true, true, true), MovedAnswer::Open);
        assert_eq!(moved_answer(true, false, true), MovedAnswer::Open);
    }

    #[test]
    fn a_failed_pop_out_keeps_what_was_written() {
        // Still in the pane: the composer stays where it is, holding everything.
        assert_eq!(moved_answer(true, true, false), MovedAnswer::Nothing);
        // Left meanwhile: nothing else holds it, so it goes to Drafts.
        assert_eq!(moved_answer(true, false, false), MovedAnswer::SaveAndClose);
    }

    #[test]
    fn a_composer_discarded_while_it_moved_never_opens() {
        assert_eq!(moved_answer(false, false, true), MovedAnswer::Close);
        assert_eq!(moved_answer(false, false, false), MovedAnswer::Nothing);
    }
}
