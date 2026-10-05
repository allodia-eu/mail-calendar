//! Relm4 input messages for the Linux shell.

use std::path::PathBuf;

use mailcal_bindings::{
    AccountCapability, AgentDraft, BulkAction, ComposeRequest, ContactDetail, ContactEdit,
    ContactTarget, DetectedSetup, ImapAuthOffer, Intent, MailtoPrefill, SearchScope, SharePrefill,
    Surface,
};

use super::{
    allodia::AllodiaOutcome,
    allodia_subscription::SubscriptionInput,
    allodia_sync::AllodiaSyncOutcome,
    calendar::{CalendarMode, CreateSlot, EventForm, EventIdentity},
    composer_model::{ComposerSubmission, PickedFile},
    contacts::EditTarget,
    folder_actions::FolderInput,
    folder_pane::SidebarTarget,
    google::GoogleOutcome,
    imap_signin::{ImapOutcome, ImapPrepared},
    invitation::InvitationAnswer,
    jmap::{JmapOutcome, JmapPrepared, JmapReauthOutcome, JmapReauthPrepared},
    mail_actions::{ActionKind, MailActionRequest, MessageTarget},
    mailbox::ThreadKey,
    microsoft::MicrosoftOutcome,
    model::OpenedMessage,
    outbox::{QueuedAction, QueuedTarget},
    reader::{ComposerHost, ReadingSource},
    selection::SelectMode,
    setup_model::{AccountSubmission, ImapForm, ManualForm},
};
use crate::boot::BootedApp;

pub(crate) enum AppInput {
    /// The core's constructor has returned on its worker thread (`docs/boot-sequence.md`). Every
    /// input that arrived before it was held, and reaches the model after it.
    Booted(Result<BootedApp, String>),
    /// The launch page has been blank for `launch_status_after_ms()`, so it now says what the
    /// wait is for. One that fires after [`Self::Booted`] has nothing left to show.
    LaunchStatusDue,
    RefreshRequested,
    RetryUnfiledCopy,
    DismissUnfiledCopy,
    RefreshCalendar,
    PeriodicCalendarRefresh,
    SurfaceChanged(Surface),
    NetworkReachabilityChanged(bool),
    CheckDeviceTimeZone,
    AcceptDeviceTimeZone,
    DismissDeviceTimeZone,
    ResolveExpiredSignIn,
    ResolveMailReauth,
    ResolveCalendarReauth,
    ShowMail,
    ShowCalendar,
    ShowContacts,
    RefreshContacts,
    SearchContacts(String),
    OpenContact(String),
    /// One detail lookup's answer, tagged with the lookup it belongs to so a slower earlier one
    /// cannot land on top of the person the user opened since. `None` means the person is gone.
    ContactOpened(u64, Option<Box<ContactDetail>>),
    /// The writable address books, read off the UI thread when the surface opened; they decide
    /// whether a create is offered at all.
    ContactTargetsLoaded(Vec<ContactTarget>),
    BeginNewContact,
    /// The Edit button beside the open person: straight into the form with one editable card,
    /// into the "which account?" question with several.
    EditOpenContact,
    /// Edit one named card: from the Edit button, or from the question's answer.
    BeginEditContact(String, String),
    /// One card's values, read off the UI thread. `None` means the card has gone since.
    ContactCardLoaded(EditTarget, Option<Box<ContactEdit>>),
    /// The editor's Save, carrying the intent the editor already built and validated.
    SubmitContactForm(Box<Intent>),
    DismissContactEditor,
    SetCalendarMode(CalendarMode),
    StepCalendar(i32),
    CalendarToday,
    ManageCalendars,
    ShowCalendarDay(String),
    ToggleAllDay,
    OpenCalendarEvent(EventIdentity),
    BeginNewEvent,
    BeginNewEventAt(CreateSlot),
    BeginEditEvent,
    /// The editor's Save, with which occurrences the user said it meant: `true` splits an
    /// override out of the series, `false` rewrites the series itself (and is the only thing an
    /// editor opened on the series can mean).
    SubmitEventForm(Box<EventForm>, bool),
    RequestDeleteEvent(EventIdentity),
    RequestDeleteCurrentEvent,
    DeleteCalendarEvent(EventIdentity),
    DismissCalendarDialog,
    /// What is in the mail-search field. Empty leaves search, which the core answers by
    /// restoring the account and folder the search was opened from.
    SearchMail(String),
    SetSearchScope(SearchScope),
    /// The horizon line's route to the setting that decides how far back a search can reach.
    OpenSyncDepthSettings,
    OpenThreadMessage(Box<OpenedMessage>),
    /// Open one message in a window of its own: a double-click on its row, or the named item on
    /// the row's menu (`docs/reading-window.md`). Opening a row whose window is already up brings
    /// that window forward rather than raising a second one on the same mail.
    OpenMessageInWindow(Box<OpenedMessage>),
    /// A reading window has gone, by the id the core holds its body under. The host forgets the
    /// header and the core drops the body; a closed window may not keep either.
    CloseReadingWindow(String),
    /// A composer window has gone: sent, discarded, or closed, which leaves its draft in Drafts.
    CloseComposerWindow(u64),
    SetThreadExpanded {
        thread: ThreadKey,
        expanded: bool,
    },
    /// A click on the row at `index` of the snapshot, with what the modifiers made it mean.
    ///
    /// The index, not the row: the gesture reads a position out of the `GtkListBox` and the model
    /// resolves it against the snapshot it is holding, so no widget has to carry a message key.
    SelectRow {
        index: usize,
        mode: SelectMode,
    },
    /// Select every row the list is showing, which is the loaded window rather than the whole
    /// folder (`docs/list-selection.md`, rule 10).
    SelectAllRows,
    ClearSelection,
    /// Run one action over every selected row, as a single batch in the core. A permanent delete
    /// asks first and arrives back as [`Self::PerformSelectionAction`].
    ActOnSelection(BulkAction),
    /// Run one action the user has already confirmed. Emitted only by the permanent-delete
    /// confirmation, so a pending dialog can never be mistaken for consent.
    PerformSelectionAction(BulkAction),
    PerformMailAction(Box<MailActionRequest>),
    PerformOpenedMailAction {
        source: ReadingSource,
        action: ActionKind,
    },
    RequestPermanentDelete(MessageTarget),
    DismissPermanentDelete,
    ArchiveThread {
        account: String,
        thread_id: String,
    },
    ActivateSidebar(SidebarTarget),
    SetAccountExpanded {
        account: String,
        expanded: bool,
    },
    /// Open or shut the folders filed inside one folder. The account travels with the key, as it
    /// does for a folder's own activation: a folder key is unique only within its account.
    SetFolderExpanded {
        account: String,
        key: String,
        expanded: bool,
    },
    /// Show the Outbox: every account's unsent messages, in one list.
    ShowOutbox,
    /// Change the folder tree, or file dropped mail (`docs/folder-pane.md`, rules 22 to 29).
    Folder(FolderInput),
    /// Send now, withdraw, or reopen one queued message (`docs/sending.md`).
    QueuedSendAction {
        target: QueuedTarget,
        action: QueuedAction,
    },
    /// Answer the invitation the named reader's message carries. The **message** is named where
    /// this is dispatched, never the event: the answer goes out as the address the invitation
    /// matched, and only the core knows the address set (`docs/invitations.md` §4).
    RespondToInvitation(ReadingSource, Box<InvitationAnswer>),
    /// Answer the standing "the organiser wasn't told" question. Carries no handle on the meeting,
    /// the core holds the question and clears it as this arrives, so pressing twice cannot email
    /// the organiser twice.
    AnswerReplyPrompt {
        send: bool,
        remember: bool,
        reply_subject: String,
    },
    LoadRemoteImages(ReadingSource),
    RetryOpen(ReadingSource),
    OpenMailto(Box<MailtoPrefill>),
    OpenShare(Box<SharePrefill>),
    OpenAgentDraft(Box<AgentDraft>),
    BeginNew,
    /// Reply to what the named reader has open, `all` for reply-all. The reader decides where the
    /// draft goes: the pane's reply replaces the pane, a window's opens a composer window
    /// (`docs/reading-window.md`).
    BeginReply {
        source: ReadingSource,
        all: bool,
    },
    BeginForward(ReadingSource),
    /// The files the forwarded message carries, staged off the GTK thread, or `Err` when they
    /// could not be read. The composer opens on this rather than on `BeginForward`: on screen
    /// holding nothing it can be sent in the window before they arrive.
    ForwardStaged(ReadingSource, Result<Vec<PickedFile>, ()>),
    /// The composer's Discard button: its host, and whether anything was written in it.
    DiscardComposer(ComposerHost, bool),
    /// Leaving a composer nothing was written in: close it, then take any waiting navigation.
    ComposerUntouched(ComposerHost),
    /// Leaving a composer that was written in: save and close it, then take any navigation.
    LeaveComposer(Box<ComposerSubmission>),
    /// The "Discard draft?" question's Discard: throw the draft away.
    DiscardDraft,
    /// The question's Keep editing: leave the composer as it was.
    KeepEditing,
    SubmitComposer(Box<ComposerSubmission>),
    /// Store the composer's message in Drafts over this composition's previous save. The Save
    /// button and the idle timer both emit it; the core cannot tell them apart (`docs/drafts.md`).
    SaveComposerDraft(Box<ComposerSubmission>),
    /// A draft the core opened back up, under the composition it was adopted into. `Err` is said,
    /// never shown as an empty composer, which would replace the draft on its next save.
    DraftResumed(Box<Result<ComposeRequest, ()>>),
    SaveAttachment {
        source: ReadingSource,
        id: u32,
        destination: PathBuf,
    },
    OpenAttachment {
        source: ReadingSource,
        id: u32,
        file_name: String,
    },
    AttachmentSaved(bool),
    /// Write the named reader's message out as the file the user just named.
    ExportMessage {
        source: ReadingSource,
        destination: PathBuf,
    },
    MessageExported(bool),
    /// A print the dialog accepted could not be laid out or handed to the printer.
    PrintFailed,
    AttachmentDecoded(Result<PathBuf, ()>),
    /// The desktop refused to open a decoded attachment; the portal's answer, which arrives
    /// after the launch rather than from it.
    AttachmentOpenFailed,
    WebViewReady,
    WebViewUnavailable,
    /// The reading document has painted, so the pane may reveal it. Carries nothing: its whole
    /// job is to provoke the render that swaps the canvas for the page.
    ReadingBodyPainted,
    OpenSettings,
    OpenAccountSetup,
    RestartAccountSetup,
    /// Back from the second step to the address, which stays as typed.
    AccountSetupBack,
    /// A pick on the link step: which picker, and which of its options (`None` for none).
    SetupLinkPicked(usize, Option<usize>),
    /// The link step is done: link what is picked, or skip.
    SetupLinksDone(bool),
    /// "Add another account" from the link step.
    SetupAddLinkedAccount,
    /// The account needs no name asked; the next one waiting is.
    SenderNameNotNeeded,
    CancelAccountSetup,
    ManualAccountSetup(String),
    EditDetectedManually,
    SelectAccountKind(Box<ManualForm>),
    ProbeManualJmapSignIn(Box<ManualForm>),
    DetectAccount(String),
    AccountDetected(String, Box<DetectedSetup>),
    JmapOAuthAvailable {
        email: String,
        server_url: String,
        available: bool,
    },
    SubmitAccount(Box<AccountSubmission>),
    /// A manual add finished: the account's id, or why it failed. The id is what raises the
    /// "your name" step, so a route that cannot report one raises nothing.
    AccountAdded(Result<String, super::setup_model::ConnectFailure>),
    /// The provider answered what it already calls this person, so the "your name" step can
    /// open seeded. Carried back from a worker thread because the read is a provider round
    /// trip; empty is the ordinary IMAP answer and means *ask*.
    SenderNameSuggested {
        account: String,
        suggestion: String,
    },
    /// Set the name an account's outgoing mail is sent under; empty clears it. Passed through
    /// unchanged, the core sanitises it (`docs/sending.md`).
    SetAccountSenderName {
        account: String,
        name: String,
    },
    /// Close the "your name" step without setting one: the account keeps sending as a bare
    /// address.
    DismissSenderNamePrompt,
    /// The address to sign in, and what the account is used for.
    StartGoogleLogin(String, Option<Vec<AccountCapability>>),
    CancelGoogleLogin,
    GoogleCallbackReceived(u64),
    GoogleFinished(u64, GoogleOutcome),
    /// The address to sign in, and what the account is used for.
    StartMicrosoftLogin(String, Option<Vec<AccountCapability>>),
    CancelMicrosoftLogin,
    MicrosoftCallbackReceived(u64),
    MicrosoftFinished(u64, MicrosoftOutcome),
    StartJmapLogin(String, String),
    ProbeManualImapSignIn(Box<ManualForm>),
    ImapAuthAnswered {
        email: String,
        imap_host: String,
        offer: Box<ImapAuthOffer>,
    },
    StartImapLogin(Box<ImapForm>),
    CancelImapLogin,
    ImapPrepared(u64, Result<Box<ImapPrepared>, String>),
    ImapCallbackReceived(u64),
    ImapFinished(u64, ImapOutcome),
    CancelJmapLogin,
    JmapPrepared(u64, Result<Box<JmapPrepared>, String>),
    JmapCallbackReceived(u64),
    JmapFinished(u64, JmapOutcome),
    JmapReauthPrepared(u64, Result<Box<JmapReauthPrepared>, String>),
    JmapReauthFinished(u64, JmapReauthOutcome),
    ReplaceAccountSecret {
        account: String,
        secret: String,
    },
    AccountSecretReplaced {
        account: String,
        success: bool,
    },
    StartAllodiaSignIn,
    StartAllodiaRegistration,
    ManageAllodiaAccount,
    CancelAllodiaSignIn,
    /// The browser hop for this attempt has outlasted the card's threshold, so the
    /// first-run card owes the person a way back ([`super::setup_onboarding`]).
    AllodiaSignInSlow(u64),
    AllodiaSignInFinished(u64, AllodiaOutcome),
    SignOutOfAllodia,
    /// Ask the account service what the person's other devices hold, and tell it what this one
    /// holds. Emitted at boot and after a sign-in; never by a timer.
    /// Everything the subscription section asks for, in one variant: it is one screen's
    /// traffic, and every piece of it ends in the same re-read.
    AllodiaSubscription(SubscriptionInput),
    SyncAllodiaAccounts,
    AllodiaSyncFinished(AllodiaSyncOutcome),
    /// Move one account to a sync position.
    SetAllodiaAccountSyncMode(String, mailcal_bindings::AllodiaAccountSyncMode),
    AllodiaSyncModeChanged(String, Option<String>),
    /// Re-read how each account is shared, after the account list changed.
    ReadAccountsSynced,
    /// The Settings sidebar moved to a page. Records it so a later redraw rebuilds on the
    /// page the person is looking at rather than the one the model last named.
    SettingsCategoryShown(crate::ui::settings::Category),
    /// Set up an account one of the person's other devices offered, on the route its record
    /// names rather than one re-derived from the address.
    SetUpOfferedAccount(Box<mailcal_bindings::AllodiaAccountOffer>),
    /// Settings → Accounts' account pages, as one input.
    Accounts(crate::ui::account_settings::AccountsInput),
    RemoveAccount(String),
    AccountRemoved(Result<(), String>),
    AnalyticsDecided(bool),
    CollectNewMail,
    BackgroundFinished,
}

/// The name each input logs under.
#[path = "input_names.rs"]
mod names;
