//! `From` conversions between the FFI mirror types (defined in [`crate`]) and the pure
//! `mailcal-app` / `mailcal-viewmodel` types: intents, and the calendar, reading, sync-progress
//! and connectivity surfaces. Two sibling modules carry the rest: the **mailbox list** and its
//! rows in `convert_mailbox`, and the **settings surface** (timezone, grouping, quote style,
//! swipe actions, per-account sync rows) in `convert_settings`. Split out of `lib.rs` to keep
//! each file under the 500-line limit; no FFI macros live here, so the generated bindings are
//! unaffected. The observer adapters ([`ObserverBridge`], [`DebouncedObserver`]) live in
//! [`crate::observer`].

use engine_api::LocalDateTime;
use engine_provider::{
    ConnectionInfo as AppConnectionInfo, HttpVersion as AppHttpVersion, TlsVersion as AppTlsVersion,
};
use mailcal_account::{EventDrag, EventEdge as AppEventEdge, EventEdit};
use mailcal_app::{
    CalendarWriteStatus as AppCalendarWriteStatus, ContactWriteStatus as AppContactWriteStatus,
    EventRef, FolderRef, Intent as AppIntent, InvitationResponse as AppInvitationResponse,
    MessageRef, RecipientSuggestion as AppRecipientSuggestion, SearchScope as AppSearchScope,
    SendStatus as AppSendStatus, Surface as AppSurface, ThreadRef,
};
use mailcal_viewmodel::{
    AccountSyncProgress as AppAccountSyncProgress, AttachmentRow as AppAttachmentRow,
    CalendarSnapshot as AppCalendarSnapshot, ConnectivitySnapshot as AppConnectivity,
    EventRow as AppEventRow, ReadingSnapshot as AppReading,
    SyncProgressSnapshot as AppSyncProgress,
};

use crate::{
    AccountSyncProgress, AttachmentRow, CalendarSnapshot, CalendarWriteStatus, ConnectionInfo,
    ConnectivitySnapshot, ContactWriteStatus, EventEdge, EventRow, HttpVersion, Intent,
    InvitationResponse, ReadingSnapshot, RecipientSuggestion, SearchScope, SendStatus, Surface,
    SyncProgressSnapshot, TlsVersion,
};

impl From<AppSurface> for Surface {
    fn from(surface: AppSurface) -> Self {
        match surface {
            AppSurface::MailboxList => Self::MailboxList,
            AppSurface::Calendar => Self::Calendar,
            AppSurface::Settings => Self::Settings,
            AppSurface::Reading => Self::Reading,
            AppSurface::Sending => Self::Sending,
            AppSurface::SyncProgress => Self::SyncProgress,
            AppSurface::Connectivity => Self::Connectivity,
            AppSurface::CalendarStatus => Self::CalendarStatus,
            AppSurface::Contacts => Self::Contacts,
            AppSurface::ContactsStatus => Self::ContactsStatus,
            AppSurface::InvitationReply => Self::InvitationReply,
            AppSurface::UnfiledCopy => Self::UnfiledCopy,
        }
    }
}

impl From<SearchScope> for AppSearchScope {
    fn from(scope: SearchScope) -> Self {
        match scope {
            SearchScope::AllFolders => Self::AllFolders,
            SearchScope::CurrentFolder => Self::CurrentFolder,
        }
    }
}

impl From<AppSendStatus> for SendStatus {
    fn from(status: AppSendStatus) -> Self {
        match status {
            AppSendStatus::Idle => Self::Idle,
            AppSendStatus::Sending => Self::Sending,
            AppSendStatus::Sent => Self::Sent,
            AppSendStatus::SentNotFiled => Self::SentNotFiled,
            AppSendStatus::Failed => Self::Failed,
        }
    }
}

impl From<AppContactWriteStatus> for ContactWriteStatus {
    fn from(status: AppContactWriteStatus) -> Self {
        match status {
            AppContactWriteStatus::Idle => Self::Idle,
            AppContactWriteStatus::Saving => Self::Saving,
            AppContactWriteStatus::Saved => Self::Saved,
            AppContactWriteStatus::Failed => Self::Failed,
            AppContactWriteStatus::Invalid => Self::Invalid,
        }
    }
}

impl From<AppCalendarWriteStatus> for CalendarWriteStatus {
    fn from(status: AppCalendarWriteStatus) -> Self {
        match status {
            AppCalendarWriteStatus::Idle => Self::Idle,
            AppCalendarWriteStatus::Saving => Self::Saving,
            AppCalendarWriteStatus::Saved => Self::Saved,
            AppCalendarWriteStatus::Failed => Self::Failed,
        }
    }
}

impl TryFrom<Intent> for AppIntent {
    /// A key-routed intent's `account` id or provider `key` was malformed, so a typed
    /// [`MessageRef`]/[`EventRef`] couldn't be built. In practice impossible: the host
    /// passes back a row's own account and key: so the
    /// [`dispatch`](crate::MailcalApp::dispatch) caller drops such an intent rather than
    /// risk routing the action to the wrong account.
    type Error = String;

    fn try_from(intent: Intent) -> Result<Self, Self::Error> {
        // Bind account + key into one typed reference at this single boundary, so no
        // downstream code can pair a key with the wrong account (or carry one without the
        // other). A malformed pair is dropped rather than misrouted.
        let message = |account: String, key: String| {
            MessageRef::from_parts(&account, key)
                .ok_or_else(|| "invalid message reference".to_owned())
        };
        let event = |account: String, key: String| {
            EventRef::from_parts(&account, key).ok_or_else(|| "invalid event reference".to_owned())
        };
        let folder = |account: String, key: String| {
            FolderRef::from_parts(&account, key)
                .ok_or_else(|| "invalid folder reference".to_owned())
        };
        let thread = |account: String, thread_id: String| {
            ThreadRef::from_parts(&account, thread_id)
                .ok_or_else(|| "invalid thread reference".to_owned())
        };
        // A wall-clock edit field: absent or empty leaves the property unchanged; a value is
        // parsed as a `LocalDateTime` in the event's own zone. A malformed value drops the
        // whole intent rather than silently editing the wrong time.
        let parse_local = |value: Option<String>| -> Result<Option<LocalDateTime>, String> {
            match value.filter(|value| !value.is_empty()) {
                Some(value) => value
                    .parse::<LocalDateTime>()
                    .map(Some)
                    .map_err(|err| format!("invalid wall-clock {value:?}: {err}")),
                None => Ok(None),
            }
        };
        Ok(match intent {
            Intent::RefreshMail => Self::RefreshMail,
            Intent::SetViewMode { mode } => Self::SetViewMode(mode.into()),
            Intent::Search { query } => Self::Search(query),
            Intent::SetSearchScope { scope } => Self::SetSearchScope(scope.into()),
            Intent::SelectAccount { account } => Self::SelectAccount(account),
            Intent::SetAccountExpanded { account, expanded } => {
                Self::SetAccountExpanded { account, expanded }
            }
            Intent::SelectFolder { account, key } => Self::SelectFolder {
                folder: folder(account, key)?,
            },
            Intent::ShowMore => Self::ShowMore,
            Intent::OpenMessage { account, key } => Self::OpenMessage {
                message: message(account, key)?,
            },
            Intent::SubmitMail { to, subject, body } => Self::SubmitMail { to, subject, body },
            Intent::RefreshCalendar => Self::RefreshCalendar,
            Intent::RefreshContacts => Self::RefreshContacts,
            Intent::SearchContacts { query } => Self::SearchContacts { query },
            Intent::CreateContact {
                account,
                address_book,
                edit,
            } => Self::CreateContact {
                account,
                address_book,
                edit: edit.into(),
            },
            Intent::UpdateContact {
                person,
                account,
                card,
                edit,
            } => Self::UpdateContact {
                person,
                account,
                card,
                edit: edit.into(),
            },
            Intent::MarkRead { account, key, read } => Self::MarkRead {
                message: message(account, key)?,
                read,
            },
            Intent::SetFlagged {
                account,
                key,
                flagged,
            } => Self::SetFlagged {
                message: message(account, key)?,
                flagged,
            },
            Intent::Delete { account, key } => Self::Delete {
                message: message(account, key)?,
            },
            Intent::PermanentlyDelete { account, key } => Self::PermanentlyDelete {
                message: message(account, key)?,
            },
            Intent::Archive { account, key } => Self::Archive {
                message: message(account, key)?,
            },
            Intent::ArchiveThread { account, thread_id } => Self::ArchiveThread {
                thread: thread(account, thread_id)?,
            },
            Intent::MarkAsSpam { account, key } => Self::MarkAsSpam {
                message: message(account, key)?,
            },
            Intent::MarkAsNotSpam { account, key } => Self::MarkAsNotSpam {
                message: message(account, key)?,
            },
            Intent::CreateEvent {
                title,
                start,
                end,
                account,
                calendar,
                all_day,
                timezone,
                notes,
                location,
                recurrence,
            } => Self::CreateEvent {
                title,
                start,
                end,
                account,
                calendar,
                all_day,
                timezone,
                notes,
                location,
                recurrence: recurrence.map(Into::into),
            },
            Intent::UpdateEvent {
                account,
                key,
                title,
                start,
                end,
                notes,
                location,
                occurrence,
                recurrence,
                times_from_occurrence,
            } => Self::UpdateEvent {
                event: event(account, key)?,
                edit: EventEdit {
                    title: title.filter(|title| !title.is_empty()),
                    start: parse_local(start)?,
                    end: parse_local(end)?,
                    notes,
                    location,
                    recurrence: recurrence.map(Into::into),
                    occurrence: parse_local(occurrence)?,
                    times_from_occurrence: parse_local(times_from_occurrence)?,
                },
            },
            Intent::MoveEvent {
                account,
                key,
                edge,
                days,
                minutes,
                occurrence,
            } => Self::MoveEvent {
                event: event(account, key)?,
                drag: EventDrag {
                    edge: match edge {
                        EventEdge::Whole => AppEventEdge::Whole,
                        EventEdge::Start => AppEventEdge::Start,
                        EventEdge::End => AppEventEdge::End,
                    },
                    days,
                    minutes,
                    // The same parse as the editor's, on the same token: a malformed value
                    // drops the whole intent rather than quietly moving the entire series when
                    // the user asked for one Tuesday.
                    occurrence: parse_local(occurrence)?,
                },
            },
            Intent::RespondToInvitation {
                account,
                key,
                response,
                comment,
                notify_organizer,
                reply_subject,
            } => Self::RespondToInvitation {
                message: message(account, key)?,
                response: match response {
                    InvitationResponse::Accept => AppInvitationResponse::Accept,
                    InvitationResponse::Tentative => AppInvitationResponse::Tentative,
                    InvitationResponse::Decline => AppInvitationResponse::Decline,
                },
                comment,
                notify_organizer,
                reply_subject,
            },
            Intent::RetryUnfiledCopy => Self::RetryUnfiledCopy,
            Intent::DismissUnfiledCopy => Self::DismissUnfiledCopy,
            Intent::AnswerReplyPrompt {
                send,
                remember,
                reply_subject,
            } => Self::AnswerReplyPrompt {
                send,
                remember,
                reply_subject,
            },
            Intent::DeleteEvent {
                account,
                key,
                occurrence,
            } => Self::DeleteEvent {
                event: event(account, key)?,
                // The same parse as the editor's, on the same token: a malformed value drops
                // the whole intent rather than deleting the entire series when the user asked
                // for one Tuesday.
                occurrence: parse_local(occurrence)?,
            },
            Intent::ReportNetworkReachable { reachable } => Self::ReportNetworkReachable(reachable),
            Intent::ReportDeviceTimeZone { id } => Self::ReportDeviceTimeZone(id),
            Intent::SetTimeZone { id } => Self::SetTimeZone(id),
            Intent::AcceptTimeZoneChange => Self::AcceptTimeZoneChange,
            Intent::DismissTimeZoneChange => Self::DismissTimeZoneChange,
        })
    }
}

impl From<AppEventRow> for EventRow {
    fn from(row: AppEventRow) -> Self {
        Self {
            account: row.account,
            key: row.key,
            title: row.title,
            start: row.start,
            can_write: row.can_write,
            participation: row.participation.into(),
        }
    }
}

impl From<AppCalendarSnapshot> for CalendarSnapshot {
    fn from(snapshot: AppCalendarSnapshot) -> Self {
        Self {
            events: snapshot.events.into_iter().map(EventRow::from).collect(),
            timezone: snapshot.timezone,
        }
    }
}

impl From<AppRecipientSuggestion> for RecipientSuggestion {
    fn from(suggestion: AppRecipientSuggestion) -> Self {
        Self {
            to: suggestion.to,
            cc: suggestion.cc,
        }
    }
}

impl From<AppReading> for ReadingSnapshot {
    fn from(snapshot: AppReading) -> Self {
        Self {
            key: snapshot.key,
            from: snapshot.from,
            avatar: snapshot.avatar.into(),
            to: snapshot.to,
            cc: snapshot.cc,
            bcc: snapshot.bcc,
            html: snapshot.html,
            plain: snapshot.plain,
            has_remote_images: snapshot.has_remote_images,
            load_error: snapshot.load_error,
            attachments: snapshot
                .attachments
                .into_iter()
                .map(AttachmentRow::from)
                .collect(),
            invitation: snapshot.invitation.map(Into::into),
            pending: snapshot.pending,
        }
    }
}

impl From<AppAttachmentRow> for AttachmentRow {
    fn from(row: AppAttachmentRow) -> Self {
        Self {
            id: row.id,
            file_name: row.file_name,
            media_type: row.media_type,
            size: row.size,
        }
    }
}

impl From<AppSyncProgress> for SyncProgressSnapshot {
    fn from(snapshot: AppSyncProgress) -> Self {
        Self {
            active: snapshot.active,
            fetched: snapshot.fetched,
            total: snapshot.total,
            accounts: snapshot
                .accounts
                .into_iter()
                .map(AccountSyncProgress::from)
                .collect(),
        }
    }
}

impl From<AppAccountSyncProgress> for AccountSyncProgress {
    fn from(account: AppAccountSyncProgress) -> Self {
        Self {
            account_id: account.account_id,
            folders_done: account.folders_done,
            folders_total: account.folders_total,
            warming_bodies: account.warming_bodies,
            bodies_done: account.bodies_done,
        }
    }
}

impl From<AppConnectivity> for ConnectivitySnapshot {
    fn from(snapshot: AppConnectivity) -> Self {
        Self {
            offline: snapshot.offline,
            unreachable_accounts: snapshot.unreachable_accounts,
            calendar_reauth_accounts: snapshot.calendar_reauth_accounts,
            mail_reauth_accounts: snapshot.mail_reauth_accounts,
            signin_expired_accounts: snapshot.signin_expired_accounts,
        }
    }
}

impl From<AppTlsVersion> for TlsVersion {
    fn from(version: AppTlsVersion) -> Self {
        match version {
            AppTlsVersion::Tls1_2 => Self::Tls1_2,
            AppTlsVersion::Tls1_3 => Self::Tls1_3,
        }
    }
}

impl From<AppHttpVersion> for HttpVersion {
    fn from(version: AppHttpVersion) -> Self {
        match version {
            AppHttpVersion::Http1_1 => Self::Http1_1,
            AppHttpVersion::Http2 => Self::Http2,
        }
    }
}

impl From<AppConnectionInfo> for ConnectionInfo {
    fn from(info: AppConnectionInfo) -> Self {
        Self {
            tls_version: info.tls_version.map(TlsVersion::from),
            http_version: info.http_version.map(HttpVersion::from),
        }
    }
}
