//! `From` conversions for the **mailbox list**: the snapshot a host renders, its flat and
//! threaded rows, and the accounts and folders of the pane beside them. Split out of `convert`
//! to keep each file under the 500-line limit; no FFI macros live here, so the generated
//! bindings are unaffected.

use mailcal_viewmodel::{
    AccountFolderRow as AppAccountFolderRow, AccountRow as AppAccountRow, FlatRow as AppFlatRow,
    FolderRole as AppFolderRole, FolderRow as AppFolderRow, MailboxListSnapshot as AppSnapshot,
    SearchHorizon as AppSearchHorizon, SnapshotRow as AppSnapshotRow,
    ThreadMessage as AppThreadMessage, ThreadRow as AppThreadRow,
};

use crate::{
    AccountFolderRow, AccountRow, FlatRow, FolderRole, FolderRow, MailboxListSnapshot,
    SearchHorizon, SnapshotRow, ThreadMessage, ThreadRow,
};

impl From<AppFolderRole> for FolderRole {
    fn from(role: AppFolderRole) -> Self {
        match role {
            AppFolderRole::Inbox => Self::Inbox,
            AppFolderRole::Drafts => Self::Drafts,
            AppFolderRole::Sent => Self::Sent,
            AppFolderRole::Archive => Self::Archive,
            AppFolderRole::Junk => Self::Junk,
            AppFolderRole::Trash => Self::Trash,
            AppFolderRole::Other => Self::Other,
        }
    }
}

impl From<AppFolderRow> for FolderRow {
    fn from(row: AppFolderRow) -> Self {
        Self {
            key: row.key,
            name: row.name,
            role: row.role.map(FolderRole::from),
            unread: row.unread,
        }
    }
}

impl From<AppAccountRow> for AccountRow {
    fn from(row: AppAccountRow) -> Self {
        Self {
            id: row.id,
            email: row.email,
            expanded: row.expanded,
        }
    }
}

impl From<AppFlatRow> for FlatRow {
    fn from(row: AppFlatRow) -> Self {
        Self {
            account: row.account,
            key: row.key,
            subject: row.subject,
            from: row.from,
            avatar: row.avatar.into(),
            date: row.date,
            unread: row.unread,
            flagged: row.flagged,
            has_attachment: row.has_attachment,
            preview: row.preview,
        }
    }
}

impl From<AppThreadRow> for ThreadRow {
    fn from(row: AppThreadRow) -> Self {
        Self {
            account: row.account,
            thread_id: row.thread_id,
            latest_key: row.latest_key,
            subject: row.subject,
            latest_from: row.latest_from,
            avatar: row.avatar.into(),
            latest_date: row.latest_date,
            message_count: row.message_count,
            unread_count: row.unread_count,
            has_attachment: row.has_attachment,
            preview: row.preview,
            messages: row.messages.into_iter().map(ThreadMessage::from).collect(),
        }
    }
}

impl From<AppThreadMessage> for ThreadMessage {
    fn from(message: AppThreadMessage) -> Self {
        Self {
            account: message.account,
            key: message.key,
            from: message.from,
            avatar: message.avatar.into(),
            date: message.date,
            preview: message.preview,
            unread: message.unread,
            outgoing: message.outgoing,
            has_attachment: message.has_attachment,
        }
    }
}

impl From<AppSnapshotRow> for SnapshotRow {
    fn from(row: AppSnapshotRow) -> Self {
        match row {
            AppSnapshotRow::Flat(flat) => Self::Flat { row: flat.into() },
            AppSnapshotRow::Thread(thread) => Self::Thread { row: thread.into() },
        }
    }
}

impl From<AppAccountFolderRow> for AccountFolderRow {
    fn from(row: AppAccountFolderRow) -> Self {
        Self {
            account_id: row.account_id,
            folders: row.folders.into_iter().map(FolderRow::from).collect(),
        }
    }
}

impl From<AppSnapshot> for MailboxListSnapshot {
    fn from(snapshot: AppSnapshot) -> Self {
        Self {
            accounts: snapshot
                .accounts
                .into_iter()
                .map(AccountRow::from)
                .collect(),
            selected_account: snapshot.selected_account,
            folders: snapshot.folders.into_iter().map(FolderRow::from).collect(),
            account_folders: snapshot
                .account_folders
                .into_iter()
                .map(AccountFolderRow::from)
                .collect(),
            unified_unread: snapshot.unified_unread,
            selected: snapshot.selected,
            mode: snapshot.mode.into(),
            rows: snapshot.rows.into_iter().map(SnapshotRow::from).collect(),
            total: snapshot.total as u64,
            search_horizon: snapshot.search_horizon.map(SearchHorizon::from),
        }
    }
}

impl From<AppSearchHorizon> for SearchHorizon {
    fn from(horizon: AppSearchHorizon) -> Self {
        match horizon {
            AppSearchHorizon::AllTime => Self::AllTime,
            AppSearchHorizon::Months(months) => Self::Months {
                months: u32::from(months),
            },
        }
    }
}
