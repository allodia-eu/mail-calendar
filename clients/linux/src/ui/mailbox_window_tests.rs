//! The two routes from a message row into a window of its own
//! (`docs/reading-window.md`), and the row that deliberately has neither.
//!
//! Functions rather than `#[test]`s, called from [`super::tests`], for the reason
//! [`super::thread_tests`] gives: the crate initialises GTK exactly once.
//!
//! The gesture is driven by emitting `pressed` on the row's own controller. A `GtkGestureClick`
//! cannot be fed a synthetic `GdkEvent` from here, and the assertion worth making is not that GTK
//! counts presses: it is that the second one, and only the second one, asks for a window.

use std::rc::Rc;

use adw::prelude::*;
use mailcal_bindings::{
    FlatRow, MenuMessage, MessageKind, RowAction, RowMenu, RowMenuItem, RowMenuSection,
    SelectedRow, ThreadMessage, ThreadRow,
};

use super::{RowMenus, flat_row, thread_message_row, thread_row};
use crate::{
    l10n,
    ui::{
        AppInput,
        mail_actions::tests::{button, popover_content},
        row_menu::RowPick,
        timestamps::MailClock,
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
        draft: false,
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
        flagged: false,
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
        flagged: false,
        outgoing: false,
        draft: false,
        has_attachment: false,
    }
}

/// The row's own primary-button gesture, or `None` where the row carries none.
fn double_click(row: &gtk::Widget) -> Option<gtk::GestureClick> {
    let controllers = row.observe_controllers();
    (0..controllers.n_items())
        .filter_map(|index| controllers.item(index))
        .filter_map(|controller| controller.downcast::<gtk::GestureClick>().ok())
        .find(|gesture| gesture.button() == gtk::gdk::BUTTON_PRIMARY)
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
    let row = flat_row(
        &message_row(),
        &RowMenus::default(),
        &MailClock::new("UTC", true),
        &sender,
    );
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
    let sub_row = thread_message_row(
        &thread,
        &thread_message(),
        &RowMenus::default(),
        &MailClock::new("UTC", true),
        &sender,
    );
    assert_eq!(
        pressed(sub_row.upcast_ref::<gtk::Widget>(), 2, &sender, &receiver),
        ["window message-b"]
    );

    let header = thread_row(
        &thread,
        false,
        &RowMenus::default(),
        &MailClock::new("UTC", true),
        &sender,
    );
    assert!(
        double_click(header.upcast_ref::<gtk::Widget>()).is_none(),
        "a conversation header keeps whatever a double-click already did there"
    );
}

/// The gesture is not reachable from the keyboard and cannot be invoked by a screen reader, so
/// every message row names the same thing on its menu; that item, not the gesture, is what the
/// capability matrix claims. The core puts it on a desktop's menu; each message row, a flat one
/// and one inside a conversation, draws that menu and opens the window it names.
pub(crate) fn every_message_row_offers_the_window_by_name() {
    let (sender, receiver) = relm4::channel::<AppInput>();
    let menus = RowMenus::default();
    menus.source.answer_with(Rc::new(|rows| {
        let Some(SelectedRow::Message { account, key }) = rows.into_iter().next() else {
            return RowMenu::default();
        };
        let target = MenuMessage {
            account,
            key,
            kind: MessageKind::Received,
        };
        RowMenu {
            sections: vec![RowMenuSection {
                items: vec![RowMenuItem {
                    action: RowAction::OpenInWindow,
                    enabled: true,
                    target: Some(target),
                    confirm: false,
                }],
            }],
        }
    }));
    let thread = conversation();
    let rows: [gtk::Widget; 2] = [
        flat_row(
            &message_row(),
            &menus,
            &MailClock::new("UTC", true),
            &sender,
        )
        .upcast(),
        thread_message_row(
            &thread,
            &thread_message(),
            &menus,
            &MailClock::new("UTC", true),
            &sender,
        )
        .upcast(),
    ];
    for (row, key) in rows.iter().zip(["message-a", "message-b"]) {
        let content = popover_content(row)
            .and_downcast::<gtk::Box>()
            .expect("the row's menu");
        let row = SelectedRow::Message {
            account: "account-a".to_owned(),
            key: key.to_owned(),
        };
        crate::ui::mail_actions_menu::fill(&content, &row, &menus, &sender);
        button(content.upcast_ref(), l10n::action_open_in_window())
            .expect("every message row names the window on its menu")
            .emit_clicked();
        assert!(matches!(
            receiver.recv_sync(),
            Some(AppInput::RowMenuPick(pick))
                if matches!(&*pick, RowPick::OpenInWindow(message) if message.key == key)
        ));
    }
}

/// A message row and a conversation row lead the same way, the unread dot and then the avatar,
/// so every avatar in the list stands in one column; and neither draws an arrow, because the
/// avatar carries the row's own action (`row_action`).
pub(crate) fn every_mail_row_leads_with_the_dot_then_the_avatar() {
    let (sender, _receiver) = relm4::channel::<AppInput>();
    let clock = MailClock::new("UTC", true);
    let menus = RowMenus::default();
    let flat = flat_row(&message_row(), &menus, &clock, &sender);
    let header = thread_row(&conversation(), false, &menus, &clock, &sender);
    for (kind, row) in [
        ("message", flat.upcast_ref::<gtk::Widget>()),
        ("conversation", header.upcast_ref::<gtk::Widget>()),
    ] {
        assert_eq!(leading(row), ["dot", "avatar"], "a {kind} row");
        assert!(
            !buttons(row)
                .iter()
                .any(|button| button.icon_name().as_deref() == Some(crate::ui::icons::NEXT)),
            "a {kind} row draws no arrow"
        );
    }
}

/// The unread dot and the avatar's action button under `root`, in the order they are drawn.
fn leading(root: &gtk::Widget) -> Vec<&'static str> {
    let mut found = Vec::new();
    let mut stack = vec![root.clone()];
    while let Some(widget) = stack.pop() {
        if let Some(button) = widget.downcast_ref::<gtk::Button>() {
            if !button.can_target() {
                found.push("avatar");
                continue;
            }
        } else if widget
            .downcast_ref::<gtk::DrawingArea>()
            .is_some_and(|area| area.content_width() == 8)
        {
            found.push("dot");
        }
        let mut children = Vec::new();
        let mut child = widget.first_child();
        while let Some(next) = child {
            child = next.next_sibling();
            children.push(next);
        }
        stack.extend(children.into_iter().rev());
    }
    found
}

/// Every `GtkButton` under `root`.
fn buttons(root: &gtk::Widget) -> Vec<gtk::Button> {
    let mut found = Vec::new();
    let mut stack = vec![root.clone()];
    while let Some(widget) = stack.pop() {
        if let Some(button) = widget.downcast_ref::<gtk::Button>() {
            found.push(button.clone());
        }
        let mut child = widget.first_child();
        while let Some(next) = child {
            child = next.next_sibling();
            stack.push(next);
        }
    }
    found
}
