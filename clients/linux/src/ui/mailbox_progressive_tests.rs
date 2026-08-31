//! Regressions for yielding a wholesale mailbox replacement back to GTK between row batches.

use std::collections::HashSet;

use adw::prelude::*;
use mailcal_bindings::{AccountRow, FlatRow, MailboxListSnapshot, SnapshotRow, ViewMode};

use super::{INITIAL_ROWS, ProgressiveRenderer};
use crate::ui::{AppInput, mailbox::tests::rendered_labels, model::blank_avatar};

fn snapshot(folder: &str) -> MailboxListSnapshot {
    let rows = (0..100)
        .map(|index| SnapshotRow::Flat {
            row: FlatRow {
                account: "fixture".to_owned(),
                key: format!("{folder}-{index}"),
                subject: format!("{folder} {index}"),
                from: "Sender".to_owned(),
                avatar: blank_avatar(),
                date: "2026-08-27".to_owned(),
                unread: false,
                flagged: false,
                has_attachment: false,
                preview: String::new(),
            },
        })
        .collect();
    MailboxListSnapshot {
        accounts: vec![AccountRow {
            id: "fixture".to_owned(),
            email: "person@example.test".to_owned(),
            expanded: true,
        }],
        selected_account: Some("fixture".to_owned()),
        selected: Some(folder.to_owned()),
        folders: Vec::new(),
        account_folders: Vec::new(),
        unified_unread: 0,
        mode: ViewMode::Flat,
        rows,
        total: 100,
        search_horizon: None,
    }
}

fn row_count(list: &gtk::ListBox) -> usize {
    let mut count = 0;
    while list.row_at_index(count).is_some() {
        count += 1;
    }
    usize::try_from(count).expect("the fixture row count fits usize")
}

pub(crate) fn a_new_folder_builds_only_the_visible_rows_synchronously() {
    let list = gtk::ListBox::new();
    let (sender, _receiver) = relm4::channel::<AppInput>();
    let mut renderer = ProgressiveRenderer::default();
    let expanded = HashSet::new();

    renderer.render(
        &list,
        &snapshot("archive"),
        &expanded,
        false,
        "UTC",
        &sender,
    );
    while gtk::glib::MainContext::default().pending() {
        gtk::glib::MainContext::default().iteration(false);
    }
    assert_eq!(row_count(&list), 100);

    renderer.render(&list, &snapshot("inbox"), &expanded, false, "UTC", &sender);
    assert_eq!(
        row_count(&list),
        INITIAL_ROWS,
        "a folder switch must yield before building rows below the viewport"
    );

    while gtk::glib::MainContext::default().pending() {
        gtk::glib::MainContext::default().iteration(false);
    }
    assert_eq!(row_count(&list), 100, "idle batches complete the list");

    // A second click before those idle batches finish cancels the first folder's work.
    renderer.render(&list, &snapshot("drafts"), &expanded, false, "UTC", &sender);
    renderer.render(&list, &snapshot("sent"), &expanded, false, "UTC", &sender);
    while gtk::glib::MainContext::default().pending() {
        gtk::glib::MainContext::default().iteration(false);
    }
    let labels = rendered_labels(list.upcast_ref());
    assert!(labels.iter().any(|label| label == "sent 99"));
    assert!(!labels.iter().any(|label| label.starts_with("drafts ")));
}
