//! Widget assertions for leaving a composer: that one navigation leaves it once. The pure
//! dirtiness rule is unit-tested beside it in [`super`], and the Discard question's own two
//! buttons in [`super::super::composer_discard`].

use super::super::{AppInput, reader::ComposerHost};

/// The pane answers a request once and ignores the re-renders that follow it, which is why each
/// navigation away from a draft has to arrive with its own number. Give two of them the same one,
/// the composer's generation, say, which does not move while one draft is open; and the second
/// click, made while the first was still being answered, gets no answer and goes nowhere. That
/// half lives in `AppModel::open_message`; this covers the pane it depends on.
pub(crate) fn each_navigation_gets_its_own_answer() {
    let pane = crate::ui::composer::ComposerPane::new();
    let (sender, receiver) = relm4::channel::<AppInput>();

    pane.check_draft(1, &sender);
    assert!(
        matches!(
            receiver.recv_sync(),
            Some(AppInput::ComposerUntouched(ComposerHost::Pane))
        ),
        "a pane with no draft has nothing to keep"
    );

    // The same request again is the re-render, not a new click. Nothing may follow it, which a
    // marker sent afterwards proves: it has to be the very next thing out of the channel.
    pane.check_draft(1, &sender);
    sender.emit(AppInput::KeepEditing);
    assert!(
        matches!(receiver.recv_sync(), Some(AppInput::KeepEditing)),
        "one navigation must not leave the draft twice"
    );

    pane.check_draft(2, &sender);
    assert!(
        matches!(
            receiver.recv_sync(),
            Some(AppInput::ComposerUntouched(ComposerHost::Pane))
        ),
        "the next navigation must get its own answer"
    );
}
