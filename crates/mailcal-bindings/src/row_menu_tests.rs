//! The row menu across the FFI: a conversation's menu names its newest message, and a row the
//! list no longer shows gets no menu at all.

use std::{
    sync::mpsc,
    time::{Duration, Instant},
};

use crate::{
    Intent, LogLevel, MailcalApp, MessageKind, RowAction, SelectedRow, SnapshotRow, ViewMode,
    tests::{ChannelObserver, NullLogger},
};

#[test]
fn a_conversation_menu_crosses_the_ffi_naming_its_newest_message() {
    let (tx, rx) = mpsc::channel();
    let app = MailcalApp::new_demo(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        "Etc/UTC".to_owned(),
    );
    app.dispatch(Intent::SetViewMode {
        mode: ViewMode::Threaded,
    });
    app.dispatch(Intent::RefreshMail);
    // Waited on by state: the demo's reply and its original group into one conversation row.
    let deadline = Instant::now() + Duration::from_secs(5);
    let thread = loop {
        let found = app
            .mailbox_list()
            .rows
            .into_iter()
            .find_map(|row| match row {
                SnapshotRow::Thread { row } => Some(row),
                SnapshotRow::Flat { .. } => None,
            });
        if let Some(thread) = found {
            break thread;
        }
        assert!(
            Instant::now() < deadline,
            "no conversation row within the timeout"
        );
        let _ = rx.recv_timeout(Duration::from_millis(50));
    };

    let menu = app.row_menu(vec![SelectedRow::Thread {
        account: thread.account.clone(),
        thread_id: thread.thread_id.clone(),
    }]);
    let open = menu
        .sections
        .iter()
        .flat_map(|section| &section.items)
        .find(|item| item.action == RowAction::Open)
        .expect("a conversation menu opens its newest message");
    let target = open.target.as_ref().expect("Open is about one message");
    assert_eq!(target.key, thread.messages[0].key);
    assert_eq!(target.kind, MessageKind::Received);
    assert!(
        menu.sections
            .iter()
            .all(|section| !section.items.is_empty())
    );

    let gone = app.row_menu(vec![SelectedRow::Thread {
        account: thread.account,
        thread_id: "no-such-thread".to_owned(),
    }]);
    assert!(gone.sections.is_empty());
}
