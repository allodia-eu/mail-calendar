//! The "your name" step: what it opens showing, and what each way out of it asks for.
//!
//! The step is raised over the running app once a connect returns, so no fixture boot reaches
//! it and the acceptance suite never sees it: these assertions are the only ones that do.
//!
//! Called from the crate's single `gtk::init` test.

use adw::prelude::*;

use super::{super::AppInput, SenderNameAsk, SenderNamePrompt};
use crate::{
    l10n,
    ui::{mail_actions::tests::button, setup_widget_tests::descendants},
};

const ACCOUNT: &str = "acct-1";

fn ask(suggestion: &str) -> SenderNameAsk {
    SenderNameAsk {
        account: ACCOUNT.to_owned(),
        suggestion: suggestion.to_owned(),
    }
}

/// Opens the step and hands back its window, with the parent kept alive for the test's length.
fn open(ask: &SenderNameAsk) -> (gtk::Window, SenderNamePrompt, relm4::Receiver<AppInput>) {
    let parent = gtk::Window::new();
    parent.present();
    let (sender, receiver) = relm4::channel::<AppInput>();
    let mut prompt = SenderNamePrompt::default();
    prompt.render(Some(ask), &parent, &sender);
    let window = prompt.window.clone().expect("the step is on screen");
    (window, prompt, receiver)
}

fn field(window: &gtk::Window) -> gtk::Entry {
    descendants::<gtk::Entry>(window.upcast_ref::<gtk::Widget>())
        .into_iter()
        .next()
        .expect("the step offers a field")
}

/// The step opens on what the provider suggested, and Continue asks for what is in the field.
///
/// Seeding is the whole point of asking after the connect rather than before it: where the
/// provider already knows the name, the user confirms it instead of typing it.
pub(crate) fn the_step_opens_seeded_and_continue_asks_for_the_field() {
    let (window, _prompt, receiver) = open(&ask("Ada Lovelace"));
    let entry = field(&window);
    assert_eq!(entry.text(), "Ada Lovelace", "seeded from the provider");

    entry.set_text("Ada King");
    button(
        window.upcast_ref::<gtk::Widget>(),
        l10n::setup_sender_name_continue(),
    )
    .expect("the step offers Continue")
    .emit_clicked();

    match receiver.recv_sync() {
        Some(AppInput::SetAccountSenderName { account, name }) => {
            assert_eq!(account, ACCOUNT, "the answer names the account it is about");
            assert_eq!(name, "Ada King");
        }
        other => panic!("Continue stores the typed name, got {other:?}"),
    }
}

/// An account whose provider holds no name opens on an empty field, never on an invented one.
///
/// Substituting the address or the login would put words in the sender's mouth
/// (`docs/sending.md` rule 3).
pub(crate) fn a_provider_with_no_name_opens_the_step_empty() {
    let (window, _prompt, _receiver) = open(&ask(""));
    assert!(
        field(&window).text().is_empty(),
        "nothing is invented for an account nobody has named"
    );
}

/// Skipping is one action, and it leaves the account nameless rather than storing anything.
pub(crate) fn skipping_stores_nothing() {
    let (window, _prompt, receiver) = open(&ask("Ada Lovelace"));
    button(
        window.upcast_ref::<gtk::Widget>(),
        l10n::setup_sender_name_skip(),
    )
    .expect("the step offers Skip")
    .emit_clicked();

    assert!(
        matches!(
            receiver.recv_sync(),
            Some(AppInput::DismissSenderNamePrompt)
        ),
        "skipping asks for no name at all"
    );
}

/// Closing the window is skipping. The account sending as a bare address is a state the app has
/// to be correct in, so the step needs no confirmation of its own; but it must still be cleared,
/// or the model would raise it again on the next render.
pub(crate) fn closing_the_window_is_skipping() {
    let (window, _prompt, receiver) = open(&ask("Ada Lovelace"));
    window.close();

    assert!(
        matches!(
            receiver.recv_sync(),
            Some(AppInput::DismissSenderNamePrompt)
        ),
        "closing the step dismisses it"
    );
}

/// A re-render of the same question leaves the open window alone, and a different one replaces
/// it.
///
/// Every model update re-renders. Rebuilding on each would take away whatever the user had
/// typed, mid-word; never rebuilding would leave the second account asking the first's question.
pub(crate) fn the_step_is_rebuilt_only_when_it_asks_about_something_else() {
    let parent = gtk::Window::new();
    parent.present();
    let (sender, _receiver) = relm4::channel::<AppInput>();
    let mut prompt = SenderNamePrompt::default();

    prompt.render(Some(&ask("Ada Lovelace")), &parent, &sender);
    let first = prompt.window.clone().expect("the step is on screen");
    field(&first).set_text("half-typed");

    prompt.render(Some(&ask("Ada Lovelace")), &parent, &sender);
    let again = prompt.window.clone().expect("the step is still on screen");
    assert_eq!(first, again, "the same question keeps the same window");
    assert_eq!(
        field(&again).text(),
        "half-typed",
        "and keeps what was typed into it"
    );

    let other = SenderNameAsk {
        account: "acct-2".to_owned(),
        suggestion: "Grace Hopper".to_owned(),
    };
    prompt.render(Some(&other), &parent, &sender);
    let replaced = prompt.window.clone().expect("the next account asks too");
    assert_ne!(first, replaced, "a different account gets its own step");
    assert_eq!(field(&replaced).text(), "Grace Hopper");

    prompt.render(None, &parent, &sender);
    assert!(prompt.window.is_none(), "nothing to ask closes the step");
}
