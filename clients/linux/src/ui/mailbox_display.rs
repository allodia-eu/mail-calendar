//! Pure rendering keys for mailbox rows.

use mailcal_bindings::{FlatRow, MailboxListSnapshot, SnapshotRow, ThreadMessage, ThreadRow};

use super::{avatar::AvatarData, mail_actions, mailbox_reconcile::Row};
use crate::l10n;

/// Only the snapshot fields the message rows draw.
///
/// Conversation expansion is absent because the widget animates its own disclosure. The display
/// zone is present so changing it rebuilds otherwise unchanged timestamp labels.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct MailboxRendering {
    rows: Vec<DisplayRow>,
    in_junk_folder: bool,
}

impl MailboxRendering {
    pub(super) fn new(snapshot: &MailboxListSnapshot, zone: &str) -> Self {
        Self {
            rows: snapshot
                .rows
                .iter()
                .map(|row| display_row(row, zone))
                .collect(),
            in_junk_folder: mail_actions::in_junk_folder(snapshot),
        }
    }
}

/// Everything a message row draws and the zone its timestamp uses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct DisplayRow {
    pub(super) key: String,
    pub(super) avatar: AvatarData,
    pub(super) title: String,
    pub(super) subtitle: String,
    pub(super) date: String,
    pub(super) unread: bool,
    pub(super) flagged: bool,
    pub(super) has_attachment: bool,
    pub(super) count: u32,
    pub(super) messages: Vec<DisplayMessage>,
    pub(super) zone: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct DisplayMessage {
    pub(super) key: String,
    pub(super) avatar: AvatarData,
    pub(super) from: String,
    pub(super) preview: String,
    pub(super) date: String,
    pub(super) unread: bool,
    pub(super) outgoing: bool,
    pub(super) has_attachment: bool,
}

impl Row for DisplayRow {
    fn key(&self) -> &str {
        &self.key
    }
}

pub(super) fn display_row(snapshot: &SnapshotRow, zone: &str) -> DisplayRow {
    match snapshot {
        SnapshotRow::Flat { row } => flat_display(row, zone),
        SnapshotRow::Thread { row } => thread_display(row, zone),
    }
}

pub(super) fn flat_display(row: &FlatRow, zone: &str) -> DisplayRow {
    DisplayRow {
        key: format!("m/{}/{}", row.account, row.key),
        avatar: AvatarData::from(&row.avatar),
        title: subject_or_fallback(&row.subject),
        subtitle: one_line(&row.from),
        date: row.date.clone(),
        unread: row.unread,
        flagged: row.flagged,
        has_attachment: row.has_attachment,
        count: 1,
        messages: Vec::new(),
        zone: zone.to_owned(),
    }
}

pub(super) fn thread_display(row: &ThreadRow, zone: &str) -> DisplayRow {
    DisplayRow {
        key: format!("t/{}/{}", row.account, row.thread_id),
        avatar: AvatarData::from(&row.avatar),
        title: subject_or_fallback(&row.subject),
        subtitle: one_line(&row.latest_from),
        date: row.latest_date.clone(),
        unread: row.unread_count > 0,
        flagged: false,
        has_attachment: row.has_attachment,
        count: row.message_count,
        messages: row.messages.iter().map(message_display).collect(),
        zone: zone.to_owned(),
    }
}

pub(super) fn message_display(message: &ThreadMessage) -> DisplayMessage {
    DisplayMessage {
        key: message.key.clone(),
        avatar: AvatarData::from(&message.avatar),
        from: one_line(&message.from),
        preview: one_line(&message.preview),
        date: message.date.clone(),
        unread: message.unread,
        outgoing: message.outgoing,
        has_attachment: message.has_attachment,
    }
}

fn subject_or_fallback(subject: &str) -> String {
    if subject.trim().is_empty() {
        l10n::mail_no_subject().to_owned()
    } else {
        one_line(subject)
    }
}

fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}
