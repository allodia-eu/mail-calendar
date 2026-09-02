//! Relm4 model and GTK/libadwaita three-pane mail shell.

use std::{collections::HashSet, sync::Arc};

use mailcal_bindings::{Intent, MailboxListSnapshot, MailcalApp, ReplyPrompt, ViewMode};

use crate::{
    l10n,
    preferences::{self, HostPreferences},
    secrets::SecretStore,
};

mod allodia;
mod allodia_sync;
mod avatar;
mod calendar;
mod calendar_actions;
mod component;
mod composer;
mod composer_draft;
mod composer_header;
mod composer_model;
mod composer_signature;
mod connectivity;
mod contacts;
mod contacts_actions;
pub(crate) mod destinations;
mod dns;
mod folder_pane;
mod google;
mod host_tasks;
mod imap_actions;
mod imap_signin;
mod input;
mod invitation;
mod invitation_actions;
mod jmap;
mod jmap_actions;
mod mail_actions;
mod mailbox;
mod mailbox_display;
mod mailbox_progressive;
mod mailbox_reconcile;
mod mcp;
mod microsoft;
mod modal;
mod model;
mod notifications;
mod oauth_actions;
mod oauth_loopback;
mod operations;
mod reading;
mod recipients;
mod row_action;
mod runtime_timers;
mod search;
mod search_actions;
mod settings;
mod setup;
mod setup_google;
mod setup_imap;
mod setup_jmap;
mod setup_manual;
#[cfg(test)]
mod setup_manual_tests;
mod setup_microsoft;
mod setup_model;
mod setup_onboarding;
#[cfg(test)]
mod setup_onboarding_tests;
#[cfg(test)]
mod setup_signin_tests;
#[cfg(test)]
mod setup_widget_tests;
mod setup_widgets;
mod shell;
#[cfg(any(debug_assertions, feature = "dev-harness"))]
mod showcase_hooks;
mod signature_image;
mod time_zone;
mod timestamps;
mod unfiled_copy;
mod update;
mod web_security;
mod webview;
mod welcome;

use calendar::CalendarModel;
use composer_draft::PendingNavigation;
use composer_model::{ComposeKind, ComposeRequest, initial_sender, quote_seed};
use connectivity::ConnectivityState;
use contacts::ContactsModel;
pub(crate) use destinations::PrimaryView;
use host_tasks::HostTasks;
pub(crate) use input::AppInput;
use mail_actions::MessageTarget;
use mailbox::ThreadKey;
#[cfg(any(debug_assertions, feature = "dev-harness"))]
use model::OpenedMessage;
use model::ReadingState;
use search::SearchState;
use setup::SetupState;
use unfiled_copy::UnfiledCopyNotice;

pub(crate) struct AppModel {
    app: Option<Arc<MailcalApp>>,
    secrets: Option<Arc<SecretStore>>,
    preferences: Arc<HostPreferences>,
    connectivity: ConnectivityState,
    /// Retained for the lifetime of the component so GIO keeps delivering default-network changes.
    _network_monitor: gtk::gio::NetworkMonitor,
    /// Retained so GLib keeps refreshing calendars throughout the foreground session.
    _calendar_refresh_timer: gtk::glib::SourceId,
    /// Retained so GLib keeps checking the device zone throughout the foreground session.
    _device_timezone_timer: gtk::glib::SourceId,
    device_zone: time_zone::DeviceZoneMonitor,
    primary: PrimaryView,
    snapshot: MailboxListSnapshot,
    /// The conversations the user has opened inline. Host state, not the core's: it survives a
    /// snapshot refresh, so a background sync doesn't collapse a conversation being read.
    expanded_threads: HashSet<ThreadKey>,
    calendar: CalendarModel,
    contacts: ContactsModel,
    /// What the mail-search chrome is showing. Not the results: those arrive in `snapshot` like
    /// any other list, so this client keeps no second copy of them.
    search: SearchState,
    reading: ReadingState,
    /// Which reading snapshot the pane has drawn. Bumped on every `Surface::Reading` pull, so the
    /// invitation card is rebuilt when the core publishes a new one and left alone on every other
    /// render; a rebuild mid-render would take a half-typed note to the organiser away.
    reading_generation: u64,
    pending_mail_delete: Option<MessageTarget>,
    composer: Option<ComposeRequest>,
    composer_generation: u64,
    composer_error: bool,
    /// The message or external draft waiting for the open composer to answer whether it is dirty.
    pending_navigation: Option<PendingNavigation>,
    /// A mail link received before an account exists. Account setup completing opens it.
    pending_mailto: Option<mailcal_bindings::MailtoPrefill>,
    /// The navigation the guard must answer, and the counter it is drawn from. Its own sequence,
    /// not the composer's: two navigations away from one draft: the second after a "Keep editing"
    /// ; must each get an answer, and reusing the composer's generation would make the pane treat
    /// the second as already asked.
    draft_check: Option<u64>,
    draft_check_seq: u64,
    /// Whether the "Discard draft?" question is on screen.
    discard_prompt: bool,
    notice: Option<String>,
    /// The mail list's bottom-bar caption while a background sync is downloading mail. `None`
    /// whenever nothing is arriving unasked, which is almost always.
    sync_hint: Option<String>,
    /// The separate foreground-download row. It wins the shared bottom strip while active.
    sync_bar: Option<model::SyncBar>,
    unfiled_copy: Option<UnfiledCopyNotice>,
    /// The standing "the organiser wasn't told" question, mirrored from the core. `None` is also
    /// how the core says *close the modal*; it clears the question the moment it is answered.
    reply_prompt: Option<ReplyPrompt>,
    /// Which question the modal is showing. The core carries no id, so the host counts, exactly as
    /// the calendar's dialogs are counted.
    reply_prompt_generation: u64,
    boot_error: Option<String>,
    webview_available: bool,
    calendar_refresh: calendar_actions::CalendarRefreshGate,
    calendar_manager_generation: u64,
    setup: SetupState,
    /// What this launch knows about the person's other devices, apart from the pass itself.
    allodia: allodia_sync::AllodiaLaunch,
    credential_repair_failed: Option<String>,
    /// What the next Settings render should show, and whether it opens the window or only redraws
    /// an open one ([`settings::SettingsState`]).
    settings: settings::SettingsState,
    host_tasks: HostTasks,
    /// The screenshot screen still waiting on something asynchronous. Only `Reply` ever waits: it
    /// cannot begin until the opened message's body has arrived, and that arrives on the observer.
    #[cfg(any(debug_assertions, feature = "dev-harness"))]
    showcase_pending: Option<crate::showcase::ShowcaseScreen>,
}

impl AppModel {
    fn dispatch(&self, intent: Intent) {
        if let Some(app) = &self.app {
            app.dispatch(intent);
        }
    }

    /// Whether the account-setup window may open. It needs somewhere to put the credential it
    /// will collect; except in a showcase build, which has no credential store *by design*
    /// (nothing a screenshot run does may reach the developer's keyring) and whose whole
    /// purpose on this screen is to photograph the window. Without the exception the
    /// `add-account` capture silently photographed the message list instead.
    fn can_open_account_setup(&self) -> bool {
        #[cfg(any(debug_assertions, feature = "dev-harness"))]
        if crate::showcase::is_on() {
            return true;
        }
        self.secrets.is_some()
    }

    #[cfg(any(debug_assertions, feature = "dev-harness"))]
    fn open_row(&mut self, index: usize) {
        let Some(row) = self.snapshot.rows.get(index) else {
            return;
        };
        self.open_message(OpenedMessage::from_row(row));
    }

    /// Records a conversation's inline disclosure and, on opening one, reads its representative
    /// message; the three-pane behaviour macOS and Windows share. Collapsing leaves the reading
    /// pane where it is: the user is closing a list row, not the message they are reading.
    fn set_thread_expanded(&mut self, thread: &ThreadKey, expanded: bool) {
        if !expanded {
            self.expanded_threads.remove(thread);
            return;
        }
        self.expanded_threads.insert(thread.clone());
        if let Some(message) = mailbox::thread_representative(&self.snapshot, thread) {
            self.open_message(message);
        }
    }

    fn retry_open(&self) {
        if let Some(opened) = &self.reading.opened {
            self.dispatch(Intent::OpenMessage {
                account: opened.account.clone(),
                key: opened.key.clone(),
            });
        }
    }

    fn begin_compose(&mut self, kind: ComposeKind) {
        let Some(app) = &self.app else {
            return;
        };
        let opened = self.reading.opened.as_ref();
        if kind != ComposeKind::New && opened.is_none() {
            return;
        }
        let (initial_to, initial_cc) = match (kind, opened) {
            (ComposeKind::Reply | ComposeKind::ReplyAll, Some(message)) => {
                let recipients = app.reply_recipients(
                    message.account.clone(),
                    message.key.clone(),
                    kind == ComposeKind::ReplyAll,
                );
                (recipients.to, recipients.cc)
            }
            _ => (String::new(), String::new()),
        };
        let quote = opened.and_then(|message| {
            let settings = app.quote_settings();
            // Only a showcase reply arrives pre-written, so the store screenshot shows a written
            // reply rather than an empty body. Every real reply passes `None`.
            #[cfg(any(debug_assertions, feature = "dev-harness"))]
            let initial_text = (kind == ComposeKind::Reply || kind == ComposeKind::ReplyAll)
                .then(|| crate::showcase::reply_text(&message.account, &message.key))
                .flatten();
            #[cfg(not(any(debug_assertions, feature = "dev-harness")))]
            let initial_text: Option<String> = None;
            quote_seed(
                message,
                &self.reading.snapshot,
                &settings.style,
                kind == ComposeKind::Forward,
                initial_text.as_deref(),
                self.calendar.display_zone(),
            )
        });
        let subject = match (kind, opened) {
            (ComposeKind::Reply | ComposeKind::ReplyAll, Some(message)) => {
                l10n::subject_reply(&message.subject)
            }
            (ComposeKind::Forward, Some(message)) => l10n::subject_forward(&message.subject),
            _ => String::new(),
        };
        self.composer_generation = self.composer_generation.wrapping_add(1);
        self.composer_error = false;
        self.composer = Some(ComposeRequest {
            kind,
            account: opened.map(|message| message.account.clone()),
            key: opened.map(|message| message.key.clone()),
            initial_to,
            initial_cc,
            initial_bcc: String::new(),
            subject,
            initial_body: None,
            quote,
            initial_from: initial_sender(
                opened,
                self.snapshot.selected_account.as_deref(),
                app.default_send_account(),
            ),
            seeds_signature: true,
        });
    }

    /// What the message list calls itself: the folder on screen, or; while a search is running
    /// ; the results, as macOS and Windows label the same list.
    fn list_title(&self) -> String {
        if self.search.is_active() {
            return l10n::search_results().to_owned();
        }
        folder_pane::header_title(&self.snapshot)
    }

    fn subtitle(&self) -> String {
        if let Some(error) = &self.boot_error {
            return l10n::status_connect_failed(error);
        }
        let total = i64::try_from(self.snapshot.total).unwrap_or(i64::MAX);
        match self.snapshot.mode {
            ViewMode::Flat => l10n::mailbox_count_messages(total),
            ViewMode::Threaded => l10n::mailbox_count_conversations(total),
        }
    }
}
