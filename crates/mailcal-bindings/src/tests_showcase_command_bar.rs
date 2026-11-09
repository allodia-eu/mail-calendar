//! The showcase's command bar: what the store screenshots show over the Inbox.
//!
//! Delete deletes for good only where there is no Trash to move to (`docs/command-bar.md`, rule
//! 6), so a seed without a Trash folder photographs "Delete permanently" over every inbox, in
//! every store, in every language.

use std::sync::mpsc;

use super::*;
use crate::tests::{ChannelObserver, NullLogger};

#[test]
fn every_showcase_inbox_offers_delete_to_trash() {
    for locale in [
        ShowcaseLocale::En,
        ShowcaseLocale::Nl,
        ShowcaseLocale::De,
        ShowcaseLocale::Fr,
        ShowcaseLocale::Es,
        ShowcaseLocale::It,
        ShowcaseLocale::Pt,
    ] {
        let (tx, _rx) = mpsc::channel();
        let app = MailcalApp::new_showcase(
            Box::new(ChannelObserver { tx }),
            Box::new(NullLogger),
            LogLevel::Info,
            "Europe/Amsterdam".to_owned(),
            locale,
        );
        let rows: Vec<SelectedRow> = app
            .mailbox_list()
            .rows
            .into_iter()
            .map(|row| match row {
                SnapshotRow::Flat { row } => SelectedRow::Message {
                    account: row.account,
                    key: row.key,
                },
                SnapshotRow::Thread { row } => SelectedRow::Thread {
                    account: row.account,
                    thread_id: row.thread_id,
                },
            })
            .collect();
        assert!(!rows.is_empty(), "{locale:?}: the inbox is seeded");
        for row in rows {
            let bar = app.command_bar(vec![row.clone()]);
            let delete = bar
                .sections
                .iter()
                .flat_map(|section| &section.items)
                .find(|item| {
                    matches!(
                        item.command,
                        Command::Act {
                            action: RowAction::Delete | RowAction::PermanentlyDelete
                        }
                    )
                })
                .expect("the bar carries Delete");
            assert_eq!(
                delete.command,
                Command::Act {
                    action: RowAction::Delete
                },
                "{locale:?}: {row:?} has a Trash to go to"
            );
            assert!(delete.enabled, "{locale:?}: {row:?}");
        }
    }
}
