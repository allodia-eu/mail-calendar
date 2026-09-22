//! `From` conversions between the FFI mirror types (defined in [`crate`]) and the pure
//! `mailcal-app` / `mailcal-viewmodel` types: intents, and the calendar and reading surfaces.
//! Three sibling modules carry the rest: the **mailbox list** and its rows in `convert_mailbox`,
//! the **settings surface** (timezone, grouping, quote style, swipe actions, per-account sync
//! rows) in `convert_settings`, and an account's **standing** (sync progress, connectivity) in
//! `convert_status`. Split out of `lib.rs` to keep each file under the 500-line limit; no FFI
//! macros live here, so the generated bindings are unaffected. The observer adapters
//! ([`ObserverBridge`], [`DebouncedObserver`]) live in [`crate::observer`].

use engine_api::LocalDateTime;
use mailcal_account::{EventDrag, EventEdge as AppEventEdge, EventEdit};
use mailcal_app::{
    BulkAction as AppBulkAction, CalendarWriteStatus as AppCalendarWriteStatus,
    ContactWriteStatus as AppContactWriteStatus, ContactsIntent as AppContactsIntent, EventRef,
    FolderRef, Intent as AppIntent, InvitationResponse as AppInvitationResponse, MessageRef,
    OutboxIntent as AppOutboxIntent, QueuedRef, ReaderId as AppReaderId,
    RecipientSuggestion as AppRecipientSuggestion, RowRef, SearchScope as AppSearchScope,
    SendStatus as AppSendStatus, Surface as AppSurface, ThreadRef,
};
use mailcal_viewmodel::{CalendarSnapshot as AppCalendarSnapshot, EventRow as AppEventRow};

use crate::{
    BulkAction, CalendarSnapshot, CalendarWriteStatus, ContactWriteStatus, EventEdge, EventRow,
    Intent, InvitationResponse, OutboxIntent, RecipientSuggestion, SearchScope, SelectedRow,
    SendStatus, Surface,
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
            AppSurface::ComposeRequest => Self::ComposeRequest,
            AppSurface::DraftStatus => Self::DraftStatus,
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

impl From<BulkAction> for AppBulkAction {
    fn from(action: BulkAction) -> Self {
        match action {
            BulkAction::MarkRead => Self::MarkRead,
            BulkAction::MarkUnread => Self::MarkUnread,
            BulkAction::Flag => Self::Flag,
            BulkAction::Unflag => Self::Unflag,
            BulkAction::Archive => Self::Archive,
            BulkAction::Delete => Self::Delete,
            BulkAction::PermanentlyDelete => Self::PermanentlyDelete,
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
            AppSendStatus::Queued => Self::Queued,
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
            Intent::SetUnifiedExpanded { expanded } => Self::SetUnifiedExpanded { expanded },
            Intent::SetFolderExpanded {
                account,
                key,
                expanded,
            } => Self::SetFolderExpanded {
                folder: folder(account, key)?,
                expanded,
            },
            Intent::SetAccountSenderName { account, name } => {
                Self::SetAccountSenderName { account, name }
            }
            Intent::SelectFolder { account, key } => Self::SelectFolder {
                folder: folder(account, key)?,
            },
            Intent::ShowMore => Self::ShowMore,
            Intent::OpenMessage { account, key } => Self::OpenMessage {
                reader: AppReaderId::Pane,
                message: message(account, key)?,
            },
            Intent::OpenMessageInWindow {
                window,
                account,
                key,
            } => Self::OpenMessage {
                reader: AppReaderId::Window(window),
                message: message(account, key)?,
            },
            Intent::SubmitMail { to, subject, body } => Self::SubmitMail { to, subject, body },
            Intent::RefreshCalendar => Self::RefreshCalendar,
            Intent::Outbox { intent } => Self::Outbox(outbox_intent(intent)?),
            Intent::DismissComposeRequest => Self::DismissComposeRequest,
            Intent::RefreshContacts => Self::Contacts(AppContactsIntent::RefreshContacts),
            Intent::SearchContacts { query } => {
                Self::Contacts(AppContactsIntent::SearchContacts { query })
            }
            Intent::CreateContact {
                account,
                address_book,
                edit,
            } => Self::Contacts(AppContactsIntent::CreateContact {
                account,
                address_book,
                edit: edit.into(),
            }),
            Intent::UpdateContact {
                person,
                account,
                card,
                edit,
            } => Self::Contacts(AppContactsIntent::UpdateContact {
                person,
                account,
                card,
                edit: edit.into(),
            }),
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
            Intent::ActOnSelection { rows, action } => Self::ActOnSelection {
                // A malformed row drops the whole intent rather than acting on the rest: a
                // selection is one command, and archiving four of five rows silently is worse
                // than archiving none.
                rows: rows
                    .into_iter()
                    .map(|row| match row {
                        SelectedRow::Message { account, key } => {
                            message(account, key).map(RowRef::Message)
                        }
                        SelectedRow::Thread {
                            account,
                            thread_id: id,
                        } => thread(account, id).map(RowRef::Thread),
                    })
                    .collect::<Result<Vec<_>, _>>()?,
                action: action.into(),
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

/// Binds each Outbox action's account id and op id into one [`QueuedRef`] at the boundary,
/// so no action can carry an op id without (or mismatched against) the account whose queue
/// holds it.
fn outbox_intent(intent: OutboxIntent) -> Result<AppOutboxIntent, String> {
    let queued = |account: &str, op: u64| {
        QueuedRef::from_parts(account, op)
            .ok_or_else(|| format!("invalid queued-send reference for account {account:?}"))
    };
    Ok(match intent {
        OutboxIntent::Show => AppOutboxIntent::Show,
        OutboxIntent::Cancel { account, op } => AppOutboxIntent::Cancel(queued(&account, op)?),
        OutboxIntent::SendNow { account, op } => AppOutboxIntent::SendNow(queued(&account, op)?),
        OutboxIntent::Edit { account, op } => AppOutboxIntent::Edit(queued(&account, op)?),
    })
}
