//! Widget assertions for leaving a composer: that one navigation leaves it once, that a reply
//! nobody typed into is not a draft, and that only the pane's composer can be popped out. The pure
//! dirtiness rule is unit-tested beside it in [`super`], and the Discard question's own two buttons
//! in [`super::super::composer_discard`].

use adw::prelude::*;

use super::super::{
    AppInput,
    composer::ComposerPane,
    composer_model::{ComposeContext, ComposeKind, new_composition},
    icons,
    reader::ComposerHost,
    setup_widget_tests::descendants,
};
use crate::l10n;

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
    let request = reply(ComposerHost::Pane);
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

/// The composer is one column that scrolls as a whole: the header, the action row, the notes, the
/// files, then the editor, and nothing in it with a vertical scroll of its own
/// (`docs/composer-layout.md`, rules 1 and 2). A popup is drawn over the column, so one inside a
/// popover is not in it.
pub(crate) fn the_composer_is_one_column_in_the_contracts_order(window: &adw::ApplicationWindow) {
    let (sender, _receiver) = relm4::channel::<AppInput>();
    let pane = ComposerPane::new();
    pane.show(
        44,
        &reply(ComposerHost::Pane),
        &[("fixture".to_owned(), "sender@example.test".to_owned())],
        None,
        window,
        sender,
    );
    let scrolls: Vec<gtk::ScrolledWindow> =
        descendants::<gtk::ScrolledWindow>(pane.widget().upcast_ref())
            .into_iter()
            .filter(|scroll| scroll.ancestor(gtk::Popover::static_type()).is_none())
            .collect();
    assert_eq!(scrolls.len(), 1, "one scroll, around the whole composer");
    let column = &scrolls[0];
    assert_eq!(column.vscrollbar_policy(), gtk::PolicyType::Automatic);
    assert_eq!(column.hscrollbar_policy(), gtk::PolicyType::Never);
    let content = descendants::<gtk::Box>(column.upcast_ref())
        .into_iter()
        .next()
        .expect("the column holds the composer");
    let order: Vec<gtk::Widget> =
        std::iter::successors(content.first_child(), gtk::Widget::next_sibling).collect();
    assert_eq!(order.len(), 6, "{order:?}");
    assert!(order[0].is::<gtk::Grid>(), "the header first");
    assert!(
        descendants::<gtk::Button>(&order[1])
            .iter()
            .any(|button| button.label().as_deref() == Some(l10n::action_attach())),
        "then the action row"
    );
    assert!(
        order[2].is::<gtk::Label>() && order[3].is::<gtk::Label>(),
        "then the notes"
    );
    assert!(order[4].is::<gtk::FlowBox>(), "then the files");
    assert!(
        !descendants::<webkit6::WebView>(&order[5]).is_empty(),
        "and the message last"
    );
    pane.teardown();
}

fn reply(host: ComposerHost) -> ComposeContext {
    ComposeContext {
        kind: ComposeKind::Reply,
        host,
        initial_to: "recipient@example.test".to_owned(),
        initial_cc: String::new(),
        initial_bcc: String::new(),
        subject: "Re: fixture".to_owned(),
        initial_body: None,
        stored_html: None,
        quote: None,
        quote_attribution: None,
        has_quote: false,
        initial_from: Some("fixture".to_owned()),
        seeds_signature: true,
        composition: new_composition(),
        attachments_failed: false,
        moved: None,
    }
}

/// Only the pane's composer offers to pop out, as a button carrying the vendored glyph and the
/// name a screen reader reads: a composer already in a window has nowhere further to go.
pub(crate) fn only_the_panes_composer_offers_to_pop_out(window: &adw::ApplicationWindow) {
    for (host, offered) in [(ComposerHost::Pane, true), (ComposerHost::Window(7), false)] {
        let (sender, _receiver) = relm4::channel::<AppInput>();
        let pane = ComposerPane::new();
        pane.show(
            43,
            &reply(host),
            &[("fixture".to_owned(), "sender@example.test".to_owned())],
            None,
            window,
            sender,
        );
        let pop_out: Vec<gtk::Button> = descendants::<gtk::Button>(pane.widget().upcast_ref())
            .into_iter()
            .filter(|button| {
                button.tooltip_text().as_deref() == Some(l10n::action_open_in_window())
            })
            .collect();
        assert_eq!(pop_out.len(), usize::from(offered), "{host:?}");
        if let Some(button) = pop_out.first() {
            assert_eq!(button.icon_name().as_deref(), Some(icons::POP_OUT));
        }
        pane.teardown();
    }
}
