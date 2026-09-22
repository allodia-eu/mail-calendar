//! Pure Linux-client state projections shared by the GTK widgets.

use std::collections::HashSet;

use mailcal_bindings::{
    Avatar, MailboxListSnapshot, ReadingSnapshot, SnapshotRow, Swatch, ThreadRow, ViewMode,
};

use super::{avatar::AvatarData, mailbox::ThreadKey};

pub(super) fn empty_mailbox() -> MailboxListSnapshot {
    MailboxListSnapshot {
        accounts: Vec::new(),
        selected_account: None,
        folders: Vec::new(),
        account_folders: Vec::new(),
        unified_unread: 0,
        unified_expanded: true,
        selected: None,
        mode: ViewMode::Threaded,
        rows: Vec::new(),
        total: 0,
        // The empty snapshot stands in before a search has run, so there is no depth to state.
        search_horizon: None,
        // Nor is there a folder on screen to explain the emptiness of.
        empty_reason: None,
        // No Outbox: this client draws none yet (`docs/sending.md`, "Known gaps"), and an
        // empty one would take the row off screen in any case (`docs/folder-pane.md`,
        // rule 18).
        showing_outbox: false,
        outbox: Vec::new(),
    }
}

/// An avatar with no letters, for a row this client builds by hand.
///
/// Deliberately not a `Default` on the FFI record: a blank avatar has to be something a caller
/// chose, never something it got by forgetting a field.
pub(super) fn blank_avatar() -> Avatar {
    Avatar {
        initials: String::new(),
        light: Swatch {
            background: String::new(),
            text: String::new(),
            border: String::new(),
        },
        dark: Swatch {
            background: String::new(),
            text: String::new(),
            border: String::new(),
        },
        image_path: None,
    }
}

pub(super) fn empty_reading() -> ReadingSnapshot {
    ReadingSnapshot {
        key: String::new(),
        from: String::new(),
        avatar: blank_avatar(),
        to: String::new(),
        cc: String::new(),
        bcc: String::new(),
        html: None,
        plain: None,
        has_remote_images: false,
        load_error: false,
        attachments: Vec::new(),
        invitation: None,
        pending: false,
    }
}

/// The representative message selected from a flat or threaded mailbox row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OpenedMessage {
    pub(crate) account: String,
    pub(crate) key: String,
    pub(crate) subject: String,
    pub(crate) from: String,
    pub(crate) date: String,
    pub(crate) avatar: AvatarData,
}

impl OpenedMessage {
    pub(crate) fn from_row(row: &SnapshotRow) -> Self {
        match row {
            SnapshotRow::Flat { row } => Self {
                account: row.account.clone(),
                key: row.key.clone(),
                subject: row.subject.clone(),
                from: row.from.clone(),
                date: row.date.clone(),
                avatar: AvatarData::from(&row.avatar),
            },
            SnapshotRow::Thread { row } => Self::from_thread(row),
        }
    }

    /// A conversation opens its representative; the latest **in-scope** message, the one the row
    /// summarises; so a Sent reply filed in another folder doesn't take the reading pane.
    pub(crate) fn from_thread(row: &ThreadRow) -> Self {
        Self {
            account: row.account.clone(),
            key: row.latest_key.clone(),
            subject: row.subject.clone(),
            from: row.latest_from.clone(),
            date: row.latest_date.clone(),
            avatar: AvatarData::from(&row.avatar),
        }
    }
}

/// The messages the list currently offers as reading stops, in display order.
pub(crate) fn readable_stops(
    snapshot: &MailboxListSnapshot,
    expanded: &HashSet<ThreadKey>,
) -> Vec<OpenedMessage> {
    snapshot
        .rows
        .iter()
        .flat_map(|row| match row {
            SnapshotRow::Flat { .. } => vec![OpenedMessage::from_row(row)],
            SnapshotRow::Thread { row } if expanded.contains(&ThreadKey::of(row)) => row
                .messages
                .iter()
                .map(|message| OpenedMessage {
                    account: message.account.clone(),
                    key: message.key.clone(),
                    subject: row.subject.clone(),
                    from: message.from.clone(),
                    date: message.date.clone(),
                    avatar: AvatarData::from(&message.avatar),
                })
                .collect(),
            SnapshotRow::Thread { row } => vec![OpenedMessage::from_thread(row)],
        })
        .collect()
}

/// Per-message reading state owned by the Linux host.
pub(crate) struct ReadingState {
    pub(crate) opened: Option<OpenedMessage>,
    pub(crate) snapshot: ReadingSnapshot,
    pub(crate) load_remote_images: bool,
}

impl ReadingState {
    pub(crate) fn new(snapshot: ReadingSnapshot) -> Self {
        Self {
            opened: None,
            snapshot,
            load_remote_images: false,
        }
    }

    pub(crate) fn open(&mut self, message: OpenedMessage) {
        self.opened = Some(message);
        self.load_remote_images = false;
    }

    pub(crate) fn close(&mut self) {
        self.opened = None;
        self.load_remote_images = false;
    }

    pub(crate) fn matches_opened(&self) -> bool {
        self.opened
            .as_ref()
            .is_some_and(|message| message.key == self.snapshot.key)
    }

    /// Whether the opened message's *body* has arrived, not merely its key. The core publishes a
    /// reading surface as soon as the selection changes, so a snapshot can carry the right key with
    /// neither body part filled in; and everything that seeds a composer from the body
    /// ([`super::composer_quote::quote_seed`]) yields nothing at that point.
    ///
    /// Only showcase mode waits on this, so it is compiled out of a release build with the rest of
    /// that path.
    #[cfg(any(debug_assertions, feature = "dev-harness"))]
    pub(crate) fn body_arrived(&self) -> bool {
        self.matches_opened()
            && (self.snapshot.html.as_deref().is_some_and(|b| !b.is_empty())
                || self
                    .snapshot
                    .plain
                    .as_deref()
                    .is_some_and(|b| !b.is_empty()))
    }
}

/// The message the reading pane opens after an archive or Trash move removes `opened` from the
/// list: the next one down, or the one above when it was last.
pub(crate) fn message_after_removing(
    opened: &OpenedMessage,
    stops: &[OpenedMessage],
) -> Option<OpenedMessage> {
    let index = stops
        .iter()
        .position(|candidate| candidate.account == opened.account && candidate.key == opened.key)?;
    stops
        .get(index + 1)
        .or_else(|| {
            index
                .checked_sub(1)
                .and_then(|previous| stops.get(previous))
        })
        .cloned()
}

#[cfg(test)]
mod tests {
    use mailcal_bindings::{FlatRow, ReadingSnapshot, SnapshotRow, ThreadRow};

    use super::{AvatarData, OpenedMessage, ReadingState, blank_avatar, message_after_removing};

    fn opened(account: &str, key: &str) -> OpenedMessage {
        OpenedMessage {
            account: account.to_owned(),
            key: key.to_owned(),
            subject: format!("Subject {key}"),
            from: format!("Sender {key}"),
            date: format!("Date {key}"),
            avatar: AvatarData::from(&blank_avatar()),
        }
    }

    fn empty_reading(key: &str) -> ReadingSnapshot {
        ReadingSnapshot {
            avatar: crate::ui::model::blank_avatar(),
            key: key.to_owned(),
            from: String::new(),
            to: String::new(),
            cc: String::new(),
            bcc: String::new(),
            html: None,
            plain: None,
            has_remote_images: false,
            load_error: false,
            attachments: Vec::new(),
            invitation: None,
            pending: false,
        }
    }

    #[test]
    fn flat_and_thread_rows_open_their_representative_message() {
        let flat = SnapshotRow::Flat {
            row: FlatRow {
                avatar: crate::ui::model::blank_avatar(),
                account: "account-a".to_owned(),
                key: "message-a".to_owned(),
                subject: "Flat subject".to_owned(),
                from: "flat@example.test".to_owned(),
                date: "2026-07-20".to_owned(),
                unread: true,
                flagged: false,
                has_attachment: false,
                preview: String::new(),
            },
        };
        let thread = SnapshotRow::Thread {
            row: ThreadRow {
                avatar: crate::ui::model::blank_avatar(),
                account: "account-b".to_owned(),
                thread_id: "thread-b".to_owned(),
                latest_key: "message-b".to_owned(),
                subject: "Thread subject".to_owned(),
                latest_from: "thread@example.test".to_owned(),
                latest_date: "2026-07-19".to_owned(),
                message_count: 2,
                unread_count: 0,
                has_attachment: true,
                preview: String::new(),
                messages: Vec::new(),
            },
        };

        assert_eq!(OpenedMessage::from_row(&flat).key, "message-a");
        assert_eq!(OpenedMessage::from_row(&thread).key, "message-b");
        assert_eq!(OpenedMessage::from_row(&thread).from, "thread@example.test");
    }

    #[test]
    fn opening_a_new_message_resets_remote_content_and_rejects_stale_bodies() {
        let mut state = ReadingState::new(empty_reading("old"));
        state.load_remote_images = true;

        state.open(OpenedMessage {
            account: "account".to_owned(),
            key: "new".to_owned(),
            subject: "Subject".to_owned(),
            from: "sender@example.test".to_owned(),
            date: "2026-07-20".to_owned(),
            avatar: AvatarData::from(&blank_avatar()),
        });

        assert!(!state.load_remote_images);
        assert!(!state.matches_opened());
        state.snapshot = empty_reading("new");
        assert!(state.matches_opened());
    }

    #[test]
    fn a_matching_key_is_not_yet_an_arrived_body() {
        let mut state = ReadingState::new(empty_reading("new"));
        state.open(OpenedMessage {
            account: "account".to_owned(),
            key: "new".to_owned(),
            subject: "Subject".to_owned(),
            from: "sender@example.test".to_owned(),
            date: "2026-07-20".to_owned(),
            avatar: AvatarData::from(&blank_avatar()),
        });

        // The core publishes the reading surface on selection, before either body part is filled
        // in. Anything that seeds a composer from the body must wait for this, not for the key.
        assert!(state.matches_opened());
        assert!(!state.body_arrived());

        state.snapshot.html = Some(String::new());
        assert!(!state.body_arrived(), "an empty body has not arrived");

        state.snapshot.plain = Some("Body".to_owned());
        assert!(state.body_arrived());
    }

    #[test]
    fn removing_an_open_message_advances_down_then_up_and_matches_the_account() {
        let stops = vec![
            opened("account-a", "same-key"),
            opened("account-b", "same-key"),
            opened("account-a", "last"),
        ];

        assert_eq!(
            message_after_removing(&stops[0], &stops),
            Some(stops[1].clone()),
            "the next row down wins"
        );
        assert_eq!(
            message_after_removing(&stops[2], &stops),
            Some(stops[1].clone()),
            "the row above wins at the end"
        );
        assert_eq!(
            message_after_removing(&opened("missing", "same-key"), &stops),
            None,
            "a provider key from another account is not the open row"
        );
        assert_eq!(message_after_removing(&stops[0], &stops[..1]), None);
    }
}
