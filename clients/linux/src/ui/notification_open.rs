//! What a clicked new-mail notification does to the mailbox (`docs/background-sync.md`).
//!
//! The portal half is [`super::notifications`]; this is the half that knows where the message is.
//! The rule the three platforms that already had this agree on: the list on screen comes first,
//! so a message in view opens where it is without moving the mailbox off the folder the person
//! left it on. Only when it is not there does the mailbox move to the target's account, and then
//! for exactly **one** snapshot.
//!
//! That bound is the whole design. A message can be missing for one snapshot because the account
//! was only just selected, which is worth waiting for; it can also be missing for good, because
//! it was deleted or sits outside the loaded window. Without the bound the second case would drag
//! the view back to the mailbox on every snapshot the person afterwards caused, fighting whatever
//! they navigated to next.

use adw::prelude::*;
use mailcal_bindings::{Intent, MailboxListSnapshot, SnapshotRow};

use super::{
    AppModel, PrimaryView, avatar::AvatarData, mailbox::ThreadKey, model::OpenedMessage,
    notifications::NotificationTarget,
};

/// Where a named message was found in the list, and the conversation it had to be opened out of.
struct Found {
    message: OpenedMessage,
    /// The conversation holding it, which has to be disclosed for the reading pane to be showing
    /// a row the list also shows; `None` when the message is a row in its own right.
    thread: Option<ThreadKey>,
}

impl AppModel {
    /// Opens the message a clicked notification named.
    pub(super) fn open_notified_message(&mut self, target: NotificationTarget) {
        if self.open_notified_from_list(&target) {
            return;
        }
        // Not in the list on screen. Point the mailbox at the account that received it and let
        // the snapshot that produces answer; `retry_notified_message` is the one retry that earns.
        self.primary = PrimaryView::Mail;
        self.dispatch(Intent::SelectAccount {
            account: Some(target.account.clone()),
        });
        self.notification_wait = Some(target);
    }

    /// The one retry the account switch earns, spent whether or not it finds the message.
    pub(super) fn retry_notified_message(&mut self) {
        if let Some(target) = self.notification_wait.take() {
            self.open_notified_from_list(&target);
        }
    }

    fn open_notified_from_list(&mut self, target: &NotificationTarget) -> bool {
        let Some(found) = find_in_list(&self.snapshot, target) else {
            return false;
        };
        if let Some(thread) = found.thread {
            self.expanded_threads.insert(thread);
        }
        self.open_message(found.message);
        true
    }
}

/// The named message as the list currently holds it, or `None` when this snapshot does not.
///
/// A conversation is searched through rather than opened at its representative: the notification
/// named one message, and a thread's representative is the newest **in-scope** message, which on
/// a busy thread is not the one that was announced.
fn find_in_list(snapshot: &MailboxListSnapshot, target: &NotificationTarget) -> Option<Found> {
    snapshot.rows.iter().find_map(|listed| match listed {
        SnapshotRow::Flat { row } if row.account == target.account && row.key == target.key => {
            Some(Found {
                message: OpenedMessage::from_row(listed),
                thread: None,
            })
        }
        SnapshotRow::Thread { row } => {
            let message = row
                .messages
                .iter()
                .find(|message| message.account == target.account && message.key == target.key)?;
            Some(Found {
                message: OpenedMessage {
                    account: message.account.clone(),
                    key: message.key.clone(),
                    // The conversation's subject, as the expanded sub-row shows it: a message on
                    // a thread carries no subject of its own.
                    subject: row.subject.clone(),
                    from: message.from.clone(),
                    date: message.date.clone(),
                    avatar: AvatarData::from(&message.avatar),
                },
                thread: Some(ThreadKey::of(row)),
            })
        }
        SnapshotRow::Flat { .. } => None,
    })
}

/// Brings the mailbox window forward, for a click that reached this app from outside it.
///
/// A notification is answered by the desktop shell, so this process does not hold the foreground
/// and the window the message opened in may be behind everything else on screen.
pub(super) fn present_main_window() {
    if let Some(window) = relm4::main_adw_application().active_window() {
        window.present();
    }
}

#[cfg(test)]
mod tests {
    use mailcal_bindings::{FlatRow, SnapshotRow, ThreadMessage, ThreadRow};

    use super::{NotificationTarget, find_in_list};
    use crate::ui::model::{blank_avatar as avatar, empty_mailbox};

    fn flat(account: &str, key: &str) -> SnapshotRow {
        SnapshotRow::Flat {
            row: FlatRow {
                account: account.to_owned(),
                key: key.to_owned(),
                subject: "Quarterly report".to_owned(),
                from: "Jane".to_owned(),
                avatar: avatar(),
                date: "09:15".to_owned(),
                unread: true,
                flagged: false,
                has_attachment: false,
                preview: String::new(),
            },
        }
    }

    fn thread(account: &str, keys: &[&str]) -> SnapshotRow {
        SnapshotRow::Thread {
            row: ThreadRow {
                account: account.to_owned(),
                thread_id: "t1".to_owned(),
                latest_key: keys[0].to_owned(),
                subject: "Budget review".to_owned(),
                latest_from: "Jane".to_owned(),
                avatar: avatar(),
                latest_date: "09:15".to_owned(),
                message_count: u32::try_from(keys.len()).unwrap_or(u32::MAX),
                unread_count: 1,
                has_attachment: false,
                preview: String::new(),
                messages: keys
                    .iter()
                    .map(|key| ThreadMessage {
                        account: account.to_owned(),
                        key: (*key).to_owned(),
                        from: "Jane".to_owned(),
                        avatar: avatar(),
                        date: "09:15".to_owned(),
                        preview: String::new(),
                        unread: true,
                        outgoing: false,
                        has_attachment: false,
                    })
                    .collect(),
            },
        }
    }

    fn target(account: &str, key: &str) -> NotificationTarget {
        NotificationTarget {
            account: account.to_owned(),
            key: key.to_owned(),
        }
    }

    #[test]
    fn a_message_on_screen_is_found_where_it_is() {
        let mut snapshot = empty_mailbox();
        snapshot.rows = vec![flat("work", "m1"), flat("work", "m2")];

        let found = find_in_list(&snapshot, &target("work", "m2")).expect("the row on screen");
        assert_eq!(found.message.key, "m2");
        assert!(
            found.thread.is_none(),
            "a row in its own right discloses nothing"
        );
    }

    #[test]
    fn the_same_key_in_another_account_is_a_different_message() {
        // A provider key is unique within its account, never across them, which is why the
        // notification carries the pair rather than the key alone.
        let mut snapshot = empty_mailbox();
        snapshot.rows = vec![flat("personal", "m1")];

        assert!(find_in_list(&snapshot, &target("work", "m1")).is_none());
    }

    #[test]
    fn a_message_inside_a_conversation_opens_that_message_and_discloses_the_thread() {
        // Not the thread's representative: the notification named one message, and on a busy
        // thread the newest in-scope message is not the one that was announced.
        let mut snapshot = empty_mailbox();
        snapshot.rows = vec![thread("work", &["newest", "announced"])];

        let found = find_in_list(&snapshot, &target("work", "announced")).expect("the sub-row");
        assert_eq!(found.message.key, "announced");
        assert_eq!(
            found.message.subject, "Budget review",
            "a message on a thread carries no subject of its own"
        );
        assert!(
            found.thread.is_some(),
            "the reading pane must be showing a row the list also shows"
        );
    }

    #[test]
    fn a_message_this_snapshot_does_not_hold_is_not_opened_against_another() {
        let mut snapshot = empty_mailbox();
        snapshot.rows = vec![flat("work", "m1"), thread("work", &["m2", "m3"])];

        assert!(find_in_list(&snapshot, &target("work", "gone")).is_none());
    }
}
