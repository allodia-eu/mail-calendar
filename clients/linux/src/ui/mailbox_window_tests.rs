//! The two routes from a message row into a window of its own
//! (`docs/reading-window.md`), and the row that deliberately has neither.
//!
//! Functions rather than `#[test]`s, called from [`super::tests`], for the reason
//! [`super::thread_tests`] gives: the crate initialises GTK exactly once.
//!
//! The gesture is driven by emitting `pressed` on the row's own controller. A `GtkGestureClick`
//! cannot be fed a synthetic `GdkEvent` from here, and the assertion worth making is not that GTK
//! counts presses: it is that the second one, and only the second one, asks for a window.

use adw::prelude::*;
use mailcal_bindings::{FlatRow, SnapshotRow, ThreadMessage, ThreadRow};

use super::{flat_row, thread_message_row, thread_row};
use crate::{
    l10n,
    ui::{
        AppInput,
        mail_actions::tests::{button, popover_content},
        model::OpenedMessage,
    },
};

fn message_row() -> FlatRow {
    FlatRow {
        avatar: crate::ui::model::blank_avatar(),
        account: "account-a".to_owned(),
        key: "message-a".to_owned(),
        subject: "Quarterly planning".to_owned(),
        from: "Sender".to_owned(),
        date: "2026-07-20".to_owned(),
        unread: false,
        flagged: false,
        has_attachment: false,
        preview: String::new(),
    }
}

fn conversation() -> ThreadRow {
    ThreadRow {
        avatar: crate::ui::model::blank_avatar(),
        account: "account-a".to_owned(),
        thread_id: "thread-a".to_owned(),
        latest_key: "message-b".to_owned(),
        subject: "Quarterly planning".to_owned(),
        latest_from: "Sender".to_owned(),
        latest_date: "2026-07-20".to_owned(),
        message_count: 2,
        unread_count: 0,
        has_attachment: false,
        preview: String::new(),
        messages: Vec::new(),
    }
}

fn thread_message() -> ThreadMessage {
    ThreadMessage {
        avatar: crate::ui::model::blank_avatar(),
        account: "account-a".to_owned(),
        key: "message-b".to_owned(),
        from: "Sender".to_owned(),
        date: "2026-07-20".to_owned(),
        preview: String::new(),
        unread: false,
        outgoing: false,
        has_attachment: false,
    }
}

/// The row's own primary-button gesture, or `None` where the row carries none.
fn double_click(row: &gtk::Widget) -> Option<gtk::GestureClick> {
    let controllers = row.observe_controllers();
    (0..controllers.n_items())
        .filter_map(|index| controllers.item(index))
        .find_map(|controller| controller.downcast::<gtk::GestureClick>().ok())
}

/// Presses `row`'s gesture `presses` times over, and reports everything that came back.
///
/// Read behind a sentinel, because the interesting answer is often *nothing*: a receiver with no
/// sentinel in it blocks rather than reporting an empty queue.
fn pressed(
    row: &gtk::Widget,
    presses: i32,
    sender: &relm4::Sender<AppInput>,
    receiver: &relm4::Receiver<AppInput>,
) -> Vec<String> {
    if let Some(gesture) = double_click(row) {
        gesture.emit_by_name::<()>("pressed", &[&presses, &0.0_f64, &0.0_f64]);
    }
    sender.emit(AppInput::ClearSelection);
    let mut seen = Vec::new();
    while let Some(input) = receiver.recv_sync() {
        match input {
            AppInput::ClearSelection => break,
            AppInput::OpenMessageInWindow(message) => seen.push(format!("window {}", message.key)),
            other => seen.push(format!("{other:?}")),
        }
    }
    seen
}

/// A double-click opens the row's message in a window; one click asks for no window at all, and
/// the click `GtkListBox` reads is what still opens the message in the pane.
pub(crate) fn a_second_click_asks_for_a_window_and_a_first_click_asks_for_nothing() {
    let (sender, receiver) = relm4::channel::<AppInput>();
    let row = flat_row(&message_row(), false, "UTC", &sender);
    let widget = row.upcast_ref::<gtk::Widget>().clone();

    assert!(
        pressed(&widget, 1, &sender, &receiver).is_empty(),
        "a single click is the list's own, and opens the message in the pane"
    );
    assert_eq!(
        pressed(&widget, 2, &sender, &receiver),
        ["window message-a"]
    );
}

/// A message inside an expanded conversation is a message row, so it opens in a window too. The
/// conversation's own header is not one: a double-click there belongs to its disclosure.
pub(crate) fn a_conversations_messages_open_in_windows_and_its_header_does_not() {
    let (sender, receiver) = relm4::channel::<AppInput>();
    let thread = conversation();
    let sub_row = thread_message_row(&thread, &thread_message(), "UTC", &sender);
    assert_eq!(
        pressed(sub_row.upcast_ref::<gtk::Widget>(), 2, &sender, &receiver),
        ["window message-b"]
    );

    let header = thread_row(&thread, false, "UTC", &sender);
    assert!(
        double_click(header.upcast_ref::<gtk::Widget>()).is_none(),
        "a conversation header keeps whatever a double-click already did there"
    );
}

/// The gesture is not reachable from the keyboard and cannot be invoked by a screen reader, so
/// every message row names the same thing on its menu; that item, not the gesture, is what the
/// capability matrix claims.
pub(crate) fn every_message_row_offers_the_window_by_name() {
    let (sender, receiver) = relm4::channel::<AppInput>();
    let row = message_row();
    let opened = OpenedMessage::from_row(&SnapshotRow::Flat { row: row.clone() });

    for menu in [
        crate::ui::mail_actions_menu::message_menu_button(&row, &opened, false, &sender),
        crate::ui::mail_actions_menu::message_window_menu_button(&opened, &sender),
    ] {
        let content = popover_content(menu.upcast_ref::<gtk::Widget>()).expect("the row's menu");
        button(&content, l10n::action_open_in_window())
            .expect("every message row names the window on its menu")
            .emit_clicked();
        assert!(matches!(
            receiver.recv_sync(),
            Some(AppInput::OpenMessageInWindow(message)) if message.key == "message-a"
        ));
    }
}
