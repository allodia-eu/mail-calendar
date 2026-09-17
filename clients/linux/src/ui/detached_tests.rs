//! What a window beside the mailbox is, and what it reports when it goes
//! (`docs/reading-window.md`).
//!
//! Functions rather than `#[test]`s, called from [`crate::ui::mailbox::tests`], because the crate
//! initialises GTK exactly once.

use adw::prelude::*;

use super::{composer_window, reading_window};
use crate::ui::AppInput;

/// Asks `window` to close, and reports what came back.
///
/// The request is emitted rather than driven through `gtk_window_close`, which is a no-op on a
/// window that was never realised: realising one of these means realising the `WebKitWebView`
/// inside it, and a web process is not something a widget test should start. What is worth
/// asserting is ours either way, that the window reports itself gone and says which one it was.
///
/// Read behind a sentinel, so a close that reports nothing fails the test rather than blocking
/// it.
fn closing(
    window: &adw::Window,
    sender: &relm4::Sender<AppInput>,
    receiver: &relm4::Receiver<AppInput>,
) -> Vec<String> {
    window.emit_by_name::<bool>("close-request", &[]);
    sender.emit(AppInput::ClearSelection);
    let mut seen = Vec::new();
    while let Some(input) = receiver.recv_sync() {
        match input {
            AppInput::ClearSelection => break,
            AppInput::CloseReadingWindow(window) => seen.push(format!("reading {window}")),
            AppInput::CloseComposerWindow(id) => seen.push(format!("composer {id}")),
            other => seen.push(format!("{other:?}")),
        }
    }
    seen
}

/// A reading window is a *view of the running app*: it is not an application window of its own, so
/// the app still ends when the mailbox does rather than living on as a scatter of message windows.
/// Closing it frees the body on both sides, and the id it reports is what the core drops its slot
/// by.
///
/// And it is the mailbox's peer, not its child. A transient parent is how GTK stacks a window
/// above another one, so a reading window built that way could never be put behind the mailbox
/// (`docs/reading-window.md`).
pub(crate) fn a_reading_window_is_the_mailbox_peer_and_not_a_second_app() {
    let (sender, receiver) = relm4::channel::<AppInput>();
    let open = reading_window("account/message-a", &sender);

    assert!(
        open.window.transient_for().is_none(),
        "a reading window the mailbox cannot be raised over is not a peer of it"
    );
    assert!(
        open.window.application().is_none(),
        "a detached window must not hold the application open on its own"
    );
    assert_eq!(
        open.window.content().as_ref(),
        Some(open.view.widget().clone().upcast_ref::<gtk::Widget>()),
        "the window's whole content is the reading view the pane also draws"
    );
    assert_eq!(
        closing(&open.window, &sender, &receiver),
        ["reading account/message-a"]
    );
}

/// Closing a composer window discards the draft exactly as Cancel does, and asks no more than
/// Cancel does: one report, no question.
pub(crate) fn closing_a_composer_window_discards_without_a_question() {
    let (sender, receiver) = relm4::channel::<AppInput>();
    let open = composer_window(7, &sender);

    assert!(
        open.window.transient_for().is_none(),
        "a composer window the mailbox cannot be raised over is not a peer of it"
    );
    assert_eq!(closing(&open.window, &sender, &receiver), ["composer 7"]);
}
