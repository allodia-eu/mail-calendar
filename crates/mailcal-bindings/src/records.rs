//! The FFI record/enum mirror types: the immutable snapshots and rows a host renders.
//!
//! Split out of `lib.rs` to keep it under the 500-line limit. These derive the UniFFI
//! scaffolding (so the generated Swift/Kotlin see them), and `convert.rs` maps them from
//! the pure `mailcal-viewmodel` types. `lib.rs` re-exports them so the FFI object and the
//! conversions reference them at the crate root.
//!
//! The Settings-surface records live in the `settings` submodule (this file was over the
//! 500-line limit); `lib.rs` re-exports both halves, so the split is invisible to a host.

pub(crate) mod settings;

use crate::records_avatar::Avatar;

/// An immutable snapshot of the display-timezone setting for a host to render.
#[derive(uniffi::Record)]
pub struct TimeZoneSnapshot {
    /// The active display zone's IANA id (what the agenda is shown in).
    pub active: String,
    /// A different zone the device reported, awaiting the user's choice to adopt or
    /// dismiss it; `None` when the device matches the active zone.
    pub pending_device: Option<String>,
}

/// An immutable reading-view snapshot for a host to render: the open message's body.
#[derive(uniffi::Record)]
pub struct ReadingSnapshot {
    /// The provider key of the message this body is for, so a host can match it to the
    /// row it opened and ignore a stale snapshot (empty before any message is opened).
    pub key: String,
    /// The sender as `Name <email>` (bare `email` if unnamed, empty if none) for the header.
    pub from: String,
    /// The sender's monogram, colour and photo. Decoration; hide it from assistive
    /// technology; the row already announces the sender.
    pub avatar: Avatar,
    /// The `To` recipients, formatted for display and comma-joined; empty when none.
    pub to: String,
    /// The `Cc` recipients, formatted and comma-joined; empty when none.
    pub cc: String,
    /// The `Bcc` recipients, formatted and comma-joined; empty when none. Present only on the
    /// sender's own Sent/Drafts copy, so the sender can see whom they Bcc'd.
    pub bcc: String,
    /// The **sanitised** HTML body, when the message has an HTML part (presentational CSS
    /// preserved; scripts/handlers/frames stripped). A host wraps it with
    /// `render_message_html` and renders it in a WebView with scripting off and navigation
    /// blocked.
    pub html: Option<String>,
    /// The plain-text body, when the message has one: the fallback when `html` is
    /// `None`.
    pub plain: Option<String>,
    /// Whether the HTML references a remote resource blocked by default: the signal to
    /// offer a "load remote images" confirmation, then re-render with
    /// `render_message_html(.., load_remote_images = true)`.
    pub has_remote_images: bool,
    /// Whether the body could not be fetched (a provider/network error), as distinct from a
    /// message that has no body. A host shows a "couldn't load; retry" affordance for this.
    pub load_error: bool,
    /// Downloadable attachments decoded from the message source.
    ///
    /// A meeting invitation's `text/calendar` payload is **not** here; it is an alternative
    /// body part, consumed into [`Self::invitation`]. A calendar file the sender explicitly
    /// attached (Gmail's duplicate `invite.ics`, a published `.ics`) still appears.
    pub attachments: Vec<AttachmentRow>,
    /// The meeting-invitation card, when this message carries an iTIP object warranting one;
    /// `None` for ordinary mail. Draw it **above** the body.
    pub invitation: Option<crate::records_invitation::InvitationCard>,
    /// The open for `key` is still running and has been long enough to be worth saying so:
    /// show the loading indicator, and **only** then.
    ///
    /// Never show one merely because no snapshot has arrived for the message being opened. A
    /// stored body comes back in milliseconds, so a spinner on every open appears and vanishes
    /// within an eyeblink and reads as flicker; until this is set, draw the body area empty and
    /// let the header the list row already gave you carry the pane. The core times the wait, so
    /// every platform draws the same conclusion.
    pub pending: bool,
}

/// One downloadable attachment in the reading view.
#[derive(uniffi::Record)]
pub struct AttachmentRow {
    /// Message-scoped attachment id; pass it back to save this part.
    pub id: u32,
    /// Suggested display/download file name.
    pub file_name: String,
    /// Media type, e.g. `application/pdf`.
    pub media_type: String,
    /// Decoded byte length.
    pub size: u64,
}

/// Suggested recipients for a reply or reply-all, for a host to pre-fill the composer's
/// editable `To`/`Cc` fields (which the user may then edit). Each is a comma-separated
/// address list; a plain reply has an empty `cc`, and both are empty when the original
/// can't be resolved. Pulled via [`crate::MailcalApp::reply_recipients`].
#[derive(uniffi::Record)]
pub struct RecipientSuggestion {
    /// The suggested `To` field: the original's `Reply-To`, else its `From`.
    pub to: String,
    /// The suggested `Cc` field: for a reply-all, the other original recipients; empty
    /// for a plain reply.
    pub cc: String,
}

/// An immutable snapshot of mail-sync progress for a host to render; two surfaces, for two
/// different questions.
///
/// The **bar** ([`active`](Self::active) and its counts) is for a download the user is *waiting
/// on*: adding an account, opening an unsynced folder, an explicit refetch. The **hint**
/// ([`accounts`](Self::accounts)) is for a pass nobody asked for: a poll tick, a push, a boot
/// catch-up, which never opens a bar and instead names the accounts currently pulling mail down.
/// Pulled via [`crate::MailcalApp::sync_progress`].
#[derive(uniffi::Record)]
pub struct SyncProgressSnapshot {
    /// Whether a **user-awaited** download is running: a host shows the bar while true and
    /// hides it once the pass completes. A background pass never sets it.
    pub active: bool,
    /// Messages committed (host-visible) so far across the folders of that download.
    pub fetched: u64,
    /// The summed expected total across those folders, or `None` until every in-flight
    /// folder has reported one (show an indeterminate bar).
    pub total: Option<u64>,
    /// The accounts whose **background** sync is downloading mail right now, in a stable order.
    /// Empty whenever nothing is arriving unasked, which is almost always: an account appears
    /// only once its pass has actually committed mail, so a poll that finds nothing stays
    /// silent. Never overlaps the bar.
    pub accounts: Vec<AccountSyncProgress>,
}

/// One account catching up in the background, as far as a status line needs it.
///
/// Two phases, in order, and an account is in exactly one of them. **Folders**: the sync pass;
/// render "3 of 12 folders". `folders_total` is what the pass set out to sync (one, for a push
/// notification that named its folder), so `folders_done` reaching it means the pass finished,
/// not that the mail ran out. **Bodies**; warming every synced message afterwards, the longer
/// half of a first sync: when `warming_bodies` is set, render `bodies_done` instead. There is no
/// body total, because the warm drains against "what is still missing" rather than a list.
#[derive(uniffi::Record)]
pub struct AccountSyncProgress {
    /// The account, to be named from the host's own account list (which already holds the
    /// address it shows everywhere else).
    pub account_id: String,
    /// Folders whose sync has finished this pass.
    pub folders_done: u32,
    /// Folders this pass is syncing in total.
    pub folders_total: u32,
    /// Whether the account is past its folders and warming message bodies. The folder counts
    /// are then final; render `bodies_done` instead.
    pub warming_bodies: bool,
    /// Message bodies warmed so far, with no total to divide by.
    pub bodies_done: u32,
}

/// How the mailbox list is grouped.
#[derive(uniffi::Enum)]
pub enum ViewMode {
    /// A flat list of messages, newest first.
    Flat,
    /// Conversations grouped by thread, newest activity first.
    Threaded,
}

/// The state of the most recent outgoing send (pulled after a `Surface::Sending` signal).
#[derive(uniffi::Enum)]
pub enum SendStatus {
    /// No send has started this session.
    Idle,
    /// A validated message is being submitted through the outbox.
    Sending,
    /// The most recent submission completed, and a copy is in the account's Sent folder.
    Sent,
    /// The message **was sent**, but its copy could not be filed in the account's Sent
    /// folder; it is not there and will not appear later. Show it as sent, with a warning:
    /// the recipients have the message, only the sender's own record of it is missing.
    /// Never as a failure, that invites a re-send of mail that already went out.
    SentNotFiled,
    /// The most recent submission failed: the message did **not** go out.
    Failed,
}

/// A message that was **sent** but whose copy is not in the account's Sent folder (pulled
/// after a `Surface::UnfiledCopy` signal).
///
/// A host shows this as a **modal**, not a banner. A Sent copy is how a person checks that a
/// message really left, so a missing one is worth interrupting for, and unlike the transient
/// send hint, this does not clear itself: it stands until the user files the copy
/// (`Intent::RetryUnfiledCopy`) or dismisses it (`Intent::DismissUnfiledCopy`).
///
/// The copy says what is true and no more: the message **was sent** and the recipients have
/// it. Only the sender's own record of it is missing. Never word this as a failed send; the
/// user's next move would be to send it again.
///
/// There is no id to pass back: the core holds the one message it is asking about and clears
/// it the moment it is filed, so a double-tap cannot file two copies.
#[derive(uniffi::Record)]
pub struct UnfiledCopy {
    /// The sent message's subject, so the modal names which message is missing its copy.
    pub subject: String,
    /// The failure class and protocol detail. **Not for the modal**: the copy a user reads
    /// is plain language. Carried for the diagnostics screen and support.
    pub detail: String,
    /// Whether a retry is already running, so a host disables its button instead of letting
    /// the user queue five of them.
    pub retrying: bool,
}

impl From<mailcal_app::UnfiledCopy> for UnfiledCopy {
    fn from(unfiled: mailcal_app::UnfiledCopy) -> Self {
        Self {
            subject: unfiled.subject,
            detail: unfiled.detail,
            retrying: unfiled.retrying,
        }
    }
}

/// The state of the most recent calendar write (create/edit/delete) pulled after a
/// `Surface::CalendarStatus` signal. A host shows a small in-calendar spinner while `Saving`
/// and, briefly, the terminal state.
///
/// **`Failed` does not mean the change was lost.** A write whose server call succeeded but
/// whose post-write reconcile could not be confirmed has already landed on the server, only
/// the local view is briefly stale, and the next sync heals it. `Failed` is the warning icon
/// ("we could not confirm this saved"), not "your change was rejected".
///
/// `Copy` and the value derives are for Linux, as on [`ResponseStatus`](crate::ResponseStatus): a
/// fieldless status is a value, not something to borrow.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum CalendarWriteStatus {
    /// No calendar write is settling.
    Idle,
    /// A create/edit/delete is in flight, or its reconcile is being retried.
    Saving,
    /// The most recent write settled and the local view holds the server's copy.
    Saved,
    /// The most recent write's server call failed, or its reconcile could not be confirmed.
    Failed,
}

/// One account in the sidebar switcher: its id, email (display label), and whether its
/// folder tree is open.
#[derive(uniffi::Record)]
pub struct AccountRow {
    /// The account's id (stable identity, used to select it).
    pub id: String,
    /// The account's email address (display label).
    pub email: String,
    /// Whether this account's folder tree is open in the sidebar; render the chevron from
    /// this and change it with `Intent::SetAccountExpanded`. Independent of selection and
    /// persisted across launches; a client that keeps its own copy will disagree with the
    /// other platforms and lose it on restart (`docs/folder-pane.md`).
    pub expanded: bool,
}

/// An immutable mailbox-list snapshot for a host to render.
#[derive(uniffi::Record)]
pub struct MailboxListSnapshot {
    /// The configured accounts, for the sidebar switcher.
    pub accounts: Vec<AccountRow>,
    /// The selected account's id, or `None` for the unified "all inboxes" view.
    pub selected_account: Option<String>,
    /// The selected account's folders (empty in the all-inboxes view).
    ///
    /// **Not what a folder pane renders**; use [`Self::account_folders`], which holds every
    /// account's tree in every view. Rendering this one is what made the pane empty itself on
    /// All Inboxes (`docs/folder-pane.md`).
    pub folders: Vec<FolderRow>,
    /// Every account's sorted folder list: the folder pane's source, populated in every view.
    pub account_folders: Vec<AccountFolderRow>,
    /// The badge on the unified "All Inboxes" row: every account's Inbox unread, summed.
    /// `0` shows no badge.
    pub unified_unread: u32,
    /// The selected folder's key, or `None` for the account's unified all-mail view.
    pub selected: Option<String>,
    /// The mode the rows are grouped in.
    pub mode: ViewMode,
    /// The rows, in display order (newest first), within the selected folder; at most the
    /// visible window (the first `limit` rows; the host grows it with `Intent::ShowMore`).
    pub rows: Vec<SnapshotRow>,
    /// How many rows exist in all for this view (flat messages, or threads in threaded mode).
    /// `rows.len() < total` means more can be shown: the host dispatches `Intent::ShowMore`
    /// as it scrolls toward the end.
    pub total: u64,
    /// How far back these results were searched, or `None` when the list is not a search.
    ///
    /// Render it beside the results, with a route to the sync-depth setting: an empty search
    /// that does not say how far it looked reads as "no such message" (`docs/search.md`).
    pub search_horizon: Option<SearchHorizon>,
}

/// How far back a search looked: the sync depth of the accounts it covered, narrowest first.
///
/// Search reads what is on the device and nothing else, so it finds only what sync depth kept.
#[derive(uniffi::Enum)]
pub enum SearchHorizon {
    /// Every message those accounts hold was searched; none of them bounds its depth.
    AllTime,
    /// Only mail from the last `months` months is on this device, so only that was searched.
    Months {
        /// The depth in months, as the sync-depth setting names it.
        months: u32,
    },
}

/// The special role a folder plays (RFC 6154 SPECIAL-USE / JMAP equivalent), exposed on
/// [`FolderRow`] so a client can badge or group well-known folders without name heuristics.
#[derive(uniffi::Enum)]
pub enum FolderRole {
    /// The primary inbox.
    Inbox,
    /// Drafts; messages in-progress.
    Drafts,
    /// Sent; copies of sent messages.
    Sent,
    /// Archive; long-term storage.
    Archive,
    /// Junk / Spam; server-side spam filter destination.
    Junk,
    /// Trash; recoverable deleted messages.
    Trash,
    /// Other role-bearing special folder (flagged, all, important, …).
    Other,
}

/// One sidebar folder: its key, display name, optional special role, and unread count.
#[derive(uniffi::Record)]
pub struct FolderRow {
    /// The mailbox's provider key (used to select it).
    pub key: String,
    /// The folder's display name.
    pub name: String,
    /// The folder's special role, or `None` for an ordinary custom folder.
    pub role: Option<FolderRole>,
    /// How many messages in the folder are unread, as the **server** counts them: so it
    /// covers mail older than the synced window. **Show no badge at `0`**: zero folds
    /// together "nothing unread" and "this provider reports no count", and both must
    /// render as nothing (`docs/folder-pane.md`).
    pub unread: u32,
}

/// One account's sorted folder list, for the navigation drawer that shows all accounts at once.
#[derive(uniffi::Record)]
pub struct AccountFolderRow {
    /// The account's stable id, matching [`AccountRow::id`].
    pub account_id: String,
    /// The account's sorted folder rows, ready for display.
    pub folders: Vec<FolderRow>,
}

/// One mailbox-list row: a single message (flat) or a conversation (threaded).
#[derive(Clone, uniffi::Enum)]
pub enum SnapshotRow {
    /// A single message.
    Flat {
        /// The message row.
        row: FlatRow,
    },
    /// A conversation summary.
    Thread {
        /// The thread row.
        row: ThreadRow,
    },
}

/// A single message row (flat mode).
#[derive(Clone, uniffi::Record)]
pub struct FlatRow {
    /// The id of the account this message belongs to (which inbox it came from, shown in
    /// the unified all-inboxes view).
    pub account: String,
    /// The message's provider key.
    pub key: String,
    /// The subject (empty if none).
    pub subject: String,
    /// The sender's display name, or their email when the header had no name (empty if none).
    pub from: String,
    /// The sender's monogram, colour and photo. Decoration; hide it from assistive
    /// technology; the row already announces the sender.
    pub avatar: Avatar,
    /// The received date, formatted (empty if unknown).
    pub date: String,
    /// Whether the message is unread.
    pub unread: bool,
    /// Whether the message is flagged.
    pub flagged: bool,
    /// Whether the provider says the message has a non-inline attachment.
    pub has_attachment: bool,
    /// A short body-preview snippet for a two-line list row (empty if none; Microsoft 365 and
    /// JMAP populate it; IMAP does not yet).
    pub preview: String,
}

/// A conversation row (threaded mode): a thread and its summary.
#[derive(Clone, uniffi::Record)]
pub struct ThreadRow {
    /// The id of the account this conversation belongs to.
    pub account: String,
    /// The thread's id.
    pub thread_id: String,
    /// The thread's representative message; its latest **in-scope** message (the newest one that
    /// touches the viewed folder): what a host opens from a thread row, and what the folder lists
    /// and orders the thread by. The owner's own Sent replies filed elsewhere ride in `messages`
    /// for reference but don't become the thread's summary.
    pub latest_key: String,
    /// The representative (latest in-scope) message's subject (empty if none).
    pub subject: String,
    /// The representative message's sender name, or their email when unnamed (empty if none).
    pub latest_from: String,
    /// The sender's monogram, colour and photo. Decoration; hide it from assistive
    /// technology; the row already announces the sender.
    pub avatar: Avatar,
    /// The representative (latest in-scope) message's date, formatted (empty if unknown).
    pub latest_date: String,
    /// How many messages the thread holds.
    pub message_count: u32,
    /// How many of them are unread.
    pub unread_count: u32,
    /// Whether any message in the conversation has a non-inline attachment.
    pub has_attachment: bool,
    /// A short preview snippet of the representative message's body (empty if none), for a
    /// two-line list row.
    pub preview: String,
    /// Every message on the thread, newest first: the whole conversation (received and the
    /// owner's own Sent replies), so a host can expand the row into a stacked reading view and
    /// open any message. Note the first entry is the newest message overall, which may be an
    /// out-of-folder Sent reply: not necessarily the one at `latest_key` (the latest in-scope
    /// message the summary reflects).
    pub messages: Vec<ThreadMessage>,
}

/// One message within an expanded conversation (threaded mode). A [`ThreadRow`] carries the
/// full, ordered set so a host renders the whole thread: the owner's own Sent replies filed
/// in another folder included, and can open any message for reading by its `key`.
#[derive(Clone, uniffi::Record)]
pub struct ThreadMessage {
    /// The id of the account this message belongs to.
    pub account: String,
    /// The message's provider key (stable identity; what a host opens for reading).
    pub key: String,
    /// The sender's display name, or their email when the header had no name (empty if none).
    pub from: String,
    /// The sender's monogram, colour and photo. Decoration; hide it from assistive
    /// technology; the row already announces the sender.
    pub avatar: Avatar,
    /// The message's date, formatted (empty if unknown).
    pub date: String,
    /// A short preview snippet for the collapsed card (empty if none).
    pub preview: String,
    /// Whether the message is unread.
    pub unread: bool,
    /// Whether the account owner sent this message (drives the "Sent" badge).
    pub outgoing: bool,
    /// Whether the message has a non-inline attachment.
    pub has_attachment: bool,
}
