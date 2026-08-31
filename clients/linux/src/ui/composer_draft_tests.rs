//! Widget assertions for the unsaved-draft question. The pure dirtiness rule is unit-tested
//! beside it in [`super`]; this covers what the two buttons actually send.

use gtk::prelude::{ButtonExt, Cast, GtkWindowExt};

use super::{super::AppInput, DiscardDraftDialog};
use crate::{
    l10n,
    ui::mail_actions::tests::{button, labels},
};

/// The destructive answer is only ever the button that says so; every other way out of the
/// dialog, the keep button included, keeps the draft.
pub(crate) fn the_draft_question_discards_only_on_the_discard_button() {
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

/// The pane answers a request once and ignores the re-renders that follow it, which is why each
/// navigation away from a draft has to arrive with its own number. Give two of them the same one,
/// the composer's generation, say, which does not move while one draft is open; and the second
/// click, the one after "Keep editing", gets no answer and goes nowhere. That half lives in
/// `AppModel::open_message`; this covers the guard it depends on.
pub(crate) fn each_navigation_gets_its_own_answer() {
    let pane = crate::ui::composer::ComposerPane::new();
    let (sender, receiver) = relm4::channel::<AppInput>();

    pane.check_draft(1, &sender);
    assert!(
        matches!(
            receiver.recv_sync(),
            Some(AppInput::ComposerDraftChecked(false))
        ),
        "a pane with no draft has nothing to lose"
    );

    // The same request again is the re-render, not a new click. Nothing may follow it, which a
    // marker sent afterwards proves: it has to be the very next thing out of the channel.
    pane.check_draft(1, &sender);
    sender.emit(AppInput::KeepEditing);
    assert!(
        matches!(receiver.recv_sync(), Some(AppInput::KeepEditing)),
        "one navigation must not be asked about twice"
    );

    pane.check_draft(2, &sender);
    assert!(
        matches!(
            receiver.recv_sync(),
            Some(AppInput::ComposerDraftChecked(false))
        ),
        "the next navigation must get its own answer"
    );
}
