//! The command bar across the FFI: every action on it is the row menu's own item over the same
//! selection, and over nothing only Sync responds.

use std::{
    sync::mpsc,
    time::{Duration, Instant},
};

use crate::{
    Command, Intent, LogLevel, MailcalApp, SelectedRow, SnapshotRow, ViewMode,
    tests::{ChannelObserver, NullLogger},
};

#[test]
fn the_bar_carries_the_row_menus_own_items() {
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
    let rows = vec![SelectedRow::Thread {
        account: thread.account,
        thread_id: thread.thread_id,
    }];

    let bar = app.command_bar(rows.clone());
    let menu: Vec<_> = app
        .row_menu(rows)
        .sections
        .into_iter()
        .flat_map(|section| section.items)
        .collect();
    let acts: Vec<_> = bar
        .sections
        .iter()
        .flat_map(|section| &section.items)
        .chain(&bar.menu)
        .filter_map(|item| match item.command {
            Command::Act { action } => Some((action, item)),
            Command::Sync => None,
        })
        .collect();
    assert!(!acts.is_empty());
    for (action, item) in acts {
        let own = menu
            .iter()
            .find(|m| m.action == action)
            .expect("a single row's menu holds every bar action");
        assert_eq!(item.enabled, own.enabled, "{action:?}");
        assert_eq!(item.target, own.target, "{action:?}");
        assert_eq!(item.confirm, own.confirm, "{action:?}");
    }

    let idle = app.command_bar(Vec::new());
    for item in idle
        .sections
        .iter()
        .flat_map(|s| &s.items)
        .chain(&idle.menu)
    {
        assert_eq!(
            item.enabled,
            item.command == Command::Sync,
            "{:?}",
            item.command
        );
    }
}
