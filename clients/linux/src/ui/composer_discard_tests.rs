//! Widget assertions for the "Discard draft?" question: what each of its two buttons sends.
//!
//! A child of [`super`], so the question and the assertions on it live together and neither has
//! to widen the other's visibility to be reachable.

use gtk::prelude::{ButtonExt, Cast, GtkWindowExt};

use super::{super::AppInput, DiscardDraftDialog};
use crate::{
    l10n,
    ui::mail_actions::tests::{button, labels},
};

/// The destructive answer is only ever the button that says so; every other way out of the
/// dialog, the keep button included, keeps the draft.
pub(crate) fn the_question_discards_only_on_its_discard_button() {
    let parent = gtk::Window::new();
    parent.present();

    let (sender, receiver) = relm4::channel::<AppInput>();
    let mut dialog = DiscardDraftDialog::default();
    dialog.render(true, &parent, &sender);
    let window = dialog.window.as_ref().expect("discard question").clone();
    assert!(
        labels(window.upcast_ref::<gtk::Widget>())
            .iter()
            .any(|label| label == l10n::compose_discard_message()),
        "the question must say what is lost"
    );
    button(window.upcast_ref::<gtk::Widget>(), l10n::action_discard())
        .expect("discard action")
        .emit_clicked();
    assert!(
        matches!(receiver.recv_sync(), Some(AppInput::DiscardDraft)),
        "the discard button must throw the draft away"
    );

    let (sender, receiver) = relm4::channel::<AppInput>();
    let mut dialog = DiscardDraftDialog::default();
    dialog.render(true, &parent, &sender);
    let window = dialog.window.as_ref().expect("discard question").clone();
    button(
        window.upcast_ref::<gtk::Widget>(),
        l10n::action_keep_editing(),
    )
    .expect("keep-editing action")
    .emit_clicked();
    assert!(
        matches!(receiver.recv_sync(), Some(AppInput::KeepEditing)),
        "keeping the draft must not discard it"
    );

    parent.close();
}
