//! Widget assertions for leaving a composer: that one navigation leaves it once, and that a reply
//! nobody typed into is not a draft. The pure dirtiness rule is unit-tested beside it in
//! [`super`], and the Discard question's own two buttons in [`super::super::composer_discard`].

use adw::prelude::*;

use super::super::{
    AppInput,
    composer::ComposerPane,
    composer_model::{ComposeContext, ComposeKind, new_composition},
    reader::ComposerHost,
};

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

/// A shown reply whose To is seeded and which nobody has typed into is not a draft: leaving it
/// saves nothing, and Discard does not ask about a message nobody wrote.
pub(crate) fn a_reply_nobody_typed_into_is_not_a_draft(window: &adw::ApplicationWindow) {
    let (sender, _receiver) = relm4::channel::<AppInput>();
    let pane = ComposerPane::new();
    let request = ComposeContext {
        kind: ComposeKind::Reply,
        host: ComposerHost::Pane,
        account: Some("fixture".to_owned()),
        key: Some("message".to_owned()),
        initial_to: "recipient@example.test".to_owned(),
        initial_cc: String::new(),
        initial_bcc: String::new(),
        subject: "Re: fixture".to_owned(),
        initial_body: None,
        stored_html: None,
        quote: None,
        initial_from: Some("fixture".to_owned()),
        seeds_signature: true,
        composition: new_composition(),
        files: Vec::new(),
    };
    pane.show(
        42,
        &request,
        &[("fixture".to_owned(), "sender@example.test".to_owned())],
        None,
        window,
        sender,
    );

    assert!(pane.is_active(42));
    assert!(pane.widget().first_child().is_some());
    let guard = pane
        .draft_cell()
        .borrow()
        .clone()
        .expect("the shown composer has its guard");
    assert!(
        !guard.header_edited(),
        "a reply nobody typed into is not a draft"
    );
    pane.teardown();
    assert!(pane.widget().first_child().is_none());
}
