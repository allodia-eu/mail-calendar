//! The FFI protocol types of the unidirectional loop: the [`Surface`]s a host observes,
//! the [`Intent`]s it dispatches, and the [`Observer`] callback it implements. Split out
//! of `lib.rs` to keep it under the 500-line limit; these derive the UniFFI scaffolding
//! (so the generated Swift/Kotlin see them) and `lib.rs` re-exports them at the crate root.

use crate::{EventEdge, RecurrenceChange, SimpleRecurrence, ViewMode};

/// A surface a host observes and pulls a snapshot for.
#[derive(uniffi::Enum)]
pub enum Surface {
    /// The mailbox/message list.
    MailboxList,
    /// The calendar agenda.
    Calendar,
    /// The settings surface: the active display timezone and any pending change.
    Settings,
    /// The reading view: the open message's fetched, sanitised body.
    Reading,
    /// The outgoing-send status; drives the composer's "sending…" → "sent" hint.
    Sending,
    /// Background mail-download progress; drives a "downloading Y of X" bar (pulled via
    /// `MailcalApp::sync_progress`).
    SyncProgress,
    /// Connectivity: the device-offline flag and per-account outage list (pulled via
    /// `MailcalApp::connectivity`); drives the offline banner and per-account warning badges.
    Connectivity,
    /// Calendar write status: the outcome of the most recent create/edit/delete (pulled via
    /// `MailcalApp::calendar_write_status`); drives a small in-calendar spinner and warning.
    CalendarStatus,
    /// The contacts list: the unified people snapshot (pulled via `MailcalApp::contact_list`).
    Contacts,
    /// A pending question about an invitation reply the calendar server could not deliver
    /// (pulled via `MailcalApp::reply_prompt`); drives the modal offering to email the
    /// organiser ourselves. `None` means there is nothing to ask.
    InvitationReply,
    /// A message that was sent but whose copy is not in the account's Sent folder (pulled via
    /// `MailcalApp::unfiled_copy`); drives the modal offering to file it. Unlike `Sending`
    /// this does **not** auto-clear; it stands until the user answers.
    UnfiledCopy,
}

/// Which folders an active search covers: the host's scope filter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum SearchScope {
    /// Every account, every folder, except each account's Trash. The default.
    AllFolders,
    /// Only what the mailbox list was showing when the search started: the selected folder,
    /// or (in the unified view) every account's Inbox.
    CurrentFolder,
}

/// The answer a user can give to an invitation.
///
/// Three values because three is all there are: "no answer yet" is the *absence* of one, and
/// delegating is a different act this release does not offer. A client shows exactly these
/// buttons.
#[derive(Debug, Clone, Copy, uniffi::Enum)]
pub enum InvitationResponse {
    /// Yes.
    Accept,
    /// Maybe.
    Tentative,
    /// No: the meeting then leaves the calendar (`docs/calendar.md`), reachable again from
    /// this card.
    Decline,
}

/// A host intent: the single inbound channel of the unidirectional loop.
#[derive(uniffi::Enum)]
pub enum Intent {
    /// Sync the account's mail (every folder), refresh the snapshot.
    RefreshMail,
    /// Switch the mailbox list between a flat and a threaded view.
    SetViewMode {
        /// The mode to switch to.
        mode: ViewMode,
    },
    /// Show full-text search results (newest first), or clear search (`None`) for the folder
    /// view. Clearing also resets the scope to `AllFolders`.
    Search {
        /// The query, or `None` to clear search.
        query: Option<String>,
    },
    /// Narrow (or re-widen) the active search. Independent of the query, so toggling the
    /// host's scope filter re-projects the results without retyping.
    SetSearchScope {
        /// The folders to cover.
        scope: SearchScope,
    },
    /// Show one account's folders (by account id), or the unified "all inboxes" view
    /// (`None`). Resets the selected folder.
    SelectAccount {
        /// The account id to focus, or `None` for all inboxes.
        account: Option<String>,
    },
    /// Open or shut one account's folder tree in the sidebar, and remember it across launches.
    ///
    /// **Not navigation**: it changes neither the selected account nor the selected folder, so
    /// any number of trees can be open at once and moving to All Inboxes, the calendar or
    /// contacts leaves them as they were. Render the chevron from `AccountRow::expanded` rather
    /// than keeping client-side state (`docs/folder-pane.md`).
    SetAccountExpanded {
        /// The account whose tree to open or shut.
        account: String,
        /// Whether the tree is open.
        expanded: bool,
    },
    /// Show one folder's mail: the folder key and the account that owns it **together**.
    ///
    /// A folder key is unique only within its account (every provider calls its inbox `inbox`),
    /// and the pane shows every account's tree at once: so pass the account the pane row sits
    /// under, not whichever one is selected. There is no folder-only form: without an account the
    /// key means nothing, and dispatching it alone used to leave the list exactly as it was
    /// (`docs/folder-pane.md`, rule 14). For an account's whole mailbox use
    /// [`Intent::SelectAccount`], which is the pane's other destination.
    SelectFolder {
        /// The id of the account whose tree the folder row sits under.
        account: String,
        /// The folder's key, from that account's `FolderRow::key`.
        key: String,
    },
    /// Grow the visible mailbox-list window by one page; dispatched as the host scrolls
    /// toward the end of the list (`MailboxListSnapshot::total` says when more remain). Any
    /// navigation (select account/folder, search, switch view mode) resets it to the first
    /// page.
    ShowMore,
    /// Open a message (by key) for reading: fetch + cache its source, extract and
    /// sanitise the body, then publish the [`Surface::Reading`] snapshot.
    OpenMessage {
        /// The id of the account that owns the message (the row's `account`).
        account: String,
        /// The message's provider key.
        key: String,
    },
    /// Send a plain-text message through the durable outbox, then refresh.
    SubmitMail {
        /// The recipient address.
        to: String,
        /// The subject line.
        subject: String,
        /// The plain-text body.
        body: String,
    },
    /// Sync the account's calendar(s) and refresh the agenda snapshot.
    RefreshCalendar,
    /// Sync every account's address books and refresh the contacts snapshot.
    RefreshContacts,
    /// Narrow the contacts list to people matching `query`; an empty one shows all.
    ///
    /// The matching runs in the core over name, email, phone, organisation and title, so every
    /// client narrows identically and a person beyond the loaded page is still findable.
    SearchContacts {
        /// The search text; empty clears the filter.
        query: String,
    },
    /// Mark a message read (`read = true`) or unread, by key.
    MarkRead {
        /// The id of the account that owns the message (the row's `account`).
        account: String,
        /// The message's provider key.
        key: String,
        /// Whether to mark it read.
        read: bool,
    },
    /// Flag (`flagged = true`) or unflag a message, by key.
    SetFlagged {
        /// The id of the account that owns the message (the row's `account`).
        account: String,
        /// The message's provider key.
        key: String,
        /// Whether to flag it.
        flagged: bool,
    },
    /// Delete a message (move it to Trash; recoverable), by key.
    Delete {
        /// The id of the account that owns the message (the row's `account`).
        account: String,
        /// The message's provider key.
        key: String,
    },
    /// **Permanently** delete a message (irreversible: not a Trash move), by key.
    PermanentlyDelete {
        /// The id of the account that owns the message (the row's `account`).
        account: String,
        /// The message's provider key.
        key: String,
    },
    /// Archive a message (move it to the account's Archive folder), by key.
    Archive {
        /// The id of the account that owns the message (the row's `account`).
        account: String,
        /// The message's provider key.
        key: String,
    },
    /// Archive a whole conversation: move every message on the thread to the account's Archive
    /// folder **except** those in the Sent folder (a sent copy never leaves Sent), by thread id.
    ArchiveThread {
        /// The id of the account that owns the conversation (the row's `account`).
        account: String,
        /// The thread's id (the row's `thread_id`).
        thread_id: String,
    },
    /// Mark a message as spam: move it to the account's Junk/Spam folder (resolved by the
    /// RFC 6154 `\Junk` role, with a conventional-name fallback), by key.
    MarkAsSpam {
        /// The id of the account that owns the message (the row's `account`).
        account: String,
        /// The message's provider key.
        key: String,
    },
    /// Mark a message as not spam: move it back to the account's Inbox, by key.
    /// Intended for use when the user is viewing the Junk/Spam folder.
    MarkAsNotSpam {
        /// The id of the account that owns the message (the row's `account`).
        account: String,
        /// The message's provider key.
        key: String,
    },
    /// Create a calendar event, then refresh the agenda.
    ///
    /// For a **timed** event (`all_day = false`) `start`/`end` are RFC 3339 UTC instants, for
    /// an **all-day** event they are `YYYY-MM-DD` dates and the end is **exclusive** (a one-day
    /// event on the 1st ends on the 2nd: the client converts its inclusive on-screen end).
    /// `account`/`calendar` are the picker's choice (the owning account id and `CalendarRow.id`);
    /// both `None` files it in the default writable account's first calendar.
    CreateEvent {
        /// The event title.
        title: String,
        /// The start: a **wall clock** (`2026-07-01T10:00:00`) when `timezone` is set, else a UTC
        /// instant; a date (`2026-07-01`) when all-day.
        start: String,
        /// The end; same terms as `start`; exclusive date for all-day.
        end: String,
        /// The chosen calendar's owning account id, or `None` for the default.
        account: Option<String>,
        /// The chosen calendar's row key (`CalendarRow.id`), or `None` for the first.
        calendar: Option<String>,
        /// Whether this is an all-day event.
        all_day: bool,
        /// The IANA zone a timed event is created in (the device's zone), so it reads back the
        /// same clock on edit. `None`/empty falls back to UTC (`start`/`end` then UTC instants).
        timezone: Option<String>,
        /// The description/notes, if any.
        notes: Option<String>,
        /// The location, if any. A create is the one write that sets it from nothing; an edit
        /// reshapes it through [`Intent::UpdateEvent`]'s `location`.
        location: Option<String>,
        /// How the event repeats, or `None` for a one-off. Changing the rule afterwards goes
        /// through [`Intent::UpdateEvent`]'s `recurrence`.
        #[uniffi(default = None)]
        recurrence: Option<SimpleRecurrence>,
    },
    /// Edit a stored calendar event, then refresh the agenda.
    ///
    /// A **provider-neutral patch**: only the fields present change; the recurrence rule,
    /// attendees, alarms and timezone survive. Every time field is a **wall clock in the
    /// event's own zone** (never a UTC instant), so a move cannot convert a zoned or all-day
    /// event: for an all-day event they are `YYYY-MM-DD` dates (end exclusive).
    ///
    /// The optional fields are three-state: absent leaves the property unchanged, an empty
    /// string clears it, a value sets it. `title`/`start`/`end` cannot be cleared (an event
    /// must keep them), so an empty value there is treated as "unchanged".
    UpdateEvent {
        /// The account that owns the event (the row's `account`).
        account: String,
        /// The event's provider key (the row's `event`/`key`).
        key: String,
        /// The new title, or `None`/empty to leave it.
        title: Option<String>,
        /// The new start wall-clock (`2026-07-01T10:00:00`, or `2026-07-01` if all-day), or
        /// `None`/empty to leave it.
        start: Option<String>,
        /// The new end, same terms as `start` (exclusive date if all-day), or `None`/empty.
        end: Option<String>,
        /// The new notes/description: `None` leaves, empty clears, a value sets.
        notes: Option<String>,
        /// The new location: `None` leaves, empty clears, a value sets.
        location: Option<String>,
        /// For a recurring event, the **original** start wall-clock of the single occurrence
        /// to edit (splitting an override out of the series); `None`/empty edits the whole
        /// series. See `TimedSegment::occurrence_start`.
        occurrence: Option<String>,
        /// What happens to the repeat rule: `None` leaves the series as it is, `Set` replaces
        /// the rule, `Clear` makes the event a single one.
        ///
        /// Only a **series** edit may carry one, and only over a rule the core described as
        /// `EventRecurrence::Simple`. Pairing it with `occurrence`, or sending one for an
        /// event whose rule read `Complex`, is refused rather than written; see
        /// [`RecurrenceChange`].
        #[uniffi(default = None)]
        recurrence: Option<RecurrenceChange>,
    },
    /// Move or resize a stored calendar event by **dragging** it on the grid, then refresh the
    /// agenda.
    ///
    /// A drag is a **delta, not a destination**, and that is the whole design. The client sends
    /// how far the hand moved (signed whole days and minutes) and the core applies it to the
    /// event's own wall clock. Three things fall out that a dropped date-and-time cannot give:
    ///
    /// - **The display zone never reaches the write.** A meeting in `Europe/Amsterdam` read on a
    ///   device set to `America/New_York` is drawn six hours earlier; the clock it was dropped
    ///   under is not the clock it must be written with. An offset is the same number in both.
    /// - **A clipped segment still works.** An event crossing midnight is drawn as one segment per
    ///   day, each clipped to its column, so a segment's `start_minutes` is `0` on every day but
    ///   the first; there is no absolute start on screen to send.
    /// - **A move preserves its duration exactly**, because both edges take the same offset.
    ///
    /// A client that snaps its drop to the quarter hour simply snaps the offset.
    ///
    /// **Only the user's own events may be dragged**: an appointment nobody was invited to, or
    /// a meeting this account organises. Gate the gesture on `TimedSegment::can_move`; the core
    /// re-checks and refuses, because a write must not trust its caller. Moving a meeting
    /// *somebody else* called is *propose a new time*, which is a separate feature.
    MoveEvent {
        /// The account that owns the event (the segment's `account`).
        account: String,
        /// The event's provider key (the segment's `event`).
        key: String,
        /// Which edges the drag moved.
        edge: EventEdge,
        /// Whole days the dragged edge(s) move by, signed.
        days: i32,
        /// Minutes within the day the dragged edge(s) move by, signed. Ignored for an all-day
        /// event, which has no clock to move along.
        minutes: i32,
        /// The occurrence that was dragged, as `TimedSegment::occurrence_start` gave it;
        /// passed back **verbatim**, never parsed or recomputed.
        ///
        /// `None`/empty moves the **whole series**. There is no default and there must not be
        /// one: dragging one Tuesday standup is not the same as rewriting every Tuesday to
        /// eternity, so a client whose segment carries a non-empty `occurrence_start` **asks**
        /// before it sends.
        occurrence: Option<String>,
    },
    /// Answer the invitation a message carries, then refresh the calendar **and** the
    /// reading view.
    ///
    /// Named by the **message**, never by the event: the answer goes out as the address the
    /// invitation matched, which on an aliased account is not the account's primary identity,
    /// and only the core knows the address set (`docs/invitations.md` §4).
    ///
    /// `comment` and `notify_organizer` are Outlook's "optional message" and "Email
    /// organiser" tick. **Offer them only when the card says so**;
    /// `InvitationCard::can_comment` / `can_choose_notify`. A transport that cannot honour one
    /// refuses the whole answer rather than dropping it, so sending a note to an account that
    /// cannot carry one loses the answer, not just the note.
    RespondToInvitation {
        /// The id of the account the message is in.
        account: String,
        /// The message's provider key.
        key: String,
        /// Accept, tentative, or decline.
        response: InvitationResponse,
        /// A note for the organiser. `None` or blank sends none. Only when `can_comment`.
        comment: Option<String>,
        /// Whether the organiser is told. Pass `true` unless `can_choose_notify` and the user
        /// cleared the tick: an invitation asks for a reply, so answering sends one.
        notify_organizer: bool,
        /// The **localised** subject for the reply, e.g. "Accepted: Sprint planning".
        ///
        /// On an account whose calendar server does no scheduling, the core sends the reply as
        /// an email itself, and this is the subject a stranger reads in their inbox, so it is
        /// the client's to translate; the core carries no locale. Compose it from the catalog:
        /// `invitation_reply_subject_accepted` / `_tentative` / `_declined`, with the meeting's
        /// summary. `None` is safe; it falls back to `Re:` plus the invitation's own subject;
        /// but it means the answer is not named in the subject line.
        reply_subject: Option<String>,
    },
    /// File the Sent copy of a message that went out without one, answering what
    /// `MailcalApp::unfiled_copy` is holding. **Sends nothing**: the message already left.
    /// Safe to dispatch twice: the core ignores a retry already in flight, and the provider
    /// checks for the copy before placing one.
    RetryUnfiledCopy,
    /// Dismiss the "your copy is not in Sent" question without filing it. The message stays
    /// sent, only the sender's record of it stays missing.
    DismissUnfiledCopy,
    /// Answer the question `MailcalApp::reply_prompt` is holding: whether to email the
    /// organiser ourselves after the calendar server reported it could not.
    ///
    /// Carries no handle on the meeting: the core holds the question, and clears it as soon as
    /// this arrives, so a modal dismissed twice cannot send two replies.
    AnswerReplyPrompt {
        /// Whether to send the email. `false` dismisses; the RSVP stays stored either way.
        send: bool,
        /// Whether this becomes the account's standing answer, so a server that fails every
        /// reply asks once instead of at every meeting. This is what a "don't ask again" or
        /// "always do this" tick sets.
        remember: bool,
        /// The **localised** subject for the reply, on the same terms as
        /// `RespondToInvitation::reply_subject`; compose it from the same catalog keys.
        reply_subject: Option<String>,
    },
    /// Delete a calendar event (or one occurrence of it) by its key, then refresh the agenda.
    DeleteEvent {
        /// The id of the account that owns the event (the row's `account`).
        account: String,
        /// The event's provider key.
        key: String,
        /// For a recurring event, the **original** start wall-clock of the single occurrence
        /// to remove; `None`/empty deletes the whole series. The same token
        /// `Intent::UpdateEvent` takes, from `TimedSegment::occurrence_start`, and the same
        /// question to put to the user, because cancelling one Tuesday and cancelling the
        /// standup are different requests.
        #[uniffi(default = None)]
        occurrence: Option<String>,
    },
    /// Report whether the device currently has network connectivity; dispatched on launch
    /// and whenever the OS reachability signal changes. Offline stops the app attempting
    /// syncs (and shows a banner); online triggers a refresh so mail catches up and dead
    /// connections heal.
    ReportNetworkReachable {
        /// Whether the device can currently reach the network.
        reachable: bool,
    },
    /// Report the device's current OS timezone (an IANA id); dispatched on launch and
    /// on the OS's zone-change signal. Adopted on first boot, else raised as a pending
    /// change when it differs from the active zone.
    ReportDeviceTimeZone {
        /// The device's IANA timezone id.
        id: String,
    },
    /// Set the active display timezone (an IANA id) via the selector.
    SetTimeZone {
        /// The chosen IANA timezone id.
        id: String,
    },
    /// Adopt the pending device timezone: the user accepted the change prompt.
    AcceptTimeZoneChange,
    /// Dismiss the pending device timezone; keep the current zone.
    DismissTimeZoneChange,
}

/// A foreign (Kotlin/Swift) observer the app notifies when a surface changes; the
/// host then pulls the new snapshot. Must be cheap and non-blocking.
#[uniffi::export(callback_interface)]
pub trait Observer: Send + Sync {
    /// Signals that `surface`'s snapshot changed.
    fn surface_changed(&self, surface: Surface);
}
