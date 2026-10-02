//! The [`Intent`] enum: the single inbound channel of the unidirectional loop, as the FFI exposes
//! it. Split from [`super`] (the surfaces, the observer and the small enums an intent carries),
//! mirroring `mailcal-app`'s own `protocol` / `protocol::intent` split; `lib.rs` re-exports both
//! halves, so a host sees one API.
//!
//! An enum cannot be split the way a module can, so a family that grows gets a nested enum of its
//! own behind one variant: [`EventIntent`](super::EventIntent),
//! [`FolderIntent`](super::FolderIntent), [`OutboxIntent`](super::OutboxIntent).

use super::{BulkAction, InvitationResponse, SearchScope, SelectedRow};
// Named only by an intra-doc link on a variant below, which rustdoc resolves against this
// module's scope rather than the parent's.
#[allow(unused_imports, reason = "named by an intra-doc link on a variant")]
use crate::Surface;
use crate::{ContactEdit, ViewMode};

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
    /// **Not navigation**, and neither are the two below it: they change neither the selected
    /// account nor the selected folder, so any number of trees can stand open and moving to All
    /// Inboxes, the calendar or contacts leaves them as they were. Render each chevron from the
    /// snapshot rather than keeping client-side state (`docs/folder-pane.md`): this one from
    /// `AccountRow::expanded`.
    SetAccountExpanded {
        /// The account whose tree to open or shut.
        account: String,
        /// Whether the tree is open.
        expanded: bool,
    },
    /// Open or shut the **All Accounts** group's tree, which sits above the accounts and holds
    /// the unified Inbox row. Its chevron is `MailboxListSnapshot::unified_expanded`.
    SetUnifiedExpanded {
        /// Whether the group's tree is open.
        expanded: bool,
    },
    /// Open or shut the folders filed **inside** one folder: an account's tree one level down.
    /// Both halves travel together for `SelectFolder`'s reason, and what to draw comes from
    /// `FolderRow` (`docs/folder-pane.md`, rule 19).
    SetFolderExpanded {
        /// The account whose folder this is.
        account: String,
        /// The folder key, from the row the user pressed.
        key: String,
        /// Whether the folders inside it are showing.
        expanded: bool,
    },
    /// Set the name one account's outgoing mail is sent under: the `Name` in
    /// `Name <address>`.
    ///
    /// An empty `name` clears it and the account sends as a bare address. The core sanitises
    /// what it is given, so a client passes the field's text through unchanged rather than
    /// validating it itself. Read the current value from `AccountRow::name`, and ask
    /// `sender_name_editable` before offering the field: on a Microsoft mailbox the name is
    /// the organisation's and cannot be changed here (`docs/sending.md`).
    SetAccountSenderName {
        /// The account whose sender name to set.
        account: String,
        /// The name to send under; empty clears it.
        name: String,
    },
    /// Show one folder's mail: the folder key and the account that owns it **together**.
    ///
    /// A folder key is unique only within its account (every provider calls its inbox `inbox`)
    /// and the pane shows every account's tree at once, so pass the account the **row** sits
    /// under, never whichever one is selected. There is no folder-only form; dispatching a key
    /// alone used to leave the list exactly as it was (`docs/folder-pane.md`, rule 14). An
    /// account'''s whole mailbox is [`Intent::SelectAccount`], the pane'''s other destination.
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
    /// Open a message (by key) for reading in the **pane**: fetch + cache its source, extract
    /// and sanitise the body, then publish the [`Surface::Reading`] snapshot, which is read
    /// back with [`MailcalApp::reading_view`](crate::MailcalApp::reading_view).
    OpenMessage {
        /// The id of the account that owns the message (the row's `account`).
        account: String,
        /// The message's provider key.
        key: String,
    },
    /// [`Intent::OpenMessage`], into a detached reading window rather than the pane
    /// (`docs/reading-window.md`), so the two show different messages at the same time. The
    /// fetch, the mark-read and the loading threshold are the pane's; only the slot differs,
    /// and it is read back with
    /// [`MailcalApp::reading_window_view`](crate::MailcalApp::reading_window_view).
    OpenMessageInWindow {
        /// The window's id: the host mints one per window and keeps it for the window's life,
        /// then spends it on
        /// [`MailcalApp::close_reading_window`](crate::MailcalApp::close_reading_window). The
        /// pane's slot is not a string and cannot be named here.
        window: String,
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
    /// Save a new contact into one address book, then refresh the list.
    ///
    /// `account`/`address_book` are the picker's choice, from `MailcalApp::contact_targets`;
    /// both `None` files it in the first writable book on offer, which is the whole picker for
    /// a user with one account. Awaited inline, its outcome surfaced through
    /// `MailcalApp::contact_write_status`.
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
    /// several accounts' cards joined on a shared address, and saving the merged values would
    /// file one account's details in another's address book. Take the pair from
    /// [`ContactDetail::editable_cards`](crate::ContactDetail::editable_cards), asking the user
    /// which card when there is more than one.
    ///
    /// A **patch**: only the fields the form actually changed are sent, so an address's label,
    /// an organisation's departments, a postal address and a photo all survive an edit that did
    /// not touch them. An edit that changed nothing sends nothing.
    UpdateContact {
        /// The person whose card this is (the row's `id`). The card is looked up among that
        /// person's sources, so a row held across a merge still opens the card it meant.
        person: String,
        /// The account holding the card.
        account: String,
        /// The card's provider id.
        card: String,
        /// The values the form holds.
        edit: ContactEdit,
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
    /// Apply one action to **every** selected row at once (`docs/list-selection.md`).
    ///
    /// Deliberately not a loop over the single-row intents: each of those re-syncs the account
    /// it touched, so a hundred selected rows would be a hundred account-wide syncs. This hides
    /// every affected row in one go and syncs each account once. A conversation row stands for
    /// its whole thread, and a move leaves a copy filed in Sent where it is.
    ActOnSelection {
        /// The selected rows, in list order; they may span accounts.
        rows: Vec<SelectedRow>,
        /// What to do to them.
        action: BulkAction,
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
    /// A write to a calendar event: create, edit, drag or delete it
    /// ([`EventIntent`](super::EventIntent)), then refresh the agenda.
    Events {
        /// Which write.
        intent: super::EventIntent,
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
    /// An action on the Outbox ([`OutboxIntent`](super::OutboxIntent)).
    Outbox {
        /// Which Outbox action.
        intent: super::OutboxIntent,
    },
    /// A change to an account's folder tree ([`FolderIntent`](super::FolderIntent)).
    Folders {
        /// Which change.
        intent: super::FolderIntent,
    },
    /// The host's composer now holds the message `Surface::ComposeRequest` offered.
    DismissComposeRequest,
    /// Report whether the device can reach the network; on launch and on every OS change.
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
