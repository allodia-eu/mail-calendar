//! The [`Intent`] enum: the single inbound channel of the unidirectional loop.
//!
//! Split from [`super`] (the surfaces, the observer and the small records an intent carries)
//! to keep each file under the 500-line limit, nothing about the enum changed in the move.

use engine_api::{AccountId, LocalDateTime};
use mailcal_account::{ContactEdit, EventDrag, EventEdit};
use mailcal_composer::ComposerDocument;
use mailcal_viewmodel::{QuoteStyleKind, SwipeActionKind, SwipeDirection, ViewMode};

// Named only by intra-doc links below, which rustdoc resolves against this module's scope.
#[allow(unused_imports, reason = "named by intra-doc links on the variants")]
use super::Surface;
use super::{ComposerBlob, SearchScope};
use crate::{
    invitations_rsvp::InvitationResponse,
    reference::{EventRef, FolderRef, MessageRef, ThreadRef},
};

/// A host intent: the single inbound channel of the unidirectional loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Intent {
    /// Sync the account's mail (every connected folder) and refresh the mailbox-list
    /// snapshot.
    RefreshMail,
    /// Switch the mailbox list between a flat and a threaded view.
    SetViewMode(ViewMode),
    /// Show full-text search results for a query (newest first), or clear search (`None`)
    /// to return to the folder view. Clearing also resets the scope to its default, so the
    /// next search opens across everything rather than inheriting the last one's narrowing.
    Search(Option<String>),
    /// Narrow (or re-widen) the active search to a [`SearchScope`]. Independent of the
    /// query, so toggling the filter re-projects without retyping.
    SetSearchScope(SearchScope),
    /// Show one account's folders (by account id), or the unified "all inboxes" view
    /// (`None`). Resets the selected folder.
    SelectAccount(Option<String>),
    /// Open or shut one account's folder tree in the sidebar, and remember it across launches.
    ///
    /// Deliberately **not** navigation: it changes neither the selected account nor the
    /// selected folder, so any number of trees can be open at once and moving to All Inboxes,
    /// the calendar or contacts leaves them all as they were. A client renders the chevron from
    /// [`AccountRow::expanded`](mailcal_viewmodel::AccountRow) and never keeps its own copy
    /// (`docs/folder-pane.md`).
    SetAccountExpanded {
        /// The account whose tree to open or shut.
        account: String,
        /// Whether the tree is open.
        expanded: bool,
    },
    /// Show one folder's mail. A [`FolderRef`], never a bare key: a key is unique only within
    /// its account, and there is no folder-only form (`docs/folder-pane.md`, rule 14). An
    /// account's own all-mail view is [`Intent::SelectAccount`].
    SelectFolder {
        /// The folder to show, bound to its owning account.
        folder: FolderRef,
    },
    /// Grow the visible mailbox-list window by one page: the host dispatches this as it
    /// scrolls toward the end of the list, so only the rows in view are built and crossed
    /// the FFI. Any navigation that changes the list (select account/folder, search, switch
    /// view mode) resets the window to the first page.
    ShowMore,
    /// Open a message (by key) for reading: fetch + cache its raw source, extract and
    /// sanitise the body, and publish the [`Surface::Reading`] snapshot.
    OpenMessage {
        /// The message to open; its account and provider key bound together, so a key
        /// two accounts share resolves within the right one.
        message: MessageRef,
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
    /// Send a rich composer document through the durable outbox, resolving local
    /// composer blob handles to attachment bytes before submission.
    SubmitRichMail {
        /// The account to send **from**: the composer's From dropdown. `None` derives it: the
        /// selected account, else the app-level default send account, else the first configured
        /// one. An account that isn't configured fails the send rather than substituting another.
        from: Option<AccountId>,
        /// The `To` recipients: a comma-separated address list the user entered.
        to: String,
        /// The `Cc` recipients: a comma-separated address list (may be empty).
        cc: String,
        /// The `Bcc` recipients: a comma-separated address list (may be empty);
        /// delivered but hidden from the other recipients.
        bcc: String,
        /// The subject line.
        subject: String,
        /// The shared composer document.
        document: ComposerDocument,
        /// Host-resolved bytes for every attachment handle referenced by `document`.
        blobs: Vec<ComposerBlob>,
    },
    /// Sync the account's calendar(s) and refresh the agenda snapshot.
    RefreshCalendar,
    /// Sync every account's address books and refresh the contacts snapshot.
    RefreshContacts,
    /// Narrow the contacts list to people matching `query`, or show all for an empty one.
    ///
    /// The matching happens in the engine (name, email, phone, organisation, title), so a
    /// person outside the snapshot's row cap is still findable, which is why this is an
    /// intent that rebuilds rather than a filter the host applies to the rows it holds.
    SearchContacts {
        /// The search text; empty clears the filter.
        query: String,
    },
    /// Save a new contact into one address book, then refresh the list.
    ///
    /// `account`/`address_book` are the client's picker choice, from
    /// [`App::contact_targets`](crate::App::contact_targets); both `None` files it in the
    /// first writable book on offer, which is the whole picker for a user with one account.
    /// A user with no writable book anywhere is offered no create at all, so this failing for
    /// want of a destination means the client offered something it should not have.
    ///
    /// Awaited inline like the calendar writes, its outcome surfaced through
    /// [`ContactWriteStatus`](crate::ContactWriteStatus). Not durable offline: a failed save
    /// stays failed rather than queueing.
    CreateContact {
        /// The chosen book's owning account, or `None` for the first on offer.
        account: Option<String>,
        /// The chosen book's provider id, or `None` for the first on offer.
        address_book: Option<String>,
        /// The values the form holds.
        edit: ContactEdit,
    },
    /// Edit one **source card** of a person, then refresh the list.
    ///
    /// Named by a card and not by a person, which is the load-bearing half: a person is
    /// several accounts' cards joined on a shared address (`docs/contacts.md` §1), and saving
    /// the merged values would file one account's details in another's address book. A client
    /// takes the pair from
    /// [`ContactDetail::editable_cards`](mailcal_viewmodel::ContactDetail::editable_cards),
    /// asking the user which card when there is more than one.
    ///
    /// The write is a **patch**: only the fields the form actually changed are sent, so an
    /// address's label, an organisation's departments, a postal address and a photo all
    /// survive an edit that did not touch them. An edit that changed nothing sends nothing.
    UpdateContact {
        /// The person whose card this is, as the row carried it. The card is looked up among
        /// that person's sources, so a retired id still opens the card it always meant.
        person: String,
        /// The account holding the card.
        account: String,
        /// The card's provider id.
        card: String,
        /// The values the form holds.
        edit: ContactEdit,
    },
    /// Mark a message read (`read = true`) or unread (`read = false`), by key.
    MarkRead {
        /// The message to mark; its account and provider key bound together.
        message: MessageRef,
        /// Whether to mark it read.
        read: bool,
    },
    /// Flag (`flagged = true`) or unflag a message, by key.
    SetFlagged {
        /// The message to flag; its account and provider key bound together.
        message: MessageRef,
        /// Whether to flag it.
        flagged: bool,
    },
    /// Delete a message (move it to Trash; recoverable).
    Delete {
        /// The message to delete; its account and provider key bound together.
        message: MessageRef,
    },
    /// **Permanently** delete a message (irreversible: not a Trash move).
    PermanentlyDelete {
        /// The message to delete; its account and provider key bound together.
        message: MessageRef,
    },
    /// Archive a message; move it to **its account's** Archive folder (resolved by role).
    /// A no-op when the account has no Archive folder.
    Archive {
        /// The message to archive; its account and provider key bound together.
        message: MessageRef,
    },
    /// Archive a whole conversation; move **every** message on the thread to the account's
    /// Archive folder **except** those filed in the Sent folder, which are never moved out of
    /// Sent (so reopening the thread from Archive still shows both the received messages and the
    /// owner's own Sent replies, which the view-model gathers across folders). A no-op when the
    /// account has no Archive folder or the thread has no archivable message.
    ArchiveThread {
        /// The conversation to archive; its account and thread id bound together.
        thread: ThreadRef,
    },
    /// Mark a message as spam; **report** it as junk to its account's provider, which files it
    /// under Junk (RFC 6154 `\Junk` role, conventional-name fallback) *and* trains its filter;
    /// one that cannot be told still gets it filed. A no-op with no resolvable Junk folder.
    MarkAsSpam {
        /// The message to mark as spam; its account and provider key bound together.
        message: MessageRef,
    },
    /// Mark a message as not spam; report it as not-junk, filing it back in **its account's**
    /// Inbox (RFC 6154 `\Inbox` role) and telling the provider it had this one wrong. A no-op
    /// with no resolvable Inbox.
    MarkAsNotSpam {
        /// The message to un-spam; its account and provider key bound together.
        message: MessageRef,
    },
    /// Reply to a message (by key) with a rich composer document. The host supplies the
    /// `to`/`cc`/`bcc` recipients (pre-filled from [`crate::App::reply_recipients`];
    /// reply or reply-all, and editable by the user) and optionally the `from` account; the app
    /// derives the `Re:` subject and the `In-Reply-To`/`References` chain from the original so
    /// the reply threads, renders the document into a rich draft (resolving blob handles to
    /// bytes), then sends.
    SubmitRichReply {
        /// The message being replied to; its account (which holds the original) and
        /// provider key bound together.
        message: MessageRef,
        /// The account to send **from**: the composer's From dropdown. `None` replies from the
        /// account that received the original (`message.account`), the default. An account that
        /// isn't configured fails the send rather than substituting another.
        from: Option<AccountId>,
        /// The `To` recipients: a comma-separated address list (a reply-all carries the
        /// other thread participants here too).
        to: String,
        /// The `Cc` recipients: a comma-separated address list (reply-all pre-fills the
        /// other original recipients; may be empty).
        cc: String,
        /// The `Bcc` recipients: a comma-separated address list (may be empty); delivered
        /// but hidden from the other recipients.
        bcc: String,
        /// The shared composer document.
        document: ComposerDocument,
        /// Host-resolved bytes for every attachment handle referenced by `document`.
        blobs: Vec<ComposerBlob>,
    },
    /// Forward a message (by key) with a rich composer document (a `Fwd:` subject; the
    /// original's `References` chain and no `In-Reply-To`, so the forward stays on the
    /// conversation it came from without answering a message), to the host-supplied
    /// `to`/`cc`/`bcc` recipients. Renders the document into a rich draft (resolving blob
    /// handles to bytes), then sends.
    SubmitRichForward {
        /// The message being forwarded; its account (which holds the original) and
        /// provider key bound together.
        message: MessageRef,
        /// The account to send **from**: the composer's From dropdown. `None` forwards from the
        /// account that holds the original (`message.account`), the default. An account that
        /// isn't configured fails the send rather than substituting another.
        from: Option<AccountId>,
        /// The `To` recipients: a comma-separated address list the user entered.
        to: String,
        /// The `Cc` recipients: a comma-separated address list (may be empty).
        cc: String,
        /// The `Bcc` recipients: a comma-separated address list (may be empty); delivered
        /// but hidden from the other recipients.
        bcc: String,
        /// The shared composer document.
        document: ComposerDocument,
        /// Host-resolved bytes for every attachment handle referenced by `document`.
        blobs: Vec<ComposerBlob>,
    },
    /// Create a calendar event, then refresh the agenda.
    ///
    /// `account`/`calendar` are the client's calendar-picker choice: the owning account id and
    /// the `CalendarRow.id` key. Both `None` files the event in the default writable account's
    /// first calendar (the legacy behaviour). `all_day` selects the event form and how the times
    /// are read; `timezone` (when set) creates a timed event in that zone; `notes` is the
    /// description; `location` is the place. A create is the one write that sets a location
    /// from nothing: an edit reshapes it through [`Intent::UpdateEvent`].
    CreateEvent {
        /// The event title.
        title: String,
        /// A timed event's start: a **wall clock** (`2026-07-01T10:00:00`) when `timezone` is set,
        /// else an RFC 3339 UTC instant (`2026-07-01T10:00:00Z`). For an all-day event, the start
        /// date (`2026-07-01`).
        start: String,
        /// The end, same terms as `start`. For an all-day event the end date is **exclusive**
        /// (a one-day event on the 1st ends on the 2nd).
        end: String,
        /// The owning account of the chosen calendar, or `None` for the default.
        account: Option<String>,
        /// The chosen calendar's row key (`CalendarRow.id`), or `None` for the account's first.
        calendar: Option<String>,
        /// Whether this is an all-day event (changes how `start`/`end` are parsed).
        all_day: bool,
        /// The IANA zone a timed event is created in: the device's zone, so it reads back the
        /// same clock on edit. `None`/empty falls back to UTC (and `start`/`end` are UTC
        /// instants).
        timezone: Option<String>,
        /// The description, if any.
        notes: Option<String>,
        /// The location, if any.
        location: Option<String>,
        /// How the event repeats, or `None` for a one-off. Changing the rule afterwards goes
        /// through [`Intent::UpdateEvent`].
        recurrence: Option<mailcal_account::SimpleRecurrence>,
    },
    /// Edit a stored calendar event; retitle, move, resize, change its notes or location;
    /// then refresh the agenda.
    ///
    /// The write is a provider-neutral patch, so the adapter applies only the changed
    /// properties and the recurrence rule, attendees, alarms and timezone survive.
    /// Rebuilding the document instead, which is all [`Intent::CreateEvent`] can do;
    /// would delete every one of them and report success.
    ///
    /// Rides the same inline-await write path as create/delete, its outcome surfaced through
    /// [`CalendarWriteStatus`](crate::CalendarWriteStatus) (`Saving` → `Saved`/`Failed`). Like
    /// them it is **not durable offline** yet: a failed edit stays failed rather than queueing
    /// (the shared outbox follow-up).
    UpdateEvent {
        /// The event to edit; its account and provider key bound together.
        event: EventRef,
        /// What to change, in the event's **own wall clock**; see
        /// [`EventEdit`](mailcal_account::EventEdit). A move must not convert a zoned or
        /// all-day event, so the edit names wall-clock times, never UTC instants.
        edit: EventEdit,
    },
    /// Move or resize a stored calendar event by **dragging** it on the grid, then refresh the
    /// agenda.
    ///
    /// A sibling of [`Intent::UpdateEvent`] rather than a special case of it, because a drag
    /// says something the editor cannot: *this far*, not *to here*. The client sends a signed
    /// offset in whole days and minutes and the core applies it to the event's own wall clock;
    /// so nothing about the zone the grid was drawn in reaches the write, a segment clipped to
    /// its day column needs no absolute anchor, and a move preserves its duration exactly. The
    /// reasoning in full: [`mailcal_account::apply_event_drag`].
    ///
    /// **Refused unless the event is the user's own**; their appointment, or a meeting they
    /// organise. A client gates the gesture on `TimedSegment::can_move`; this checks the same
    /// rule again, because a write must not trust a caller.
    ///
    /// Rides the same inline-await patch path as [`Intent::UpdateEvent`], with the same
    /// [`CalendarWriteStatus`](crate::CalendarWriteStatus) reporting and the same lack of an
    /// offline outbox.
    MoveEvent {
        /// The event to move; its account and provider key bound together.
        event: EventRef,
        /// The drag: which edges moved, how far, and which occurrence of a series it was.
        drag: EventDrag,
    },
    /// Answer the invitation the open message carries, then refresh the calendar **and** the
    /// reading view.
    ///
    /// Named by the **message**, not by an event: the address the answer goes out as is the
    /// one the invitation matched, which on an aliased account is not the account's primary
    /// identity, and only the core can work that out (`docs/invitations.md` §4). A client
    /// that named the event would have to know the alias rule too.
    ///
    /// `comment` and `notify_organizer` are Outlook's "optional message" and "Email
    /// organiser" tick, and **a client may only offer them when the card says so**
    /// (`InvitationCard::can_comment` / `can_choose_notify`). Both are refused rather than
    /// dropped by a transport that cannot honour them, so sending one unasked is an error,
    /// not a silent no-op.
    ///
    /// Rides the same inline-await write path as the other calendar writes, its outcome
    /// surfaced through [`CalendarWriteStatus`](crate::CalendarWriteStatus). Not durable
    /// offline yet.
    RespondToInvitation {
        /// The message carrying the invitation; its account and provider key bound together.
        message: MessageRef,
        /// Accept, tentative, or decline.
        response: InvitationResponse,
        /// An optional note for the organiser. `None`, or blank, sends none.
        comment: Option<String>,
        /// Whether the organiser is told. `true` is the RFC 5546 default: an invitation asks
        /// for a reply, so answering sends one.
        notify_organizer: bool,
        /// The localised subject for the reply **this core sends itself**, e.g. "Accepted:
        /// Sprint planning". Used only on the client-iMIP route; ignored where the calendar
        /// server sends the reply, since then no message of ours exists to put it on.
        ///
        /// It comes from the client because the core has no locale (`AGENTS.md` →
        /// "Localisation is client-side"), and this is copy a stranger reads in their inbox.
        /// `None` falls back to `Re: <the invitation's own subject>`; deliberately not an
        /// English "Accepted: …", which would be a worse answer than quoting the organiser's
        /// own words back at them.
        reply_subject: Option<String>,
    },
    /// Answer the question raised when a calendar server reported it could not deliver a
    /// reply (`Surface::InvitationReply`): send the reply as email ourselves, or don't, and
    /// optionally stop asking for this account.
    ///
    /// Carries no handle on the meeting. The prompt the core is holding names it, and the send
    /// re-derives the whole reply from the message, so a client cannot answer a question the
    /// core is no longer asking, and two clicks cannot send two replies.
    AnswerReplyPrompt {
        /// Whether to email the organiser. `false` dismisses the question, leaving the RSVP
        /// stored (it was never in doubt) and the organiser untold.
        send: bool,
        /// Whether this becomes the account's standing answer, so a server that fails every
        /// reply asks once instead of at every meeting. `false` asks again next time.
        remember: bool,
        /// The localised subject for the reply, on the same terms as
        /// [`RespondToInvitation::reply_subject`](Self::RespondToInvitation): the core has no
        /// locale, and this is copy a stranger reads in their inbox.
        reply_subject: Option<String>,
    },
    /// File the Sent copy of a message delivered without one ([`Surface::UnfiledCopy`]).
    /// **Sends nothing.** Carries no handle on the message: the core holds the one it is
    /// asking about, so a double-tap cannot file two copies.
    RetryUnfiledCopy,
    /// Dismiss the "your copy is not in Sent" question without filing it. The message stays
    /// sent either way.
    DismissUnfiledCopy,
    /// Delete a calendar event, or one occurrence of it, then refresh the agenda.
    DeleteEvent {
        /// The event to delete; its account and provider key (resource href) bound
        /// together, so a key two accounts share affects only the owning account.
        event: EventRef,
        /// Which occurrence of a recurring event to remove, named by its **original**
        /// start. `None` deletes the whole series. There is no default on purpose, for the
        /// same reason [`EventEdit`](mailcal_account::EventEdit)`::occurrence` has none:
        /// cancelling one Tuesday and cancelling the standup are different requests, and
        /// only the user knows which they meant: so ask them.
        occurrence: Option<LocalDateTime>,
    },
    /// Report whether the device currently has network connectivity (from the host's OS
    /// reachability API). The host dispatches this on launch and whenever reachability
    /// changes. Going offline stops the app attempting network syncs (and shows a banner);
    /// coming back online triggers a refresh so mail catches up and dead connections heal.
    ReportNetworkReachable(bool),
    /// Report the device's current OS timezone (an IANA id). The host dispatches this
    /// on launch and whenever the OS signals a zone change; the app adopts it on first
    /// boot, else raises a pending change for the user to accept or dismiss when it
    /// differs from the active zone.
    ReportDeviceTimeZone(String),
    /// Set the active display timezone explicitly via the selector (an IANA id). Clears
    /// any pending device-zone change and re-orders the agenda.
    SetTimeZone(String),
    /// Adopt the pending device timezone: the user accepted the change prompt.
    AcceptTimeZoneChange,
    /// Dismiss the pending device timezone; keep the current zone.
    DismissTimeZoneChange,
    /// Set the persisted default reply/forward quote style. The host seeds a new reply's
    /// composer with this style; signals [`Surface::Settings`].
    SetQuoteStyle(QuoteStyleKind),
    /// Set whether the composer offers a per-message quote-style override. Off by default: a
    /// reply or forward silently uses the app default and shows no picker. Signals
    /// [`Surface::Settings`].
    SetQuoteStylePerMessage(bool),
    /// Set the persisted **default send account**: the account a new message composes from in
    /// the unified all-inboxes view, where no selected mailbox scopes the choice. `None` clears
    /// it (falling back to the first configured account); signals [`Surface::Settings`].
    SetDefaultSendAccount(Option<String>),
    /// Set what one swipe direction does to a message row (Trash / Archive / Star). The two
    /// directions are configured independently; signals [`Surface::Settings`].
    SetSwipeAction {
        /// Which swipe to bind.
        direction: SwipeDirection,
        /// What that swipe does.
        action: SwipeActionKind,
    },
}
